use p4rs::packet_in;
use std::thread::spawn;
use xfr::{RingConsumer, RingProducer};

pub struct Phy<const R: usize, const N: usize, const F: usize> {
    pub index: usize,
    ingress: RingProducer<R, N, F>,
}

pub struct Frame<'a> {
    pub dst: [u8; 6],
    pub src: [u8; 6],
    pub ethertype: u16,
    pub payload: &'a [u8],
}

impl<'a> Frame<'a> {
    pub fn new(
        dst: [u8; 6],
        src: [u8; 6],
        ethertype: u16,
        payload: &'a [u8],
    ) -> Self {
        Self {
            dst,
            src,
            ethertype,
            payload,
        }
    }
}

impl<const R: usize, const N: usize, const F: usize> Phy<R, N, F> {
    pub fn new(index: usize, ingress: RingProducer<R, N, F>) -> Self {
        Self { index, ingress }
    }

    pub fn write<'a>(&self, frames: &[Frame<'a>]) -> Result<(), xfr::Error> {
        let n = frames.len();
        let fps = self.ingress.reserve(n)?;
        for (i, fp) in fps.enumerate() {
            let f = &frames[i];
            self.ingress.write_at(fp, f.dst.as_slice(), 0);
            self.ingress.write_at(fp, f.src.as_slice(), 6);
            self.ingress
                .write_at(fp, f.ethertype.to_be_bytes().as_slice(), 12);
            self.ingress.write_at(fp, f.payload, 14);
        }
        self.ingress.produce(n)
    }

    pub fn run(
        &self,
        egress: RingConsumer<R, N, F>,
        egress_handler: fn(frame: &[u8]),
    ) {
        spawn(move || loop {
            let mut count = 0;
            for fp in egress.consumable() {
                let content = egress.read(fp);
                egress_handler(content);
                count += 1;
            }
            egress.consume(count).unwrap();
        });
    }
}

pub fn run_pipeline<
    P: p4rs::Pipeline,
    const R: usize,
    const N: usize,
    const F: usize,
>(
    ingress: &[RingConsumer<R, N, F>],
    egress: &[RingProducer<R, N, F>],
    pipeline: &mut P,
) {
    loop {
        // TODO: yes this is a highly suboptimal linear gather-scatter across
        // each ingress. Will update to something more concurrent eventually.
        for (i, ig) in ingress.iter().enumerate() {
            // keep track of how many frames we've produced for each egress
            let mut egress_count = vec![0; egress.len()];

            // keep track of how many frames we've consumed for this ingress
            let mut frames_in = 0;

            for fp in ig.consumable() {
                frames_in += 1;
                let content = ig.read_mut(fp);

                let mut pkt = packet_in::new(content);

                match pipeline.process_packet(i as u8, &mut pkt) {
                    Some((out_pkt, port)) => {
                        let port = port as usize;

                        //
                        // get frame for packet
                        //

                        let eg = &egress[port];
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

                        egress_count[port] += 1;
                    }
                    None => {}
                }
            }
            ig.consume(frames_in).unwrap();

            for (j, n) in egress_count.iter().enumerate() {
                egress[j].produce(*n).unwrap();
            }
        }
    }
}
