use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};

p4_macro::use_p4!(p4 = "test/src/p4/hub.p4", pipeline_name = "hub2");

///
///                           ~~~~~~~~~~
///                           ~        ~
///                           ~   p4   ~
///                           ~        ~
///                           ~~~~~~~~~~
///                               |
///                               |
/// *=======*                *==========*                *=======*
/// |       | --- ( rx ) --> |          | <-- ( rx ) --- |       |
/// | phy 1 |                | pipeline |                | phy 3 |
/// |       | <-- ( tx ) --- |          | --- ( tx ) --> |       |
/// *=======*                *==========*                *=======*
///                           tx |  |
///                              |  |   
///                              |  | rx
///                           *========*
///                           |        |
///                           |        |
///                           |  phy2  |
///                           *========*
///
#[test]
fn hub2() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(3, main_pipeline::new(3), false);
    let phy1 = npu.phy(0);
    let phy2 = npu.phy(1);
    let phy3 = npu.phy(2);

    npu.run();

    let et = 0;
    let msg = muffins!();

    phy1.send(&[TxFrame::new(phy2.mac, et, msg.0)])?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, et, msg.0)]);
    expect_frames!(phy3, &[RxFrame::new(phy1.mac, et, msg.0)]);

    phy2.send(&[TxFrame::new(phy1.mac, et, msg.1)])?;
    expect_frames!(phy1, &[RxFrame::new(phy2.mac, et, msg.1)]);
    expect_frames!(phy3, &[RxFrame::new(phy2.mac, et, msg.1)]);

    phy1.send(&[TxFrame::new(phy2.mac, et, msg.2)])?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, et, msg.2)]);
    expect_frames!(phy3, &[RxFrame::new(phy1.mac, et, msg.2)]);

    phy2.send(&[TxFrame::new(phy1.mac, et, msg.3)])?;
    phy2.send(&[TxFrame::new(phy1.mac, et, msg.4)])?;
    phy2.send(&[TxFrame::new(phy1.mac, et, msg.5)])?;
    expect_frames!(
        phy1,
        &[
            RxFrame::new(phy2.mac, et, msg.3),
            RxFrame::new(phy2.mac, et, msg.4),
            RxFrame::new(phy2.mac, et, msg.5),
        ]
    );
    expect_frames!(
        phy3,
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

    assert_eq!(phy3.tx_count(), 0usize);
    assert_eq!(phy3.rx_count(), 6usize);

    Ok(())
}
