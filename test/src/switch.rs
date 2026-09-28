use crate::dynamic_softnpu::{Interface4, RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};
use std::net::Ipv4Addr;

mod switch_program {
    p4_macro::use_p4!(p4 = "test/src/p4/switch.p4", pipeline_name = "switch");
}

mod user_program {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program.p4", pipeline_name = "user_program");
}

const BASIC_UDP_PORT: u16 = 4000;

///
///                basic udp traffic                  geneve traffic (dst_port == 6081)
///                (port 0 <-> 1 swap)                diverted to loaded program 10
///
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~*
///                    ~               ~ -----------------------------> ~                    ~
///                    ~   switch.p4   ~                                ~  user_program.p4   ~
///                    ~   (system)    ~                                ~   (program 10)     ~
///                    ~               ~                                ~                    ~
///                    *~~~~~~~~~~~~~~~*                                *~~~~~~~~~~~~~~~~~~~~*
///                       |         |                                             |
///           rx  |  tx   |         |  tx  | rx                         rx 0,1 -> | tx port 2
///               v       |         |      v                                      v
///  *=======*            |         |             *=======*                  *=======*
///  |       | -----------+         +-----------  |       |                  |       |
///  | phy 0 |                                    | phy 1 |                  | phy 2 |
///  |       | <----------+         +-----------> |       |                  | (tap) |
///  *=======*                                    *=======*                  *=======*
///

#[test]
fn dynamic_prog() -> Result<(), anyhow::Error> {
    let mut npu = SoftNpu::new(3, switch_program::main_pipeline::new(3), false);
    npu.load_user_program(10, user_program::main_pipeline::new(3));

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);

    npu.run();

    let msg = muffins!();
    let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 1));
    let pkt1 = Interface4::new(phy1.clone(), Ipv4Addr::new(10, 0, 0, 2));

    // Basic udp traffic: port 0 <-> port 1 swap (Same as hub-style)
    // Test intended to never hit the resubmit path

    let sent0 = pkt0.send_udp(phy1.mac, Ipv4Addr::new(10, 0, 0, 2), BASIC_UDP_PORT, msg.0)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0x0800, &sent0)]);

    let sent1 = pkt1.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 0, 1), BASIC_UDP_PORT, msg.1)?;
    expect_frames!(phy0, &[RxFrame::new(phy1.mac, 0x0800, &sent1)]);

    let sent2 = pkt0.send_udp(phy1.mac, Ipv4Addr::new(10, 0, 0, 2), BASIC_UDP_PORT, msg.2)?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0x0800, &sent2)]);

    // Three packets sent back-to-back, port 1 -> port 0
    let sent3 = pkt1.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 0, 1), BASIC_UDP_PORT, msg.3)?;
    let sent4 = pkt1.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 0, 1), BASIC_UDP_PORT, msg.4)?;
    let sent5 = pkt1.send_udp(phy0.mac, Ipv4Addr::new(10, 0, 0, 1), BASIC_UDP_PORT, msg.5)?;
    expect_frames!(
        phy0,
        &[
            RxFrame::new(phy1.mac, 0x0800, &sent3),
            RxFrame::new(phy1.mac, 0x0800, &sent4),
            RxFrame::new(phy1.mac, 0x0800, &sent5),
        ]
    );

    // Geneve traffic (dst_port 6081): diverted through user_program.p4
    // (loaded as program 10) and lands on port 2 regardless of source port.

    let sent_geneve0 =
        pkt0.send_geneve(phy1.mac, Ipv4Addr::new(10, 0, 0, 2), phy1.mac, phy0.mac, 0x0800, msg.0)?;
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0x0800, &sent_geneve0)]);

    let sent_geneve1 =
        pkt1.send_geneve(phy0.mac, Ipv4Addr::new(10, 0, 0, 1), phy0.mac, phy1.mac, 0x0800, msg.1)?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0x0800, &sent_geneve1)]);

    // Traffic counts

    assert_eq!(phy0.tx_count(), 3usize); // sent0, sent2, sent_geneve0
    assert_eq!(phy0.rx_count(), 4usize); // sent1, sent3, sent4, sent5

    assert_eq!(phy1.tx_count(), 5usize); // sent1, sent3, sent4, sent5, sent_geneve1
    assert_eq!(phy1.rx_count(), 2usize); // sent0, sent2

    assert_eq!(phy2.tx_count(), 0usize); // phy2 never sends anything
    assert_eq!(phy2.rx_count(), 2usize); // sent_geneve0, sent_geneve1

    Ok(())
}