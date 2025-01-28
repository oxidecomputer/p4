#![allow(clippy::needless_update)]
use tests::expect_frames;
use tests::softnpu::{RxFrame, SoftNpu, TxFrame};

const NUM_PORTS: u16 = 3;

p4_macro::use_p4!(
    p4 = "book/code/src/bin/hello-world.p4",
    pipeline_name = "hello"
);

fn main() -> Result<(), anyhow::Error> {
    let mut npu =
        SoftNpu::new(NUM_PORTS.into(), main_pipeline::new(NUM_PORTS), false);
    let phy1 = npu.phy(0);
    let phy2 = npu.phy(1);
    let phy3 = npu.phy(2);

    npu.run();

    // Expect this packet to be dropped
    phy3.send(&[TxFrame::new(phy3.mac, 0, b"to the bit bucket with you!")])?;

    phy1.send(&[TxFrame::new(phy2.mac, 0, b"hello")])?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0, b"hello")]);

    phy2.send(&[TxFrame::new(phy1.mac, 0, b"world")])?;
    expect_frames!(phy1, &[RxFrame::new(phy2.mac, 0, b"world")]);

    Ok(())
}
