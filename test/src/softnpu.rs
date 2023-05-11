use crate::packet;
use colored::Colorize;
use p4rs::packet_in;
use rand::Rng;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::spawn;
use xfr::{ring, FrameBuffer, RingConsumer, RingProducer};

pub fn do_expect_frames(
    name: &str,
    phy: &Arc<OuterPhy<RING, FBUF, MTU>>,
    expected: &[RxFrame],
    dmac: Option<[u8; 6]>,
) {
    let n = expected.len();
    let mut frames = Vec::new();
    loop {
        let fs = phy.recv();
        frames.extend_from_slice(&fs);
        // TODO this is not a great interface, if frames.len() > n, we should do
        // something besides hang forever.
        if frames.len() == n {
            break;
        }
    }
    for i in 0..n {
        let payload = match frames[i].ethertype {
            0x0901 => {
                let mut payload = &frames[i].payload[..];
                let et = u16::from_be_bytes([payload[5], payload[6]]);
                payload = &payload[23..];
                if et == 0x86dd {
                    payload = &payload[40..];
                }
                if et == 0x0800 {
                    payload = &payload[20..];
                }
                payload
            }
            0x86dd => &frames[i].payload[40..],
            0x0800 => &frames[i].payload[20..],
            _ => &frames[i].payload[..],
        };
        let m = String::from_utf8_lossy(payload).to_string();
        println!("[{}] {}", name.magenta(), m.dimmed());
        assert_eq!(frames[i].src, expected[i].src, "src");
        if let Some(d) = dmac {
            assert_eq!(frames[i].dst, d, "dst");
        }
        assert_eq!(frames[i].ethertype, expected[i].ethertype, "ethertype");
        assert_eq!(payload, expected[i].payload, "payload");
    }
}

#[macro_export]
macro_rules! expect_frames {
    ($phy:expr, $expected:expr) => {
        $crate::softnpu::do_expect_frames(
            stringify!($phy),
            &$phy,
            $expected,
            None,
        )
    };
    ($phy:expr, $expected:expr, $dmac:expr) => {
        $crate::softnpu::do_expect_frames(
            stringify!($phy),
            &$phy,
            $expected,
            Some($dmac),
        )
    };
}

const RING: usize = 1024;
const FBUF: usize = 4096;
const MTU: usize = 1500;

pub struct SoftNpu<P: p4rs::Pipeline> {
    pub pipeline: Option<P>,
    inner_phys: Option<Vec<InnerPhy<RING, FBUF, MTU>>>,
    outer_phys: Vec<Arc<OuterPhy<RING, FBUF, MTU>>>,
    _fb: Arc<FrameBuffer<FBUF, MTU>>,
}

