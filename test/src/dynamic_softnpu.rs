use crate::packet;                               // brings in the IPv4/IPv6 packet-building helpers (packet::v4, packet::v6)
use colored::Colorize;                           // enables .magenta()/.dimmed() for colored terminal output in test logs
use p4rs::packet_in;                             // the P4 runtime's "packet being parsed" type, fed to compiled pipelines
use rand::Rng;                                   // RNG trait, used to generate a random fake MAC per port
use std::net::{Ipv4Addr, Ipv6Addr};              // IP address types used by Interface4/Interface6
use std::sync::atomic::{AtomicUsize, Ordering};  // lock-free counters for tx/rx stats, safely shared across threads
use std::sync::Arc;                              // shared ownership handle, needed since OuterPhy is handed to test code
use std::thread::spawn;                          // spawns the background thread that runs the packet-processing loop
use std::collections::HashMap;
use xfr::{ring, FrameBuffer, RingConsumer, RingProducer}; // the ring-buffer library backing every port's rx/tx queues

pub fn do_expect_frames(                         // test helper: block until n frames arrive, then assert they match
    name: &str,                                  // label for log output (usually the phy variable's own name via stringify!)
    phy: &Arc<OuterPhy<RING, FBUF, MTU>>,        // which port to read from
    expected: &[RxFrame],                        // what the test expects to see, in order
    dmac: Option<[u8; 6]>,                       // optional expected destination MAC (None = don't check it)
) {
    let n = expected.len();                      // how many frames we're waiting for
    let mut frames = Vec::new();                 // accumulator for received frames
    loop {
        let fs = phy.recv();                     // blocking-ish receive: returns whatever frames are currently available
        frames.extend_from_slice(&fs);           // append them to what we've collected so far
        // TODO this is not a great interface, if frames.len() > n, we should do
        // something besides hang forever.
        if frames.len() == n {                   // stop once we have exactly as many as expected
            break;
        }
    }
    for i in 0..n {                              // check each received frame against its expected counterpart
        let payload = match frames[i].ethertype { // strip known header bytes to isolate the actual payload
            0x0901 => {                           // sidecar-tagged frame
                let mut payload = &frames[i].payload[..];
                let et = u16::from_be_bytes([payload[5], payload[6]]); // read sc_ether_type out of the sidecar header
                payload = &payload[23..];         // skip past the 23-byte sidecar header
                if et == 0x86dd {                 // inner frame is IPv6
                    payload = &payload[40..];     // skip the 40-byte IPv6 header too
                }
                if et == 0x0800 {                 // inner frame is IPv4
                    payload = &payload[20..];     // skip the 20-byte IPv4 header too
                }
                payload
            }
            0x86dd => &frames[i].payload[40..],   // plain IPv6: skip fixed 40-byte header
            0x0800 => &frames[i].payload[20..],   // plain IPv4: skip minimum 20-byte header
            _ => &frames[i].payload[..],          // unknown ethertype: don't strip anything
        };
        let m = String::from_utf8_lossy(payload).to_string(); // best-effort render payload as text, for logging
        println!("[{}] {}", name.magenta(), m.dimmed());      // log what was received
        assert_eq!(frames[i].src, expected[i].src, "src");     // verify source MAC matches
        if let Some(d) = dmac {
            assert_eq!(frames[i].dst, d, "dst");                // verify destination MAC, only if caller cared
        }
        assert_eq!(frames[i].ethertype, expected[i].ethertype, "ethertype"); // verify ethertype matches
        assert_eq!(payload, expected[i].payload, "payload");    // verify payload bytes match
    }
}

#[macro_export]
macro_rules! dynamic_expect_frames {                     // sugar so tests can write dynamic_expect_frames!(phy, &[...]) instead of the full call
    ($phy:expr, $expected:expr) => {
        $crate::dynamic_softnpu::do_expect_frames(
            stringify!($phy),                    // auto-derive the "name" arg from the variable's own source text
            &$phy,
            $expected,
            None,                                // this arm: no dmac check
        )
    };
    ($phy:expr, $expected:expr, $dmac:expr) => {  // second arm: caller supplied a dmac to check
        $crate::dynamic_softnpu::do_expect_frames(
            stringify!($phy),
            &$phy,
            $expected,
            Some($dmac),
        )
    };
}

