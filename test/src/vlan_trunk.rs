use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};

p4_macro::use_p4!(
    p4 = "test/src/p4/vlan_trunk.p4",
    pipeline_name = "vlan_trunk",
);

//
//   ~~~~~~~~~~
//   ~        ~                 *   *=======*
//   ~   p4   ~                 |   |       |
//   ~        ~                 |---| phy 0 |  ingress
//   ~~~~~~~~~~                 |   |       |
//       |                      |   *=======*
//       |                      |   *=======*
//  *==========*                |   |       |
//  |          | <-- ( rx ) --- |---| phy 1 |  ingress
//  | pipeline |                |   |       |
//  |          | --- ( tx ) --> |   *=======*
//  *==========*                |   *=======*
//                              |   |       |
//                              |---| phy 2 |  uplink
//                              |   |       |
//                              |   *=======*
//                              |   *=======*
//                              |   |       |
//                              |---| phy 3 |  uplink
//                              |   |       |
//                              *   *=======*
//

/// Return key bytes (range, exact)
fn trunk_key(begin: u16, end: u16, port: u16) -> Vec<u8> {
    let mut buf = begin.to_le_bytes().to_vec();
    buf.extend_from_slice(&end.to_le_bytes());
    buf.extend_from_slice(&port.to_le_bytes());

    buf
}

#[test]
fn vlan_trunk() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    //
    // add table entries
    //

    // port 0: [100, 199] -> port 2
    let buf = trunk_key(100, 199, 0);
    pipeline.add_ingress_trunk_entry("forward", &buf, &2u16.to_le_bytes(), 0);

    // port 1: [100, 199] -> port 3
    let buf = trunk_key(100, 199, 1);
    pipeline.add_ingress_trunk_entry("forward", &buf, &3u16.to_le_bytes(), 0);

    // port 0: [300, 350] -> port 3
    let buf = trunk_key(300, 350, 0);
    pipeline.add_ingress_trunk_entry("forward", &buf, &3u16.to_le_bytes(), 0);

    //
    // run program
    //

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    npu.run();

    let et = 0;
    let msg = muffins!();

    // Different redirects for each port
    phy0.send(&[TxFrame::newv(phy2.mac, et, msg.0, 150)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy0.mac, et, msg.0, 150)]);
    phy1.send(&[TxFrame::newv(phy3.mac, et, msg.1, 150)])?;
    expect_frames!(phy3, &[RxFrame::newv(phy1.mac, et, msg.1, 150)]);

    // Range bounds are inclusive
    phy0.send(&[TxFrame::newv(phy2.mac, et, msg.2, 100)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy0.mac, et, msg.2, 100)]);
    phy0.send(&[TxFrame::newv(phy2.mac, et, msg.3, 199)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy0.mac, et, msg.3, 199)]);

    // Testing [300, 350] block for port 0
    phy0.send(&[TxFrame::newv(phy3.mac, et, msg.4, 320)])?;
    expect_frames!(phy3, &[RxFrame::newv(phy0.mac, et, msg.4, 320)]);

    // No match -> should drop
    phy0.send(&[TxFrame::newv(phy2.mac, et, b"dropped muffin", 250)])?;
    phy1.send(&[TxFrame::newv(phy3.mac, et, b"lost muffin", 320)])?;
    phy2.send(&[TxFrame::newv(phy2.mac, et, b"stray muffin", 150)])?;

    // Push a good one through
    phy0.send(&[TxFrame::newv(phy2.mac, et, msg.5, 150)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy0.mac, et, msg.5, 150)]);
    Ok(())
}
