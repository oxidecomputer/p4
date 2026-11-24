use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffin_man};
use std::net::Ipv4Addr;

mod user_program_60 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_60.p4", pipeline_name = "user_program_60");
}

const ET: u16 = 0x0800;

///
///
///     Packet must match on both ipv4 and port
///     else, route through normal port swap forwarding.
///
///                                                                     traffic where src or dst 
///                                                                     match ipv4 (10.0.60.1) and
///                    basic ipv4/udp traffic                           SRC_PORT or DST_PORT match (6001)
///                    (port 0 <-> 1 swap)                              diverted to loaded program 60
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_60.p4 ~
///                    ~   (system)    ~                                ~     (program 60)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 60
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 60 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///
///


#[test]
fn user_prog_60() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(61, crate::switch_program::main_pipeline::new(61), false);
    npu.load_user_program(60, user_program_60::main_pipeline::new(61));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy60 = npu.phy(60); // egress port for user_program_60

    npu.run();

    let target_ip = Ipv4Addr::new(10, 0, 60, 1);
    let other_ip = Ipv4Addr::new(10, 1, 60, 50);
    let target_port: u16 = 6001;
    let other_port: u16 = 6050;

    let mut ip_key = target_ip.octets().to_vec();
    ip_key.push(32); // lpm prefix length
    let port_key = target_port.to_le_bytes();

    // src ipv4 table entries
    npu.add_table_entry(
        "ingress.configure_programs.src_ipv4_match",
        "check_ip_match",
        &ip_key,
        &60u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        60,
        "ingress.src_ipv4_match",
        "check_ip_match",
        &ip_key,
        &[],
    );

    // dst ipv4 table entries
    npu.add_table_entry(
        "ingress.configure_programs.dst_ipv4_match",
        "check_ip_match",
        &ip_key,
        &60u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        60,
        "ingress.dst_ipv4_match",
        "check_ip_match",
        &ip_key,
        &[],
    );

    // src port table entries
    npu.add_table_entry(
        "ingress.configure_programs.src_port_match",
        "check_port_match",
        &port_key,
        &60u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        60,
        "ingress.src_port_match",
        "check_port_match",
        &port_key,
        &[],
    );

    // dst port table entries
    npu.add_table_entry(
        "ingress.configure_programs.dst_port_match",
        "check_port_match",
        &port_key,
        &60u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        60,
        "ingress.dst_port_match",
        "check_port_match",
        &port_key,
        &[],
    );

    // fwd table entry
    npu.add_user_program_table_entry(
        60,
        "ingress.fwd",
        "forward",
        &[],
        &60u16.to_le_bytes(),
    );

    let msg = muffin_man!();
    let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 5));
    let pkt1 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 1, 5));
    let target_src_ipv4 = Interface4::new(phy1.clone(), target_ip);
    let other_src_ipv4 = Interface4::new(phy1.clone(), other_ip);

    // Basic ipv4 traffic: port 0 <-> port 1 swap (same as hub-style)
    let sent0 = pkt0.send_ipv4(phy1.mac, Ipv4Addr::new(10, 0, 1, 5), msg.0)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, ET, &sent0[20..])]);

    let sent1 = pkt1.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), msg.1)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent1[20..])]);


    // SRC_IPv4 + SRC_PORT match, jump to user_program_60.p4 file
    let sent2 = target_src_ipv4.send_udp(phy0.mac, other_ip, target_port, other_port, msg.2)?;
    expect_frames!(phy60, &[RxFrame::new(phy1.mac, ET, &sent2[28..])]);

    // SRC_IPv4 + DST_PORT match, jump to user_program_60.p4 file
    let sent3 = target_src_ipv4.send_udp(phy0.mac, other_ip, other_port, target_port, msg.3)?;
    expect_frames!(phy60, &[RxFrame::new(phy1.mac, ET, &sent3[28..])]);

    // DST_IPv4 + SRC_PORT match, jump to user_program_60.p4 file
    let sent4 = other_src_ipv4.send_udp(phy0.mac, target_ip, target_port, other_port, msg.4)?;
    expect_frames!(phy60, &[RxFrame::new(phy1.mac, ET, &sent4[28..])]);

    // DST_IPv4 + DST_PORT match, jump to user_program_60.p4 file
    let sent5 = other_src_ipv4.send_udp(phy0.mac, target_ip, other_port, target_port, msg.5)?;
    expect_frames!(phy60, &[RxFrame::new(phy1.mac, ET, &sent5[28..])]);

    // IPv4 matches, but neither port matches, normal fwd swap
    let sent6 = target_src_ipv4.send_udp(phy0.mac, other_ip, other_port, other_port, msg.6)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent6[28..])]);

    // Port matches, but neither IPv4 matches, normal fwd swap
    let sent7 = other_src_ipv4.send_udp(phy0.mac, other_ip, target_port, other_port, msg.7)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent7[28..])]);


    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 3usize); // sent1, sent6, sent7

    assert_eq!(phy1.tx_count(), 7usize); // sent1, sent2, sent3, sent4, sent5, sent6, sent7
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy60.tx_count(), 0usize);
    assert_eq!(phy60.rx_count(), 4usize); // sent2, sent3, sent4, sent5

    Ok(())
}