const RING: usize = 1024;                        // number of slots in each ring buffer
const FBUF: usize = 4096;                        // number of frame-buffer slots backing those rings
const MTU: usize = 1500;                         // max bytes per frame
const RESUBMIT_STACK: u16 = 0x8000;
const MAX_RESUBMIT_HOPS: u8 = 8;

pub struct SoftNpu<P: p4rs::Pipeline> {          // the emulated ASIC, generic over whatever compiled P4 program it runs
    pub pipeline: Option<P>,                      // the pipeline itself; Option so it can be taken exactly once by run()
    inner_phys: Option<Vec<InnerPhy<RING, FBUF, MTU>>>, // pipeline-facing port halves; same take-once pattern
    outer_phys: Vec<Arc<OuterPhy<RING, FBUF, MTU>>>,    // test-facing port halves, kept for the life of the SoftNpu
    _fb: Arc<FrameBuffer<FBUF, MTU>>,             // shared backing storage; held only to keep it alive (never read directly)
    user_program: Option<HashMap<u16, Box<dyn p4rs::Pipeline>>>,
}

impl<P: p4rs::Pipeline + 'static> SoftNpu<P> {
    /// Create a new SoftNpu ASIC emulator. The `radix` indicates the number of
    /// ports. The `pipeline` is the `x4c` compiled program that the ASIC will
    /// run. When `cpu_port` is set to true, sidecar data in `TxFrame` elements
    /// will be added to packets sent through port 0 (as a sidecar header) on
    /// the way to the ASIC.
    pub fn new(radix: usize, pipeline: P, cpu_port: bool) -> Self {
        let fb = Arc::new(FrameBuffer::<FBUF, MTU>::new()); // one shared buffer pool for every port
        let mut inner_phys = Vec::new();          // will hold one InnerPhy per port
        let mut outer_phys = Vec::new();          // will hold one OuterPhy per port
        for i in 0..radix {                       // build each port
            let (rx_p, rx_c) = ring::<RING, FBUF, MTU>(fb.clone()); // rx ring: producer/consumer pair
            let (tx_p, tx_c) = ring::<RING, FBUF, MTU>(fb.clone()); // tx ring: producer/consumer pair
            let inner_phy = InnerPhy::new(i, rx_c, tx_p);           // pipeline reads rx, writes tx
            let mut outer_phy = OuterPhy::new(i, rx_p, tx_c);       // test code writes rx, reads tx (opposite ends)
            inner_phys.push(inner_phy);
            if i == 0 && cpu_port {               // special-case port 0 as the "scrimlet" link
                outer_phy.sidecar_encap = true;   // frames sent through it get auto-wrapped in a sidecar header
            }
            outer_phys.push(Arc::new(outer_phy)); // wrap in Arc so test code can hold shared references
        }
        let inner_phys = Some(inner_phys);        // wrap for the take-once pattern used by run()
        SoftNpu {
            inner_phys,
            outer_phys,
            _fb: fb,                              // keep the buffer alive as long as the SoftNpu exists
            pipeline: Some(pipeline),              // same take-once wrapping as inner_phys
            user_program: Some(HashMap::new()),
        }
    }

    pub fn load_user_program(
        &mut self,
        program_id: u16,
        pipeline: impl p4rs::Pipeline + 'static,
    ) {
        self.user_program
            .as_mut()
            .expect("cannot load a user program after run() has started")
            .insert(program_id, Box::new(pipeline));
    }

    pub fn run(&mut self) {
        let inner_phys = match self.inner_phys.take() { // pull the vector out, leaving None behind
            Some(phys) => phys,
            None => panic!("phys already in use"),      // catches a double-call to run()
        };
        let pipe = match self.pipeline.take() {   // same take-once extraction for the pipeline
            Some(pipe) => pipe,
            None => panic!("pipe already in use"),
        };
        let user_program = self.user_program.take().unwrap_or_default();
        spawn(move || {                           // hand both off to a new background thread
            Self::do_run(inner_phys, pipe, user_program);       // and start the infinite processing loop there
        });
    }

    fn do_run(
        inner_phys: Vec<InnerPhy<RING, FBUF, MTU>>, 
        mut pipeline: P,
        mut user_program: HashMap<u16, Box<dyn p4rs::Pipeline>>,
    ) {
        loop {                                     // runs forever, processing traffic across all ports
            // TODO: yes this is a highly suboptimal linear gather-scatter across
            // each ingress. Will update to something more concurrent eventually.
            for (i, ig) in inner_phys.iter().enumerate() { // visit each port in turn as a potential ingress
                let mut egress_count = vec![0; inner_phys.len()]; // tracks how many frames go out each port this round
                let mut frames_in = 0;             // tracks how many frames were consumed from this ingress this round

                for fp in ig.rx_c.consumable() {   // iterate every frame currently waiting on this port's rx ring
                    frames_in += 1;
                    let content = ig.rx_c.read_mut(fp); // get a mutable view of this frame's raw bytes

                    let mut pkt = packet_in::new(content); // wrap the bytes for the pipeline to parse

                    let port = i as u16;           // the ingress port number, as the pipeline expects it
                    let mut output = pipeline.process_packet(port, &mut pkt); // the one call into the compiled P4 program

                    let mut hops = 0u8;
                    while let Some(idx) = output.iter().position(|(_, p)| *p >= RESUBMIT_STACK) {
                        let (_, marker) = output.remove(idx);
                        let program_id = marker - RESUBMIT_STACK;
                        hops += 1;
                        if hops > MAX_RESUBMIT_HOPS {
                            eprintln!("dropping packet: exceeded max resubmit hops (target {program_id})");
                            continue;
                        }
                        match user_program.get_mut(&program_id) {
                            Some(user_pipeline) => {
                                let mut resubmit_pkt = packet_in::new(content);
                                let mut resubmitted = user_pipeline.process_packet(port, &mut resubmit_pkt);
                                output.append(&mut resubmitted);
                            }
                            None => {
                                eprintln!("dropping packet: no user program loaded for id {program_id}");
                            }
                        }
                    }

                    for (out_pkt, out_port) in &output { // for every (packet, destination port) the pipeline produced
                        let out_port = *out_port as usize;
                        //
                        // get frame for packet
                        //
                        let phy = &inner_phys[out_port]; // look up that destination port's InnerPhy
                        let eg = &phy.tx_p;               // its tx producer, where we'll write the outgoing frame
                        let mut fps = eg.reserve(1).unwrap(); // reserve one slot in that port's tx ring
                        let fp = fps.next().unwrap();          // get a handle to the reserved slot

                        //
                        // emit headers
                        //
                        eg.write_at(fp, out_pkt.header_data.as_slice(), 0); // write header bytes at offset 0

                        //
                        // emit payload
                        //
                        eg.write_at(
                            fp,
                            out_pkt.payload_data,               // write payload bytes
                            out_pkt.header_data.len(),           // right after the headers
                        );

                        egress_count[out_port] += 1;   // record that this port got one more outgoing frame
                    }
                }
                ig.rx_c.consume(frames_in).unwrap();   // mark all those ingress frames as consumed, freeing ring space
                ig.rx_counter.fetch_add(frames_in, Ordering::Relaxed); // update this port's rx stat counter

                for (j, n) in egress_count.iter().enumerate() { // now flush all the outgoing frames we staged above
                    if *n == 0 {
                        continue;                    // nothing was sent out this port this round, skip it
                    }
                    let phy = &inner_phys[j];
                    phy.tx_p.produce(*n).unwrap();    // commit those n reserved slots, making them visible to readers
                    phy.tx_counter.fetch_add(*n, Ordering::Relaxed); // update this port's tx stat counter
                }
            }
        }
    }

    pub fn phy(&self, i: usize) -> Arc<OuterPhy<RING, FBUF, MTU>> {
        self.outer_phys[i].clone()                 // hand the test code a shared handle to port i's test-facing half
    }
}

