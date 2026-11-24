use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffin_man};
use std::net::Ipv4Addr;

mod user_program_50 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_50.p4", pipeline_name = "user_program_50");
}

const ET: u16 = 0x0800;

///
///
///                                                                     traffic where SRC_PORT or
///                    basic ipv4/udp traffic                           DST_PORT match (5001)
///                    (port 0 <-> 1 swap)                              diverted to loaded program 50
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_50.p4 ~
///                    ~   (system)    ~                                ~     (program 50)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 50
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 50 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///
///


#[test]
fn user_prog_50() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(51, crate::switch_program::main_pipeline::new(51), false);
    npu.load_user_program(50, user_program_50::main_pipeline::new(51));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy50 = npu.phy(50); // egress port for user_program_50

    npu.run();

    let target_ip = Ipv4Addr::new(10, 0, 50, 1);
    let other_ip = Ipv4Addr::new(10, 1, 50, 10);
    let target_port: u16 = 5001;
    let other_port: u16 = 5010;

    let port_key = target_port.to_le_bytes();

    // src port table entries
    npu.add_table_entry(
        "ingress.configure_programs.src_port_pkt",
        "set_user_program_id",
        &port_key,
        &50u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        50,
        "ingress.src_port_match",
        "check_port_match",
        &port_key,
        &[],
    );

    // dst port table entries
    npu.add_table_entry(
        "ingress.configure_programs.dst_port_pkt",
        "set_user_program_id",
        &port_key,
        &50u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        50,
        "ingress.dst_port_match",
        "check_port_match",
        &port_key,
        &[],
    );

    // fwd table entry
    npu.add_user_program_table_entry(
        50,
        "ingress.fwd",
        "forward",
        &[],
        &50u16.to_le_bytes(),
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


    // SRC_PORT matches target, jump to user_program_50.p4
    let sent2 = target_src_ipv4.send_udp(phy0.mac, other_ip, target_port, other_port, msg.2)?;
    expect_frames!(phy50, &[RxFrame::new(phy1.mac, ET, &sent2[28..])]);

    // DST_PORT matches target, jump to user_program_50.p4
    let sent3 = other_src_ipv4.send_udp(phy0.mac, target_ip, other_port, target_port, msg.3)?;
    expect_frames!(phy50, &[RxFrame::new(phy1.mac, ET, &sent3[28..])]);

    // Neither port matches, normal fwd swap
    let sent4 = other_src_ipv4.send_udp(phy0.mac, other_ip, other_port, other_port, msg.4)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent4[28..])]);


    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 2usize); // sent1, sent4

    assert_eq!(phy1.tx_count(), 4usize); // sent1, sent2, sent3, sent4
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy50.tx_count(), 0usize);
    assert_eq!(phy50.rx_count(), 2usize); // sent2, sent3

    Ok(())
}
