use crate::softnpu::{Interface6, RxFrame, SoftNpu};
use crate::{expect_frames, muffins};
use std::net::Ipv6Addr;

p4_macro::use_p4!(
    p4 = "test/src/p4/dynamic_router_noaddr_nbr.p4",
    pipeline_name = "mac_rewrite",
);

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
///      |  |                    |   *=======*
///      |  |                    |   |       |
///   *========*                 |---| phy 3 |
///   |        |                 |   |       |
///   |   sc   |                 *   *=======*
///   | (phy0) |                 
///   *========*                 
///

#[test]
fn mac_rewrite2() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new();

    //
    // add table entries
    //
    let addr_c: Ipv6Addr = "fe80::aae1:deff:fe01:701c".parse().unwrap();
    let addr_d: Ipv6Addr = "fe80::aae1:deff:fe01:701d".parse().unwrap();
    let addr_e: Ipv6Addr = "fe80::aae1:deff:fe01:701e".parse().unwrap();

    pipeline.add_local_local_entry(
        "set_local".into(),
        addr_c.octets().as_ref(),
        &Vec::new(),
    );
    pipeline.add_local_local_entry(
        "set_local".into(),
        addr_d.octets().as_ref(),
        &Vec::new(),
    );
    pipeline.add_local_local_entry(
        "set_local".into(),
        addr_e.octets().as_ref(),
        &Vec::new(),
    );

    // resolver table entries

    pipeline.add_router_resolver_resolver_entry(
        "rewrite_dst".into(),
        addr_c.octets().as_ref(),
        &[0x44, 0x44, 0x44, 0x44, 0x44, 0x44],
    );

    pipeline.add_router_resolver_resolver_entry(
        "rewrite_dst".into(),
        addr_d.octets().as_ref(),
        &[0x33, 0x33, 0x33, 0x33, 0x33, 0x33],
    );

    pipeline.add_router_resolver_resolver_entry(
        "rewrite_dst".into(),
        addr_e.octets().as_ref(),
        &[0x22, 0x22, 0x22, 0x22, 0x22, 0x22],
    );

    // routing table entries

    let prefix: Ipv6Addr = "fd00:1000::".parse().unwrap();
    let mut key = prefix.octets().to_vec();
    key.push(24); // prefix length
    let mut args = 1u16.to_be_bytes().to_vec();
    args.extend_from_slice(&addr_c.octets());
    pipeline.add_router_router_entry("forward", &key, &args);

    let prefix: Ipv6Addr = "fd00:2000::".parse().unwrap();
    let mut key = prefix.octets().to_vec();
    key.push(24); // prefix length
    let mut args = 2u16.to_be_bytes().to_vec();
    args.extend_from_slice(&addr_d.octets());
    pipeline.add_router_router_entry("forward", &key, &args);

    let prefix: Ipv6Addr = "fd00:3000::".parse().unwrap();
    let mut key = prefix.octets().to_vec();
    key.push(24); // prefix length
    let mut args = 3u16.to_be_bytes().to_vec();
    args.extend_from_slice(&addr_e.octets());
    pipeline.add_router_router_entry("forward", &key, &args);

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
    let m = [0x33, 0x33, 0x33, 0x33, 0x33, 0x33];
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0x86dd, msg.0)], m);

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