pub struct InnerPhy<const R: usize, const N: usize, const F: usize> { // pipeline's view of one port
    pub index: usize,                              // this port's number
    rx_c: RingConsumer<R, N, F>,                   // reads packets that arrived (from the OuterPhy's producer side)
    tx_p: RingProducer<R, N, F>,                   // writes packets to send out (read by the OuterPhy's consumer side)
    tx_counter: AtomicUsize,                       // running count of frames sent
    rx_counter: AtomicUsize,                       // running count of frames received
}

pub struct OuterPhy<const R: usize, const N: usize, const F: usize> { // test code's view of the same port
    pub index: usize,                              // this port's number
    pub mac: [u8; 6],                              // this port's simulated MAC address
    rx_p: RingProducer<R, N, F>,                   // injects packets as if they arrived on the wire
    tx_c: RingConsumer<R, N, F>,                   // reads packets the pipeline sent out
    tx_counter: AtomicUsize,                       // running count of frames sent (via this OuterPhy)
    rx_counter: AtomicUsize,                       // running count of frames received (via this OuterPhy)
    sidecar_encap: bool,                           // if true, auto-wrap outgoing frames in a sidecar header
}

unsafe impl<const R: usize, const N: usize, const F: usize> Send  // manually assert this type can move between threads
    for OuterPhy<R, N, F>
{
}

