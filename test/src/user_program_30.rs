use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffin_man};
use std::net::Ipv4Addr;

mod user_program_30 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_30.p4", pipeline_name = "user_program_30");
}

const ET: u16 = 0x0800;
const SRC_PORT1: u16 = 4000;
const SRC_PORT2: u16 = 4001;
const DST_PORT: u16 = 5000;


///
///                                                                     traffic from src ipv4 
///                                                                     (10.0.30.1 or 10.0.30.2)
///                    basic ipv4/udp traffic                           and SRC_PORT (4000 or 4001)
///                    (port 0 <-> 1 swap)                              diverted to loaded program 30
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_30.p4 ~
///                    ~   (system)    ~                                ~     (program 30)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 30
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 30 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///


#[test]
fn user_prog_30() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(31, crate::switch_program::main_pipeline::new(31), false);
    npu.load_user_program(30, user_program_30::main_pipeline::new(31));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy30 = npu.phy(30); // egress port for user_program_30

    npu.run();

    for (addr, port) in [
        (Ipv4Addr::new(10, 0, 30, 1), SRC_PORT1),
        (Ipv4Addr::new(10, 0, 30, 1), SRC_PORT2),
        (Ipv4Addr::new(10, 0, 30, 2), SRC_PORT1),
        (Ipv4Addr::new(10, 0, 30, 2), SRC_PORT2),
    ] {
        let mut key = addr.octets().to_vec();
        key.push(32); // lpm prefix length
        key.extend_from_slice(&port.to_le_bytes());

        npu.add_table_entry(
            "ingress.configure_programs.src_ipv4_port_pkt",
            "set_user_program_id",
            &key,
            &30u16.to_le_bytes(),
        );

        npu.add_user_program_table_entry(
            30,
            "ingress.tbl",
            "forward",
            &key,
            &30u16.to_le_bytes(),
        );
    }

    let msg = muffin_man!();
    let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 5));
    let pkt1 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 1, 5));
    let pkt31 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 30, 1));
    let pkt32 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 30, 2));

    // Basic ipv4 traffic: port 0 <-> port 1 swap (same as hub-style)
    let sent0 = pkt0.send_ipv4(phy1.mac, Ipv4Addr::new(10, 0, 1, 5), msg.0)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, ET, &sent0[20..])]);

    let sent1 = pkt1.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), msg.1)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent1[20..])]);

    // IPv4 + SRC_PORT match, jump to user_program_30.p4 file
    let sent2 = pkt31.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 1), SRC_PORT1, DST_PORT, msg.2)?;
    expect_frames!(phy30, &[RxFrame::new(phy1.mac, ET, &sent2[28..])]);

    let sent3 = pkt32.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 2), SRC_PORT1, DST_PORT, msg.3)?;
    expect_frames!(phy30, &[RxFrame::new(phy1.mac, ET, &sent3[28..])]);

    // Flood a single port (phy30)
    let sent4 = pkt31.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 1), SRC_PORT1, DST_PORT, msg.4)?;
    let sent5 = pkt32.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 2), SRC_PORT2, DST_PORT, msg.5)?;
    let sent6 = pkt31.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 1), SRC_PORT2, DST_PORT, msg.6)?;
    let sent7 = pkt32.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 30, 2), SRC_PORT1, DST_PORT, msg.7)?;
    expect_frames!(
        phy30,
        &[
            RxFrame::new(phy1.mac, ET, &sent4[28..]),
            RxFrame::new(phy1.mac, ET, &sent5[28..]),
            RxFrame::new(phy1.mac, ET, &sent6[28..]),
            RxFrame::new(phy1.mac, ET, &sent7[28..]),
        ]
    );

    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 1usize); // sent1

    assert_eq!(phy1.tx_count(), 7usize); // sent1, sent2, sent3, sent4, sent5, sent6, sent7
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy30.tx_count(), 0usize);
    assert_eq!(phy30.rx_count(), 6usize); // sent2, sent3, sent4, sent5, sent6, sent7

    Ok(())
}
