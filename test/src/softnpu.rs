use xfr::{RingProducer, RingConsumer};
use p4rs::{bit, packet_in};
use crate::hub::{IngressMetadata, EgressMetadata, headers_t};
use std::thread::spawn;

pub struct Phy<const R: usize, const N: usize, const F: usize>  {
    index: usize,
    ingress: RingProducer::<R, N, F>,
    /*
    parser: fn(pkt: &mut packet_in<'a>, headers: &mut headers_t<'a>) -> bool,
    control: fn(
        hdr: &mut headers_t<'a>,
        ingress: &mut IngressMetadata,
        egress: &mut EgressMetadata,
        tbl: &std::collections::HashMap::<
            bit::<8usize>,
            &'a dyn Fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
        >,
    ),
    tbl: &'a std::collections::HashMap::<
        bit::<8usize>,
        &'a dyn Fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
    >,
    */
}


impl<const R: usize, const N: usize, const F: usize> Phy<R, N, F> {
    pub fn new(
        index: usize,
        ingress: RingProducer::<R, N, F>,
        /*
        parser: fn(pkt: &mut packet_in<'a>, headers: &mut headers_t<'a>) -> bool,
        control: fn(
            hdr: &mut headers_t<'a>,
            ingress: &mut IngressMetadata,
            egress: &mut EgressMetadata,
            tbl: &std::collections::HashMap::<
                bit::<8usize>,
                &'a dyn Fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
            >,
        ),
        tbl: &'a std::collections::HashMap::<
            bit::<8usize>,
            &'a dyn Fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
        >,
        */
    ) -> Self {
        Self {
            index,
            ingress,
            /*
            parser,
            control,
            tbl,
            */
        }
    }

    pub fn write(&self, frames: &[&[u8]]) -> Result<(), xfr::Error> {
        let n = frames.len();
        let fps = self.ingress.reserve(n)?;
        for (i, fp) in fps.enumerate() {
            self.ingress.write(fp, frames[i]);
        }
        self.ingress.produce(n)
    }

    pub fn run(
        &self,
        egress: RingConsumer::<R, N, F>,
        egress_handler: fn(frame: &[u8]),
    ) {

        spawn(move || {

            loop {

                let mut count = 0;
                for fp in egress.consumable() {
                    let content = egress.read(fp);
                    egress_handler(content);
                    count += 1;
                }
                egress.consume(count).unwrap();

            }
        });
    }
}