unsafe impl<const R: usize, const N: usize, const F: usize> Sync  // manually assert this type can be shared between threads
    for OuterPhy<R, N, F>
{
}

pub struct Interface6<const R: usize, const N: usize, const F: usize> { // convenience wrapper: a port bound to an IPv6 address
    pub phy: Arc<OuterPhy<R, N, F>>,                // the underlying port
    pub addr: Ipv6Addr,                             // this interface's own IPv6 address (used as packet source)
    pub sc_egress: u16,                             // sidecar egress port to stamp on outgoing frames, if relevant
}

impl<const R: usize, const N: usize, const F: usize> Interface6<R, N, F> {
    pub fn new(phy: Arc<OuterPhy<R, N, F>>, addr: Ipv6Addr) -> Self {
        Self {
            phy,
            addr,
            sc_egress: 0,                          // defaults to port 0
        }
    }

    pub fn send(
        &self,
        mac: [u8; 6],                              // destination MAC for the frame
        ip: Ipv6Addr,                              // destination IP
        payload: &[u8],                            // application payload to wrap
    ) -> Result<(), anyhow::Error> {
        let n = 40 + payload.len();                // total size = fixed 40-byte IPv6 header + payload
        let mut buf = [0u8; F];                    // scratch buffer sized to the port's MTU
        packet::v6(self.addr, ip, payload, &mut buf); // build the IPv6 header + payload into buf
        let mut txf = TxFrame::new(mac, 0x86dd, &buf[..n]); // wrap as a TxFrame with IPv6 ethertype
        txf.sc_egress = self.sc_egress;            // stamp the configured sidecar egress port
        self.phy.send(&[txf])?;                    // hand it to the underlying port to actually transmit
        Ok(())
    }
}

pub struct Interface4<const R: usize, const N: usize, const F: usize> { // same idea, for IPv4
    pub phy: Arc<OuterPhy<R, N, F>>,
    pub addr: Ipv4Addr,
    pub sc_egress: u16,
}

impl<const R: usize, const N: usize, const F: usize> Interface4<R, N, F> {
    pub fn new(phy: Arc<OuterPhy<R, N, F>>, addr: Ipv4Addr) -> Self {
        Self { 
            phy, 
            addr, 
            sc_egress: 0,
        }
    }

    pub fn send_ipv4(
        &self, 
        mac: [u8; 6], 
        ip: Ipv4Addr, 
        payload: &[u8],
    ) -> Result<(), anyhow::Error> {
        let n = 20 + payload.len();
        let mut buf = [0u8; F];
        packet::v4(self.addr, ip, payload, &mut buf);
        let mut txf = TxFrame::new(mac, 0x0800, &buf[..n]);
        txf.sc_egress = self.sc_egress;
        self.phy.send(&[txf])?;
        Ok(())
    }

