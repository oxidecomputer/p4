use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffins};
use std::net::Ipv4Addr;

mod user_program_20 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_20.p4", pipeline_name = "user_program_20");
}

const ET: u16 = 0x0800;
const SRC_PORT: u16 = 4000;
const DST_PORT: u16 = 5000;


///
///                    basic ipv4/udp traffic                           traffic from src 10.0.20.5:4000
///                    (port 0 <-> 1 swap)                              diverted to loaded program 20
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_20.p4 ~
///                    ~   (system)    ~                                ~     (program 20)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 20
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 20 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///


#[test]
fn user_prog_20() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(21, crate::switch_program::main_pipeline::new(21), false);
    npu.load_user_program(20, user_program_20::main_pipeline::new(21));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy20 = npu.phy(20); // egress port for user_program_20

    npu.run();

    let mut key = Ipv4Addr::new(10, 0, 20, 5).octets().to_vec();
    key.push(32); // lpm prefix length
    key.extend_from_slice(&SRC_PORT.to_le_bytes());

    npu.add_table_entry(
        "ingress.configure_programs.src_ipv4_port_pkt",
        "set_user_program_id",
        &key,
        &20u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        20,
        "ingress.tbl",
        "forward",
        &key,
        &20u16.to_le_bytes(),
    );

    let msg = muffins!();
    let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 5));
    let pkt1 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 1, 5));
    let pkt20 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 20, 5));

    // Basic ipv4 traffic: port 0 <-> port 1 swap (same as hub-style)
    let sent0 = pkt0.send_ipv4(phy1.mac, Ipv4Addr::new(10, 0, 1, 5), msg.0)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, ET, &sent0[20..])]);

    let sent1 = pkt1.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), msg.1)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent1[20..])]);

    // IPv4 + SRC_PORT match, jump to user_program_20.p4 file
    let sent2 = pkt20.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), SRC_PORT, DST_PORT, msg.2)?;
    expect_frames!(phy20, &[RxFrame::new(phy1.mac, ET, &sent2[28..])]);

    // Flood a single port (phy20)
    let sent3 = pkt20.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 20, 5), SRC_PORT, DST_PORT, msg.3)?;
    let sent4 = pkt20.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 20, 5), SRC_PORT, DST_PORT, msg.4)?;
    let sent5 = pkt20.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 20, 5), SRC_PORT, DST_PORT, msg.5)?;
    expect_frames!(
        phy20,
        &[
            RxFrame::new(phy1.mac, ET, &sent3[28..]),
            RxFrame::new(phy1.mac, ET, &sent4[28..]),
            RxFrame::new(phy1.mac, ET, &sent5[28..]),
        ]
    );

    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 1usize); // sent1

    assert_eq!(phy1.tx_count(), 5usize); // sent1, sent2, sent3, sent4, sent5
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy20.tx_count(), 0usize);
    assert_eq!(phy20.rx_count(), 4usize); // sent2, sent3, sent4, sent5

    Ok(())
}
