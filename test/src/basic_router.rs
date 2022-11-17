use crate::softnpu::{Interface6, RxFrame, SoftNpu};
use crate::{expect_frames, muffins};

p4_macro::use_p4!(
    p4 = "p4/examples/codegen/router.p4",
    pipeline_name = "basic_router",
);

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
fn basic_router2() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(2, main_pipeline::new(), false);
    let phy1 = npu.phy(0);
    let phy2 = npu.phy(1);

    let if1 = Interface6::new(phy1.clone(), "fd00:1000::1".parse().unwrap());
    let if2 = Interface6::new(phy2.clone(), "fd00:2000::1".parse().unwrap());

    npu.run();

    let et = 0x86dd;
    let msg = muffins!();

    if1.send(phy2.mac, if2.addr, msg.0)?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, et, msg.0)]);

    if2.send(phy1.mac, if1.addr, msg.1)?;
    expect_frames!(phy1, &[RxFrame::new(phy2.mac, et, msg.1)]);

    if1.send(phy2.mac, if2.addr, msg.2)?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, et, msg.2)]);

    if2.send(phy1.mac, if1.addr, msg.3)?;
    if2.send(phy1.mac, if1.addr, msg.4)?;
    if2.send(phy1.mac, if1.addr, msg.5)?;
    expect_frames!(
        phy1,
        &[
            RxFrame::new(phy2.mac, et, msg.3),
            RxFrame::new(phy2.mac, et, msg.4),
            RxFrame::new(phy2.mac, et, msg.5),
        ]
    );

    assert_eq!(phy1.tx_count(), 2usize);
    assert_eq!(phy1.rx_count(), 4usize);

    assert_eq!(phy2.tx_count(), 4usize);
    assert_eq!(phy2.rx_count(), 2usize);

    Ok(())
}
