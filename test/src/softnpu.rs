
//TODO abstract away generated code from here
//use crate::hub::{ethernet_t, headers_t, EgressMetadata, IngressMetadata};
//use crate::router::{ipv6_t, ethernet_t, headers_t, EgressMetadata, IngressMetadata};
//use crate::router::{ipv6_t, ethernet_t, headers_t};
use crate::disag_router::{
    ipv6_t, ethernet_t, headers_t, sidecar_t, EgressMetadata, IngressMetadata
};

use p4rs::{bit, packet_in, Header};
use std::collections::HashMap;
use std::thread::spawn;
use xfr::{RingConsumer, RingProducer};
use bitvec::prelude::*;

/*
mod p4 {
    p4_macro::use_p4!("/Users/ry/src/p4/p4/examples/codegen/hub.p4");
}
*/

//p4_macro::use_p4!("p4/examples/codegen/softnpu.p4");

pub struct Phy<const R: usize, const N: usize, const F: usize> {
    index: usize,
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

pub fn run<'a, const R: usize, const N: usize, const F: usize>(
    ingress: &[RingConsumer<R, N, F>],
    egress: &[RingProducer<R, N, F>],
    local: &p4rs::table::Table<
        1usize,
        fn(&mut headers_t<'a>, &mut bool)
    >,
    router: &p4rs::table::Table<
        1usize,
        fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
    >,
    parse: fn(pkt: &mut packet_in<'a>, headers: &mut headers_t<'a>) -> bool,
    control: fn(
        hdr: &mut headers_t<'a>,
        ingress: &mut IngressMetadata,
        egress: &mut EgressMetadata,
        local: &p4rs::table::Table<
            1usize,
            fn(&mut headers_t<'a>, &mut bool)
        >,
        router: &p4rs::table::Table<
            1usize,
            fn(&mut headers_t<'a>, &mut IngressMetadata, &mut EgressMetadata),
        >,
    ),
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

                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                // XXX TODO XXX TODO XXX TODO XXX TODO XXX TODO XXX TODO XXX
                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                //
                // yes this is terrible, need some more lifetime gymnastics
                // to make this work propertly
                //
                let _content = unsafe {
                    std::slice::from_raw_parts_mut(
                        content.as_mut_ptr(),
                        content.len(),
                    )
                };
                //
                //
                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                // XXX TODO XXX TODO XXX TODO XXX TODO XXX TODO XXX TODO XXX
                // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!

                let mut pkt = packet_in::new(_content);

                // TODO these types are from the user p4 (not the device p4) and
                // should not be here, need more abstraction.
                let mut header = headers_t {
                    ethernet: ethernet_t::new(),
                    sidecar: sidecar_t::new(),
                    ipv6: ipv6_t::new(),
                };

                // assumes phys are ordered starting from 1
                let mut ingress_metadata = IngressMetadata {
                    //TODO more than u8::MAX ports
                    port: ((i+1) as u8).view_bits::<Lsb0>().to_bitvec(),
                };

                // to be filled in by pipeline
                let mut egress_metadata = EgressMetadata {
                    port: 0u8.view_bits::<Lsb0>().to_bitvec(),
                };

                println!("begin");

                // run the parser block
                let accept = parse(&mut pkt, &mut header);
                if !accept {
                    // drop the packet
                    println!("parser drop");
                    continue;
                }

                println!("parser accepted");

                // run the control block
                control(
                    &mut header,
                    &mut ingress_metadata,
                    &mut egress_metadata,
                    &local,
                    &router,
                );

                // write to egress port
                let port: usize = egress_metadata
                    .port
                    .as_raw_slice()[0] as usize;

                if port == 0 {
                    // indicates no table match
                    println!("no match");
                    continue
                }
                let eg = &egress[port - 1];
                let mut fps = eg.reserve(1).unwrap();
                let fp = fps.next().unwrap();

                //
                // emit headers
                //

                println!("control pass");

                let mut is_valid = false;
                let mut off = 0;
                let mut out = 0;
                let n = ethernet_t::size()>>3;
                if header.ethernet.is_valid() {
                    is_valid = true;
                    eg.write_at(fp, &content[0..n], out);
                    println!("ethernet {} -> {}", off, out);
                    out += n;
                    off += n;
                }
                let n = sidecar_t::size()>>3;
                if header.sidecar.is_valid() {
                    is_valid = true;
                    eg.write_at(fp, &content[off..off+n], out);
                    println!("sidecar {} -> {}", off, out);
                    out += n;
                    off += n;
                }
                let n = ipv6_t::size()>>3;
                if header.ipv6.is_valid() {
                    is_valid = true;
                    eg.write_at(fp, &content[off..off+n], out);
                    println!("ipv6 {} -> {}", off, out);
                    out += n;
                    off += n;
                }

                //
                // emit payload
                //

                if is_valid {
                    eg.write_at(fp, &content[off..], out);
                    println!("payload {} -> {}", off, out);
                }
                //eg.write(fps.next().unwrap(), content);
                egress_count[port - 1] += 1;
            }

            ig.consume(frames_in).unwrap();

            for (j, n) in egress_count.iter().enumerate() {
                egress[j].produce(*n).unwrap();
            }
        }
    }
}