impl<P: p4rs::Pipeline + 'static> SoftNpu<P> {
    /// Create a new SoftNpu ASIC emulator. The `radix` indicates the number of
    /// ports. The `pipeline` is the `x4c` compiled program that the ASIC will
    /// run. When `cpu_port` is set to true, sidecar data in `TxFrame` elements
    /// will be added to packets sent through port 0 (as a sidecar header) on
    /// the way to the ASIC.
    pub fn new(radix: usize, pipeline: P, cpu_port: bool) -> Self {
        let fb = Arc::new(FrameBuffer::<FBUF, MTU>::new());
        let mut inner_phys = Vec::new();
        let mut outer_phys = Vec::new();
        for i in 0..radix {
            let (rx_p, rx_c) = ring::<RING, FBUF, MTU>(fb.clone());
            let (tx_p, tx_c) = ring::<RING, FBUF, MTU>(fb.clone());
            let inner_phy = InnerPhy::new(i, rx_c, tx_p);
            let mut outer_phy = OuterPhy::new(i, rx_p, tx_c);
            inner_phys.push(inner_phy);
            if i == 0 && cpu_port {
                outer_phy.sidecar_encap = true;
            }
            outer_phys.push(Arc::new(outer_phy));
        }
        let inner_phys = Some(inner_phys);
        SoftNpu {
            inner_phys,
            outer_phys,
            _fb: fb,
            pipeline: Some(pipeline),
        }
    }

    pub fn run(&mut self) {
        let inner_phys = match self.inner_phys.take() {
            Some(phys) => phys,
            None => panic!("phys already in use"),
        };
        let pipe = match self.pipeline.take() {
            Some(pipe) => pipe,
            None => panic!("pipe already in use"),
        };
        spawn(move || {
            Self::do_run(inner_phys, pipe);
        });
    }

    fn do_run(inner_phys: Vec<InnerPhy<RING, FBUF, MTU>>, mut pipeline: P) {
        loop {
            // TODO: yes this is a highly suboptimal linear gather-scatter across
            // each ingress. Will update to something more concurrent eventually.
            for (i, ig) in inner_phys.iter().enumerate() {
                // keep track of how many frames we've produced for each egress
                let mut egress_count = vec![0; inner_phys.len()];

                // keep track of how many frames we've consumed for this ingress
                let mut frames_in = 0;

                for fp in ig.rx_c.consumable() {
                    frames_in += 1;
                    let content = ig.rx_c.read_mut(fp);

                    let mut pkt = packet_in::new(content);

                    let port = i as u16;
                    let output = pipeline.process_packet(port, &mut pkt);
                    for (out_pkt, out_port) in &output {
                        let out_port = *out_port as usize;
                        //
                        // get frame for packet
                        //

                        let phy = &inner_phys[out_port];
                        let eg = &phy.tx_p;
                        let mut fps = eg.reserve(1).unwrap();
                        let fp = fps.next().unwrap();

                        //
                        // emit headers
                        //

                        eg.write_at(fp, out_pkt.header_data.as_slice(), 0);

                        //
                        // emit payload
                        //

                        eg.write_at(
                            fp,
                            out_pkt.payload_data,
                            out_pkt.header_data.len(),
                        );

                        egress_count[out_port] += 1;
                    }
                }
                ig.rx_c.consume(frames_in).unwrap();
                ig.rx_counter.fetch_add(frames_in, Ordering::Relaxed);

                for (j, n) in egress_count.iter().enumerate() {
                    if *n == 0 {
                        continue;
                    }
                    let phy = &inner_phys[j];
                    phy.tx_p.produce(*n).unwrap();
                    phy.tx_counter.fetch_add(*n, Ordering::Relaxed);
                }
            }
        }
    }

    pub fn phy(&self, i: usize) -> Arc<OuterPhy<RING, FBUF, MTU>> {
        self.outer_phys[i].clone()
    }
}

pub struct InnerPhy<const R: usize, const N: usize, const F: usize> {
    pub index: usize,
    rx_c: RingConsumer<R, N, F>,
    tx_p: RingProducer<R, N, F>,
    tx_counter: AtomicUsize,
    rx_counter: AtomicUsize,
}

pub struct OuterPhy<const R: usize, const N: usize, const F: usize> {
    pub index: usize,
    pub mac: [u8; 6],
    rx_p: RingProducer<R, N, F>,
    tx_c: RingConsumer<R, N, F>,
    tx_counter: AtomicUsize,
    rx_counter: AtomicUsize,
    sidecar_encap: bool,
}

pub struct Interface6<const R: usize, const N: usize, const F: usize> {
    pub phy: Arc<OuterPhy<R, N, F>>,
    pub addr: Ipv6Addr,
    pub sc_egress: u16,
}

impl<const R: usize, const N: usize, const F: usize> Interface6<R, N, F> {
    pub fn new(phy: Arc<OuterPhy<R, N, F>>, addr: Ipv6Addr) -> Self {
        Self {
            phy,
            addr,
            sc_egress: 0,
        }
    }

    pub fn send(
        &self,
        mac: [u8; 6],
        ip: Ipv6Addr,
        payload: &[u8],
    ) -> Result<(), anyhow::Error> {
        let n = 40 + payload.len();
        let mut buf = [0u8; F];
        packet::v6(self.addr, ip, payload, &mut buf);
        let mut txf = TxFrame::new(mac, 0x86dd, &buf[..n]);
        txf.sc_egress = self.sc_egress;
        self.phy.send(&[txf])?;
        Ok(())
    }
}

pub struct Interface4<const R: usize, const N: usize, const F: usize> {
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

    pub fn send(
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
        Self {
            src,
            ethertype,
            payload,
            vid: None,
        }
    }
    pub fn newv(
        src: [u8; 6],
        ethertype: u16,
        payload: &'a [u8],
        vid: u16,
    ) -> Self {
        Self {
            src,
            ethertype,
            payload,
            vid: Some(vid),
        }
    }
}

impl<'a> TxFrame<'a> {
    pub fn new(dst: [u8; 6], ethertype: u16, payload: &'a [u8]) -> Self {
        Self {
            dst,
            ethertype,
            payload,
            sc_egress: 0,
            vid: None,
        }
    }

