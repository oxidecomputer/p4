use std::sync::Arc;
use std::thread::{spawn, sleep};
use std::time::Duration;
use crate::softnpu::Phy;
use xfr::{FrameBuffer, ring};

p4_macro::use_p4!("/Users/ry/src/p4/p4/examples/codegen/hub.p4");

#[test]
fn hub() {

    const R: usize = 1024;
    const N: usize = 4096;
    const F: usize = 1500;

    let fb = Arc::new(FrameBuffer::<N, F>::new());

    // ingress rings
    let (rx1_p, rx1_c) = ring::<R, N, F>(fb.clone());
    let (rx2_p, rx2_c) = ring::<R, N, F>(fb.clone());

    // egress rings
    let (tx1_p, tx1_c) = ring::<R, N, F>(fb.clone());
    let (tx2_p, tx2_c) = ring::<R, N, F>(fb.clone());
    

    let phy1 = Phy::new(1, rx1_p, /*parse_start, ingress_apply, &tbl*/);
    let phy2 = Phy::new(2, rx2_p, /*parse_start, ingress_apply, &tbl*/);

    phy1.run(tx1_c, phy1_egress);
    phy2.run(tx2_c, phy2_egress);

    phy1.write(&[b"do you know the muffin man?"]).unwrap();
    phy2.write(&[b"the muffin man?"]).unwrap();
    phy1.write(&[b"the muffin man!"]).unwrap();
    phy2.write(&[
        b"why yes",
        b"i know the muffin man",
        b"the muffin man is me!!!",
    ]).unwrap();

    spawn(move || {

        let tbl = ingress_table_tbl();

        loop {

            let mut frames_phy1 = 0;
            let mut frames_phy2 = 0;

            let mut consume = 0;

            // handle packets from phy1
            for fp in rx1_c.consumable() {

                consume += 1;

                let mut content = rx1_c.read_mut(fp);
                let mut pkt = packet_in::new(&mut content);

                let mut header = headers_t{
                    ethernet: ethernet_t::new()
                };

                // TODO parse packets, need actual ethernet frames, more than
                // muffins.

                let mut ingress_metadata = IngressMetadata{
                    port: bit::<8>::from(1),
                };
                let mut egress_metadata = EgressMetadata{
                    port: bit::<8>::from(0),
                };

                ingress_apply(&mut header, &mut ingress_metadata, &mut egress_metadata, &tbl);

                if egress_metadata.port == bit::<8>::from(1) {
                    let mut fps = tx1_p.reserve(1).unwrap();
                    tx1_p.write(fps.next().unwrap(), content);
                    frames_phy1 += 1;
                }

                if egress_metadata.port == bit::<8>::from(2) {
                    let mut fps = tx2_p.reserve(1).unwrap();
                    tx2_p.write(fps.next().unwrap(), content);
                    frames_phy2 += 1;
                }

            }

            rx1_c.consume(consume);
            consume = 0;

            if frames_phy1 > 0 {
                tx1_p.produce(frames_phy1);
                frames_phy1 = 0;
            }

            if frames_phy2 > 0 {
                tx2_p.produce(frames_phy2);
                frames_phy2 = 0;
            }

            // handle packets from phy2
            for fp in rx2_c.consumable() {

                consume += 1;

                //XXX copy pasta from rx1 iterator
                let mut content = rx1_c.read_mut(fp);
                let mut pkt = packet_in::new(&mut content);

                let mut header = headers_t{
                    ethernet: ethernet_t::new()
                };

                // TODO parse packets, need actual ethernet frames, more than
                // muffins.

                let mut ingress_metadata = IngressMetadata{
                    port: bit::<8>::from(2),
                };
                let mut egress_metadata = EgressMetadata{
                    port: bit::<8>::from(0),
                };

                ingress_apply(&mut header, &mut ingress_metadata, &mut egress_metadata, &tbl);

                if egress_metadata.port == bit::<8>::from(1) {
                    let mut fps = tx1_p.reserve(1).unwrap();
                    tx1_p.write(fps.next().unwrap(), content);
                    frames_phy1 += 1;
                }

                if egress_metadata.port == bit::<8>::from(2) {
                    let mut fps = tx2_p.reserve(1).unwrap();
                    tx2_p.write(fps.next().unwrap(), content);
                    frames_phy2 += 1;
                }
            }

            rx2_c.consume(consume);
            consume = 0;

            if frames_phy1 > 0 {
                tx1_p.produce(frames_phy1);
                frames_phy1 = 0;
            }

            if frames_phy2 > 0 {
                tx2_p.produce(frames_phy2);
                frames_phy2 = 0;
            }
        }

    });

    sleep(Duration::from_secs(2));

}

#[cfg(test)]
fn phy1_egress(frame: &[u8]) {
    println!("phy 1 !!! {}", std::str::from_utf8(frame).unwrap());
}

#[cfg(test)]
fn phy2_egress(frame: &[u8]) {
    println!("phy 2 !!! {}", std::str::from_utf8(frame).unwrap());
}
