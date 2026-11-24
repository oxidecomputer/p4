use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffin_man};
use std::net::Ipv4Addr;

mod user_program_40 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_40.p4", pipeline_name = "user_program_40");
}

const ET: u16 = 0x0800;

///
///
///                                                                     traffic where src or dst 
///                                                                     match ipv4 (10.0.40.1)
///                    (port 0 <-> 1 swap)                              diverted to loaded program 40
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_40.p4 ~
///                    ~   (system)    ~                                ~     (program 40)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 40
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 40 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///
///


#[test]
fn user_prog_40() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(41, crate::switch_program::main_pipeline::new(41), false);
    npu.load_user_program(40, user_program_40::main_pipeline::new(41));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy40 = npu.phy(40); // egress port for user_program_40

    npu.run();

    let target_ip = Ipv4Addr::new(10, 0, 40, 1);
    let other_ip = Ipv4Addr::new(10, 1, 40, 50);

    let mut ip_key = target_ip.octets().to_vec();
    ip_key.push(32); // lpm prefix length

    // src ipv4 table entries
    npu.add_table_entry(
        "ingress.configure_programs.src_ipv4_pkt",
        "set_user_program_id",
        &ip_key,
        &40u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        40,
        "ingress.src_ipv4_match",
        "check_ip_match",
        &ip_key,
        &[],
    );

    // dst ipv4 table entries
    npu.add_table_entry(
        "ingress.configure_programs.dst_ipv4_pkt",
        "set_user_program_id",
        &ip_key,
        &40u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        40,
        "ingress.dst_ipv4_match",
        "check_ip_match",
        &ip_key,
        &[],
    );

    // fwd table entry
    npu.add_user_program_table_entry(
        40,
        "ingress.fwd",
        "forward",
        &[],
        &40u16.to_le_bytes(),
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


    // SRC_IPv4 matches target, jump to user_program_40.p4
    let sent2 = target_src_ipv4.send_ipv4(phy0.mac, other_ip, msg.2)?;
    expect_frames!(phy40, &[RxFrame::new(phy1.mac, ET, &sent2[20..])]);

    // DST_IPv4 matches target, jump to user_program_40.p4
    let sent3 = other_src_ipv4.send_ipv4(phy0.mac, target_ip, msg.3)?;
    expect_frames!(phy40, &[RxFrame::new(phy1.mac, ET, &sent3[20..])]);

    // Neither src nor dst matches, normal fwd swap
    let sent4 = other_src_ipv4.send_ipv4(phy0.mac, other_ip, msg.4)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent4[20..])]);


    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 2usize); // sent1, sent4

    assert_eq!(phy1.tx_count(), 4usize); // sent1, sent2, sent3, sent4
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy40.tx_count(), 0usize);
    assert_eq!(phy40.rx_count(), 2usize); // sent2, sent3

    Ok(())
}
