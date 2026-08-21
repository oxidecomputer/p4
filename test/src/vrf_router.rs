use crate::softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{expect_frames, muffins};
use std::net::Ipv4Addr;

p4_macro::use_p4!(
    p4 = "test/src/p4/vrf_router.p4",
    pipeline_name = "vrf_router",
);

// A simple VRF router with composite (lpm, exact) key.
//
//   ~~~~~~~~~~
//   ~        ~                 *   *=======*
//   ~   p4   ~                 |   |       |
//   ~        ~                 |---| phy 0 |  vrf a
//   ~~~~~~~~~~                 |   |       |
//       |                      |   *=======*
//       |                      |   *=======*
//  *==========*                |   |       |
//  |          | <-- ( rx ) --- |---| phy 1 |  vrf b
//  | pipeline |                |   |       |
//  |          | --- ( tx ) --> |   *=======*
//  *==========*                |   *=======*
//                              |   |       |
//                              |---| phy 2 |
//                              |   |       |
//                              |   *=======*
//                              |   *=======*
//                              |   |       |
//                              |---| phy 3 |
//                              |   |       |
//                              *   *=======*
//

/// Return key bytes (prefix, length, exact)
fn vrf_key(prefix: Ipv4Addr, len: u8, port: u16) -> Vec<u8> {
    let mut buf = prefix.octets().to_vec();
    buf.push(len);
    buf.extend_from_slice(&port.to_le_bytes());

    buf
}

#[test]
fn vrf_router() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    //
    // add table entries
    //

    // vrf a (port 0): 10.1.0.0/16 -> port 2
    let buf = vrf_key("10.1.0.0".parse().unwrap(), 16, 0);
    pipeline.add_ingress_vrf_router_entry(
        "forward",
        &buf,
        &2u16.to_le_bytes(),
        0,
    );

    // vrf b (port 0): 10.1.0.0/16 -> port 3
    let buf = vrf_key("10.1.0.0".parse().unwrap(), 16, 1);
    pipeline.add_ingress_vrf_router_entry(
        "forward",
        &buf,
        &3u16.to_le_bytes(),
        0,
    );

    // vrf a (port 0): 10.1.1.0/24 -> port 1
    let buf = vrf_key("10.1.1.0".parse().unwrap(), 24, 0);
    pipeline.add_ingress_vrf_router_entry(
        "forward",
        &buf,
        &1u16.to_le_bytes(),
        0,
    );

    //
    // run program
    //

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    let if0 = Interface4::new(phy0.clone(), "1.0.0.1".parse().unwrap());
    let if1 = Interface4::new(phy1.clone(), "1.0.0.2".parse().unwrap());
    let if2 = Interface4::new(phy2.clone(), "1.0.0.3".parse().unwrap());

    npu.run();

    let et = 0x0800;
    let msg = muffins!();

    // Each VRF routes this destination differently
    if0.send(phy2.mac, "10.1.47.1".parse().unwrap(), msg.0)?;
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, et, msg.0)]);
    if1.send(phy3.mac, "10.1.47.1".parse().unwrap(), msg.1)?;
    expect_frames!(phy3, &[RxFrame::new(phy1.mac, et, msg.1)]);

    // More specific match for vrf a
    if0.send(phy1.mac, "10.1.1.1".parse().unwrap(), msg.2)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, et, msg.2)]);

    // No /24 for vrf b
    if1.send(phy3.mac, "10.1.1.1".parse().unwrap(), msg.3)?;
    expect_frames!(phy3, &[RxFrame::new(phy1.mac, et, msg.3)]);

    // No match: should drop coming from port 2. Port 0 should go through.
    if2.send(phy2.mac, "10.1.47.1".parse().unwrap(), msg.4)?;
    if0.send(phy2.mac, "10.1.47.1".parse().unwrap(), msg.5)?;
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, et, msg.5)]);

    Ok(())
}
