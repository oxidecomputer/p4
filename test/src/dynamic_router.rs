use crate::softnpu::{Interface6, RxFrame, SoftNpu};
use crate::{expect_frames, muffins};
use std::net::Ipv6Addr;

p4_macro::use_p4!(
    p4 = "test/src/p4/dynamic_router.p4",
    pipeline_name = "dynamic",
);

/// This test is the same as the disag router test, except table entries are
/// added dynamically instead of statically defined in the P4 program.
///
///   ~~~~~~~~~~                 
///   ~        ~                 
///   ~   p4   ~                 *   *=======*
///   ~        ~                 |   |       |
///   ~~~~~~~~~~                 |---| phy 1 |
///       |                      |   |       |
///       ▼                      |   *=======*
///  *==========*                |   *=======*
///  |          | <-- ( rx ) --- |   |       |
///  | pipeline |                |---| phy 2 |
///  |          | --- ( tx ) --> |   |       |
///  *==========*                |   *=======*
///   tx |  ▲                    |   *=======*
///      |  |                    |   |       |
///      ▼  | rx                 |---| phy 3 |
///   *========*                 |   |       |
///   |        |                 *   *=======*
///   |        |
///   |  CPU   |
///   *========*
///

#[test]
fn dynamic_router2() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    //
    // add table entries
    //

    let prefix: Ipv6Addr = "fd00:1000::".parse().unwrap();
    let mut buf = prefix.octets().to_vec();
    buf.push(24); // prefix length

    pipeline.add_ingress_router_router_entry(
        "forward",
        &buf,
        &1u16.to_le_bytes(),
    );

    let prefix: Ipv6Addr = "fd00:2000::".parse().unwrap();
    let mut buf = prefix.octets().to_vec();
    buf.push(24); // prefix length

    pipeline.add_ingress_router_router_entry(
        "forward",
        &buf,
        &2u16.to_le_bytes(),
    );

    let prefix: Ipv6Addr = "fd00:3000::".parse().unwrap();
    let mut buf = prefix.octets().to_vec();
    buf.push(24); // prefix length

    pipeline.add_ingress_router_router_entry(
        "forward",
        &buf,
        &3u16.to_le_bytes(),
    );

    //
    // run program
    //

    let mut npu = SoftNpu::new(4, pipeline, true);
    let cpu = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    let if1 = Interface6::new(phy1.clone(), "fd00:1000::1".parse().unwrap());
    let if2 = Interface6::new(phy2.clone(), "fd00:2000::1".parse().unwrap());
    let mut if3 = Interface6::new(
        cpu.clone(),
        "fe80::aae1:deff:fe01:701c".parse().unwrap(),
    );
    if3.sc_egress = 1;
    let mut if4 = Interface6::new(
        cpu.clone(),
        "fe80::aae1:deff:fe01:701d".parse().unwrap(),
    );
    if4.sc_egress = 3;
    let mc1: Ipv6Addr = "ff02::1:ff01:701c".parse().unwrap();

    npu.run();

    let msg = muffins!();

    if1.send(phy2.mac, if2.addr, msg.0)?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0x86dd, msg.0)]);

    // multicast should go to the CPU port
    if2.send(phy1.mac, mc1, msg.1)?;
    expect_frames!(cpu, &[RxFrame::new(phy2.mac, 0x0901, msg.1)]);

    // link-local should go to the CPU port
    if1.send(phy2.mac, if3.addr, msg.2)?;
    expect_frames!(cpu, &[RxFrame::new(phy1.mac, 0x0901, msg.2)]);

    // link-local should go to the CPU port
    if2.send(phy1.mac, if4.addr, msg.3)?;
    expect_frames!(cpu, &[RxFrame::new(phy2.mac, 0x0901, msg.3)]);

    // from the CPU port to phy1
    if3.send(phy1.mac, if1.addr, msg.4)?;
    expect_frames!(phy1, &[RxFrame::new(cpu.mac, 0x86dd, msg.4)]);

    // from the CPU port to phy1
    if4.send(phy2.mac, if2.addr, msg.5)?;
    expect_frames!(phy3, &[RxFrame::new(cpu.mac, 0x86dd, msg.5)]);
    Ok(())
}