    pub fn newv(
        dst: [u8; 6],
        ethertype: u16,
        payload: &'a [u8],
        vid: u16,
    ) -> Self {
        Self {
            dst,
            ethertype,
            payload,
            sc_egress: 0,
            vid: Some(vid),
        }
    }
}

#[derive(Clone)]
pub struct OwnedFrame {
    pub dst: [u8; 6],
    pub src: [u8; 6],
    pub vid: Option<u16>,
    pub ethertype: u16,
    pub payload: Vec<u8>,
}

impl OwnedFrame {
    pub fn new(
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
            tx_counter: AtomicUsize::new(0),
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
        let mut rng = rand::thread_rng();
        let m = rng.gen_range::<u32, _>(0xf00000..0xffffff).to_le_bytes();
        let mac = [0xa8, 0x40, 0x25, m[0], m[1], m[2]];

        Self {
            index,
            rx_p,
            tx_c,
            mac,
            tx_counter: AtomicUsize::new(0),
            rx_counter: AtomicUsize::new(0),
            sidecar_encap: false,
        }
    }

    pub fn send(&self, frames: &[TxFrame<'_>]) -> Result<(), xfr::Error> {
        let n = frames.len();
        let fps = self.rx_p.reserve(n)?;
        for (i, fp) in fps.enumerate() {
            let f = &frames[i];
            self.rx_p.write_at(fp, f.dst.as_slice(), 0);
            self.rx_p.write_at(fp, &self.mac, 6);
            let mut off = 12;
            if self.sidecar_encap {
                self.rx_p
                    .write_at(fp, 0x0901u16.to_be_bytes().as_slice(), off);
                off += 2;
                // sc_code = SC_FWD_FROM_USERSPACE
                self.rx_p.write_at(fp, &[0u8], off);
                off += 1;
                // sc_ingress
                let ingress = f.sc_egress;
                self.rx_p
                    .write_at(fp, ingress.to_be_bytes().as_slice(), off);
                off += 2;
                // sc_egress
                let egress = f.sc_egress;
                self.rx_p.write_at(fp, egress.to_be_bytes().as_slice(), off);
                off += 2;
                // sc_ether_type
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),
                    off,
                );
                off += 2;
                // sc_payload
                self.rx_p.write_at(fp, [0u8; 16].as_slice(), off);
                off += 16;
            } else if let Some(vid) = f.vid {
                self.rx_p
                    .write_at(fp, 0x8100u16.to_be_bytes().as_slice(), off);
                off += 2;
                self.rx_p.write_at(fp, vid.to_be_bytes().as_slice(), off);
                off += 2;
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),
                    off,
                );
                off += 2;
            } else {
                self.rx_p.write_at(
                    fp,
                    f.ethertype.to_be_bytes().as_slice(),
                    off,
                );
                off += 2;
            }

            self.rx_p.write_at(fp, f.payload, off);
        }
        self.rx_p.produce(n)?;
        self.tx_counter.fetch_add(n, Ordering::Relaxed);
        Ok(())
    }

    pub fn recv(&self) -> Vec<OwnedFrame> {
        let mut buf = Vec::new();
        loop {
            for fp in self.tx_c.consumable() {
                let b = self.tx_c.read(fp);
                let mut et = u16::from_be_bytes([b[12], b[13]]);
                let mut vid: Option<u16> = None;
                let payload = if et == 0x8100 {
                    let v = u16::from_be_bytes([b[14], b[15]]);
                    et = u16::from_be_bytes([b[16], b[17]]);
                    vid = Some(v);
                    b[18..].to_vec()
                } else {
                    b[14..].to_vec()
                };
                let frame = OwnedFrame::new(
                    b[0..6].try_into().unwrap(),
                    b[6..12].try_into().unwrap(),
                    et,
                    vid,
                    payload,
                );
                buf.push(frame);
            }
            if !buf.is_empty() {
                break;
            }
        }
        self.tx_c.consume(buf.len()).unwrap();
        self.rx_counter.fetch_add(buf.len(), Ordering::Relaxed);

        buf
    }

    pub fn recv_buffer_len(&self) -> usize {
        self.tx_c.consumable().count()
    }

    pub fn tx_count(&self) -> usize {
        self.tx_counter.load(Ordering::Relaxed)
    }

    pub fn rx_count(&self) -> usize {
        self.rx_counter.load(Ordering::Relaxed)
    }
}
