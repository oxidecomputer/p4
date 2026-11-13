use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu};
use crate::{dynamic_expect_frames as expect_frames, muffins};
use std::net::Ipv4Addr;

mod switch_program {
    p4_macro::use_p4!(p4 = "test/src/p4/switch.p4", pipeline_name = "main_pipeline");
}

mod user_program_10 {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program_10.p4", pipeline_name = "user_program");
}

const ET: u16 = 0x0800;


///
///                    basic udp traffic                                traffic from src 10.0.10.5
///                    (port 0 <-> 1 swap)                              diverted to loaded program 10
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                     ~
///                    ~   switch.p4   ~                                ~  user_program_10.p4 ~
///                    ~   (system)    ~                                ~     (program 10)    ~
///                    ~               ~                                ~                     ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///             rx | tx   |         |   tx | rx                         rx 0,1 -> | tx port 10
///                v      |         |      v                                      v
///  *=======*            |         |             *=======*                   *========*
///  |       | -----------+         +-----------  |       |                   |        |
///  | phy 0 |                                    | phy 1 |                   | phy 10 |
///  |       | <----------+         +-----------> |       |                   |(egress)|
///  *=======*                                    *=======*                   |        |
///                                                                           *========*
///


#[test]
fn dynamic_prog() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(11, switch_program::main_pipeline::new(11), false);
    npu.load_user_program(10, user_program_10::main_pipeline::new(11));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy10 = npu.phy(10); // egress port for user_program_10

    npu.run();

    npu.add_table_entry(
        "ingress.configure_programs.src_ipv4_pkt",
        "set_user_program_id",
        &[10, 0, 10, 5, 32],
        &10u16.to_le_bytes(),
    );

    npu.add_user_program_table_entry(
        10,
        "ingress.tbl",
        "forward",
        &[10, 0, 10, 5, 32],
        &10u16.to_le_bytes(),
    );

    let msg = muffins!();
    let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 5));
    let pkt1 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 1, 5));
    let pkt10 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 10, 5));

    // Basic udp traffic: port 0 <-> port 1 swap (same as hub-style)
    let sent0 = pkt0.send_ipv4(phy1.mac, Ipv4Addr::new(10, 0, 1, 5), msg.0)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, ET, &sent0[20..])]);

    let sent1 = pkt1.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), msg.1)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, ET, &sent1[20..])]);

    // IPv4 address match, jump to user_program_10.p4 file
    let sent2 = pkt10.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 0, 5), msg.2)?;
    expect_frames!(phy10, &[RxFrame::new(phy1.mac, ET, &sent2[20..])]);

    // Flood a single port (phy10)
    let sent3 = pkt10.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 10, 5), msg.3)?;
    let sent4 = pkt10.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 10, 5), msg.4)?;
    let sent5 = pkt10.send_ipv4(phy0.mac, Ipv4Addr::new(10, 0, 10, 5), msg.5)?;
    expect_frames!(
        phy10,
        &[
            RxFrame::new(phy1.mac, ET, &sent3[20..]),
            RxFrame::new(phy1.mac, ET, &sent4[20..]),
            RxFrame::new(phy1.mac, ET, &sent5[20..]),
        ]
    );

    // Traffic counts
    assert_eq!(phy0.tx_count(), 1usize); // sent0
    assert_eq!(phy0.rx_count(), 1usize); // sent1

    assert_eq!(phy1.tx_count(), 5usize); // sent1, sent2, sent3, sent4, sent5
    assert_eq!(phy1.rx_count(), 1usize); // sent0

    assert_eq!(phy10.tx_count(), 0usize);
    assert_eq!(phy10.rx_count(), 4usize); // sent2, sent3, sent4, sent5

    Ok(())
}
