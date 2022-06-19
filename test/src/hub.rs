use crate::softnpu::{self, Frame, Phy};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use xfr::{ring, FrameBuffer};

p4_macro::use_p4!("p4/examples/codegen/hub.p4");

///
///                           ~~~~~~~~~~
///                           ~        ~
///                           ~   p4   ~
///                           ~        ~
///                           ~~~~~~~~~~
///                               |
///                               ▼
/// *=======*                *==========*                *=======*
/// |       | --- ( rx ) --> |          | <-- ( rx ) --- |       |
/// | phy 1 |                | pipeline |                | phy 2 |
/// |       | <-- ( tx ) --- |          | --- ( tx ) --> |       |
/// *=======*                *==========*                *=======*
///
///
#[test]
fn hub() -> Result<(), anyhow::Error> {
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

    // create phys
    let phy1 = Phy::new(1, rx1_p);
    let phy2 = Phy::new(2, rx2_p);

    // run phys
    phy1.run(tx1_c, phy1_egress);
    phy2.run(tx2_c, phy2_egress);

    // run the softnpu with the compiled p4 pipelines
    spawn(move || {
        let rx = &[rx1_c, rx2_c];
        let tx = &[tx1_p, tx2_p];
        softnpu::run(rx, tx, ingress_table_tbl(), parse_start, ingress_apply);
    });

    // shove some test data through the soft npu
    let mac1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    let mac2 = [0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];
    let ethertype = 0x86ed;
    let et = 0x86ed;

    phy1.write(&[Frame::new(mac2, mac1, et, b"do you know the muffin man?")])?;
    phy2.write(&[Frame::new(mac1, mac2, ethertype, b"the muffin man?")])?;
    phy1.write(&[Frame::new(mac2, mac1, ethertype, b"the muffin man!")])?;
    phy2.write(&[
        Frame::new(mac1, mac2, ethertype, b"why yes"),
        Frame::new(mac1, mac2, ethertype, b"i know the muffin man"),
        Frame::new(mac1, mac2, ethertype, b"the muffin man is me!!!"),
    ])?;

    sleep(Duration::from_secs(2));

    Ok(())
}

#[cfg(test)]
fn phy1_egress(frame: &[u8]) {
    println!("phy 1 !!! {}", String::from_utf8_lossy(&frame[14..]));
}

#[cfg(test)]
fn phy2_egress(frame: &[u8]) {
    println!("phy 2 !!! {}", String::from_utf8_lossy(&frame[14..]));
}