    pub fn send_udp(
        &self,
        mac: [u8; 6],
        ip: Ipv4Addr,
        dst_port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, anyhow::Error> {
        let buf = packet::v4_udp(self.addr, ip, dst_port, payload);
        let mut txf = TxFrame::new(mac, 0x0800, &buf);
        txf.sc_egress = self.sc_egress;
        self.phy.send(&[txf])?;
        Ok(buf)
    }

    pub fn send_geneve(
        &self,
        mac: [u8; 6],
        ip: Ipv4Addr,
        inner_dst_mac: [u8; 6],
        inner_src_mac: [u8; 6],
        inner_ether_type: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, anyhow::Error> {
        let buf = packet::v4_udp_geneve_eth(self.addr, ip, inner_dst_mac, inner_src_mac, inner_ether_type, payload);
        let mut txf = TxFrame::new(mac, 0x0800, &buf);
        txf.sc_egress = self.sc_egress;
        self.phy.send(&[txf])?;
        Ok(buf)
    }
}

pub struct TxFrame<'a> {
    pub dst: [u8; 6],
    pub ethertype: u16,
    pub payload: &'a [u8],
    pub sc_egress: u16,
    pub vid: Option<u16>,
}

pub struct RxFrame<'a> {
    pub src: [u8; 6],
    pub ethertype: u16,
    pub payload: &'a [u8],
    pub vid: Option<u16>,
}

impl<'a> RxFrame<'a> {
    pub fn new(src: [u8; 6], ethertype: u16, payload: &'a [u8]) -> Self {
        Self { src, ethertype, payload, vid: None }
    }
    pub fn newv(src: [u8; 6], ethertype: u16, payload: &'a [u8], vid: u16) -> Self {
        Self { src, ethertype, payload, vid: Some(vid) }
    }
}

impl<'a> TxFrame<'a> {
    pub fn new(dst: [u8; 6], ethertype: u16, payload: &'a [u8]) -> Self {
        Self { dst, ethertype, payload, sc_egress: 0, vid: None }
    }
    pub fn newv(dst: [u8; 6], ethertype: u16, payload: &'a [u8], vid: u16) -> Self {
        Self { dst, ethertype, payload, sc_egress: 0, vid: Some(vid) }
    }
}

#[derive(Clone)]
pub struct OwnedFrame {                            // a frame that was actually received, fully owned (not borrowed)
    pub dst: [u8; 6],
    pub src: [u8; 6],
    pub vid: Option<u16>,
    pub ethertype: u16,
    pub payload: Vec<u8>,                          // owned copy, since the underlying ring slot may get reused
}

impl OwnedFrame {
    pub fn new(                                     // plain constructor, just assigns every field
        dst: [u8; 6],
        src: [u8; 6],
        ethertype: u16,
        vid: Option<u16>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            dst,
            src,
            vid,
            ethertype,
            payload,
        }
    }
}

impl<const R: usize, const N: usize, const F: usize> InnerPhy<R, N, F> {
    pub fn new(
        index: usize,
        rx_c: RingConsumer<R, N, F>,
        tx_p: RingProducer<R, N, F>,
    ) -> Self {
        Self {
            index,
            rx_c,
            tx_p,
            tx_counter: AtomicUsize::new(0),        // start counters at zero
            rx_counter: AtomicUsize::new(0),
        }
    }
}

impl<const R: usize, const N: usize, const F: usize> OuterPhy<R, N, F> {
    pub fn new(
        index: usize,
        rx_p: RingProducer<R, N, F>,
        tx_c: RingConsumer<R, N, F>,
    ) -> Self {
        let mut rng = rand::rng();                  // get a random number generator
        let m = rng.random_range::<u32, _>(0xf00000..0xffffff).to_le_bytes(); // random 24-bit suffix for the MAC
        let mac = [0xa8, 0x40, 0x25, m[0], m[1], m[2]]; // 0xa84025 is Oxide's OUI prefix, rest is randomized

        Self {
            index,
            rx_p,
            tx_c,
            mac,
            tx_counter: AtomicUsize::new(0),
            rx_counter: AtomicUsize::new(0),
            sidecar_encap: false,                   // off by default; SoftNpu::new turns it on for port 0 if requested
        }
    }



    pub fn send(&self, frames: &[TxFrame<'_>]) -> Result<(), xfr::Error> {
        let n = frames.len();                       // how many frames to send in this batch
        let fps = self.rx_p.reserve(n)?;             // reserve n slots on the rx ring (this injects "arriving" traffic)
        for (i, fp) in fps.enumerate() {             // fill in each reserved slot
            let f = &frames[i];
            self.rx_p.write_at(fp, f.dst.as_slice(), 0);  // bytes 0-5: destination MAC
            self.rx_p.write_at(fp, &self.mac, 6);         // bytes 6-11: this port's own MAC as source
            let mut off = 12;                             // next write position, right after both MACs
            if self.sidecar_encap {                       // this port wraps everything in a sidecar header
                self.rx_p
                    .write_at(fp, 0x0901u16.to_be_bytes().as_slice(), off); // sidecar ethertype
                off += 2;
                // sc_code = SC_FWD_FROM_USERSPACE
                self.rx_p.write_at(fp, &[0u8], off);       // sc_code byte
                off += 1;
                // sc_ingress
                let ingress = f.sc_egress;                 // (reuses sc_egress value for both fields)
                self.rx_p
                    .write_at(fp, ingress.to_be_bytes().as_slice(), off); // sc_ingress field
                off += 2;
                // sc_egress
                let egress = f.sc_egress;
                self.rx_p.write_at(fp, egress.to_be_bytes().as_slice(), off); // sc_egress field
                off += 2;
                // sc_ether_type
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),  // the frame's real ethertype, now nested inside sidecar
                    off,
                );
                off += 2;
                // sc_payload
                self.rx_p.write_at(fp, [0u8; 16].as_slice(), off); // 16 zero bytes of reserved sidecar payload
                off += 16;
            } else if let Some(vid) = f.vid {              // no sidecar, but this frame wants a VLAN tag
                self.rx_p
                    .write_at(fp, 0x8100u16.to_be_bytes().as_slice(), off); // 802.1Q tag-protocol-id
                off += 2;
                self.rx_p.write_at(fp, vid.to_be_bytes().as_slice(), off); // the VLAN ID itself
                off += 2;
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),  // real ethertype, after the VLAN tag
                    off,
                );
                off += 2;
            } else {                                        // plain frame: no sidecar, no VLAN
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),  // just the ethertype
                    off,
                );
                off += 2;
            }

            self.rx_p.write_at(fp, f.payload, off);        // finally, write the actual payload bytes
        }
        self.rx_p.produce(n)?;                             // commit all n slots, making them visible to the pipeline
        self.tx_counter.fetch_add(n, Ordering::Relaxed);   // bump this OuterPhy's "sent" counter
        Ok(())
    }

    pub fn recv(&self) -> Vec<OwnedFrame> {
        let mut buf = Vec::new();                          // accumulator for decoded frames
        loop {
            for fp in self.tx_c.consumable() {             // iterate every frame currently waiting on tx
                let b = self.tx_c.read(fp);                // raw bytes of this frame
                let mut et = u16::from_be_bytes([b[12], b[13]]); // read ethertype at its normal offset
                let mut vid: Option<u16> = None;
                let payload = if et == 0x8100 {            // it's actually a VLAN tag, not the real ethertype
                    let v = u16::from_be_bytes([b[14], b[15]]); // read the VLAN ID
                    et = u16::from_be_bytes([b[16], b[17]]);    // re-read the real ethertype, shifted by 4 bytes
                    vid = Some(v);
                    b[18..].to_vec()                        // payload starts after the VLAN tag
                } else {
                    b[14..].to_vec()                        // no VLAN: payload starts right after ethertype
                };
                let frame = OwnedFrame::new(
                    b[0..6].try_into().unwrap(),            // dst MAC
                    b[6..12].try_into().unwrap(),           // src MAC
                    et,
                    vid,
                    payload,
                );
                buf.push(frame);
            }
            if !buf.is_empty() {                            // keep looping until at least one frame was found
                break;
            }
        }
        self.tx_c.consume(buf.len()).unwrap();              // release those ring slots back for reuse
        self.rx_counter.fetch_add(buf.len(), Ordering::Relaxed); // bump this OuterPhy's "received" counter

        buf
    }

    pub fn recv_buffer_len(&self) -> usize {
        self.tx_c.consumable().count()                      // how many frames are currently waiting, unread
    }

    pub fn tx_count(&self) -> usize {
        self.tx_counter.load(Ordering::Relaxed)              // total frames sent through this port so far
    }

    pub fn rx_count(&self) -> usize {
        self.rx_counter.load(Ordering::Relaxed)              // total frames received through this port so far
    }
}