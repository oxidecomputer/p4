use crate::softnpu::{self, Frame, Phy};
use colored::*;
use pnet::packet::ipv6::MutableIpv6Packet;
use std::net::Ipv6Addr;
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use xfr::{ring, FrameBuffer};

const R: usize = 1024;
const N: usize = 4096;
const F: usize = 1500;

p4_macro::use_p4!(p4 = "test/src/p4/router.p4", pipeline_name = "disag",);

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
fn disag_router() -> Result<(), anyhow::Error> {
    let fb = Arc::new(FrameBuffer::<N, F>::new());

    // ingress rings
    let (rx0_p, rx0_c) = ring::<R, N, F>(fb.clone());
    let (rx1_p, rx1_c) = ring::<R, N, F>(fb.clone());
    let (rx2_p, rx2_c) = ring::<R, N, F>(fb.clone());
    let (rx3_p, rx3_c) = ring::<R, N, F>(fb.clone());

    // egress rings
    let (tx0_p, tx0_c) = ring::<R, N, F>(fb.clone());
    let (tx1_p, tx1_c) = ring::<R, N, F>(fb.clone());
    let (tx2_p, tx2_c) = ring::<R, N, F>(fb.clone());
    let (tx3_p, tx3_c) = ring::<R, N, F>(fb);

    // create phys
    let phy0 = Phy::new(0, rx0_p);
    let phy1 = Phy::new(1, rx1_p);
    let phy2 = Phy::new(2, rx2_p);
    let phy3 = Phy::new(2, rx3_p);

    // run phys
    phy0.run(tx0_c, phy0_egress);
    phy1.run(tx1_c, phy1_egress);
    phy2.run(tx2_c, phy2_egress);
    phy3.run(tx3_c, phy3_egress);

    // run the softnpu with the compiled p4 pipelines
    spawn(move || {
        let rx = &[rx0_c, rx1_c, rx2_c, rx3_c];
        let tx = &[tx0_p, tx1_p, tx2_p, tx3_p];

        let mut pipeline = main_pipeline::new();
        softnpu::run_pipeline(rx, tx, &mut pipeline);
    });

    // shove some test data through the soft npu
    let ip1: Ipv6Addr = "fd00:1000::1".parse().unwrap();
    let mac1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

    let ip2: Ipv6Addr = "fd00:2000::1".parse().unwrap();
    let mac2 = [0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];

    let ip3: Ipv6Addr = "fe80::aae1:deff:fe01:701c".parse().unwrap();
    let mac3 = [0x01, 0xde, 0xde, 0x01, 0x70, 0x1c];

    let ip4: Ipv6Addr = "fe80::aae1:deff:fe01:701d".parse().unwrap();
    let mac4 = [0x01, 0xde, 0xde, 0x01, 0x70, 0x1d];

    let mc1: Ipv6Addr = "ff02::1:ff01:701c".parse().unwrap();
    let mmc1 = [0x33, 0x33, 0xff, 0x01, 0x70, 0x1c];

    let p = b"do you know the muffin man?";
    write(
        &phy1,
        99,
        1701,
        p.len(),
        p,
        47,
        23,
        ip1,
        ip2,
        mac1,
        mac2,
        None,
    );

    //~~~~
    let p = b"the muffin man?";
    write(
        &phy2,
        101,
        1701,
        p.len(),
        p,
        74,
        32,
        ip2,
        mc1,
        mac2,
        mmc1,
        None,
    );
    //~~~~~~~

    let p = b"the muffin man!";
    write(
        &phy1,
        99,
        1701,
        p.len(),
        p,
        47,
        23,
        ip1,
        ip3,
        mac1,
        mac3,
        None,
    );

    let p = b"why yes";
    write(
        &phy2,
        101,
        1701,
        p.len(),
        p,
        74,
        32,
        ip2,
        ip4,
        mac2,
        mac4,
        None,
    );

    let p = b"i know the muffin man";
    let mut sc = [0u8; 23];
    sc[0] = 1;
    sc[1] = 0;
    sc[2] = 3;
    sc[3] = 0;
    sc[4] = 2;
    sc[5] = 0x86;
    sc[6] = 0xdd;
    write(
        &phy0,
        101,
        1701,
        p.len(),
        p,
        74,
        32,
        ip3,
        ip2,
        mac3,
        mac2,
        Some(sc),
    );

    sc[2] = 1;
    let p = b"the muffin man is me!!!";
    write(
        &phy0,
        101,
        1701,
        p.len(),
        p,
        74,
        32,
        ip3,
        ip1,
        mac3,
        mac1,
        Some(sc),
    );

    sleep(Duration::from_secs(2));

    assert_eq!(
        phy0.count() + phy1.count() + phy2.count() + phy3.count(),
        6usize,
    );

    Ok(())
}

#[cfg(test)]
fn write(
    phy: &Phy<R, N, F>,
    traffic_class: u8,
    flow_label: u32,
    payload_length: usize,
    payload: &[u8],
    next_header: u8,
    hop_limit: u8,
    src: Ipv6Addr,
    dst: Ipv6Addr,
    smac: [u8; 6],
    dmac: [u8; 6],
    sc: Option<[u8; 23]>,
) {
    let mut data = [0u8; 256];
    let (index, et) = match sc {
        Some(sc) => {
            data[..23].copy_from_slice(&sc);
            (23, 0x0901u16)
        }
        None => (0, 0x86ddu16),
    };
    let _pkt = v6_pkt(
        &mut data[index..],
        traffic_class,
        flow_label,
        payload_length,
        payload,
        next_header,
        hop_limit,
        src,
        dst,
    );
    //println!("SEND {:x?}", data);
    phy.write(&[Frame::new(smac, dmac, et, &data)])
        .expect("phy write");
}

#[cfg(test)]
fn v6_pkt<'a>(
    data: &'a mut [u8],
    traffic_class: u8,
    flow_label: u32,
    payload_length: usize,
    payload: &[u8],
    next_header: u8,
    hop_limit: u8,
    src: Ipv6Addr,
    dst: Ipv6Addr,
) -> MutableIpv6Packet<'a> {
    let mut pkt = MutableIpv6Packet::new(data).unwrap();
    pkt.set_version(6);
    pkt.set_traffic_class(traffic_class);
    pkt.set_flow_label(flow_label);
    pkt.set_payload_length(payload_length as u16);
    pkt.set_payload(payload);
    pkt.set_next_header(pnet::packet::ip::IpNextHeaderProtocol(next_header));
    pkt.set_hop_limit(hop_limit);
    pkt.set_source(src);
    pkt.set_destination(dst);
    pkt
}

#[cfg(test)]
fn phy0_egress(frame: &[u8]) {
    let expected_messages = vec![
        b"the muffin man?".as_slice(),
        b"the muffin man!".as_slice(),
        b"why yes".as_slice(),
    ];
    let pkt = pnet::packet::ipv6::Ipv6Packet::new(&frame[37..77]).unwrap();
    let n = pkt.get_payload_length() as usize;
    let msg = &frame[77..77 + n];
    let sc = &frame[14..37];
    let dump =
        format!("{:#?} | {:x?} | {}", pkt, sc, String::from_utf8_lossy(msg),);
    println!("[{}] {}", "phy 0".magenta(), dump.dimmed());
    assert!(expected_messages.contains(&msg), "{:?}", msg);
}

#[cfg(test)]
fn phy1_egress(frame: &[u8]) {
    let expected_messages = vec![b"the muffin man is me!!!".as_slice()];
    let pkt = pnet::packet::ipv6::Ipv6Packet::new(&frame[14..54]).unwrap();
    let n = pkt.get_payload_length() as usize;
    let msg = &frame[54..54 + n];
    let dump = format!("{:#?} | {}", pkt, String::from_utf8_lossy(msg));
    println!("[{}] {}", "phy 1".magenta(), dump.dimmed());

    assert!(expected_messages.contains(&msg), "{:?}", msg);
}

#[cfg(test)]
fn phy2_egress(frame: &[u8]) {
    let expected_messages = vec![b"do you know the muffin man?".as_slice()];
    let pkt = pnet::packet::ipv6::Ipv6Packet::new(&frame[14..54]).unwrap();
    let n = pkt.get_payload_length() as usize;
    let msg = &frame[54..54 + n];
    let dump = format!("{:#?} | {}", pkt, String::from_utf8_lossy(msg));
    println!("[{}] {}", "phy 2".magenta(), dump.dimmed());

    assert!(expected_messages.contains(&msg), "{:?}", msg);
}

#[cfg(test)]
fn phy3_egress(frame: &[u8]) {
    let expected_messages = vec![b"i know the muffin man".as_slice()];
    let pkt = pnet::packet::ipv6::Ipv6Packet::new(&frame[14..54]).unwrap();
    let n = pkt.get_payload_length() as usize;
    let msg = &frame[54..54 + n];
    let dump = format!("{:#?} | {}", pkt, String::from_utf8_lossy(msg));
    println!("[{}] {}", "phy 3".magenta(), dump.dimmed());

    assert!(expected_messages.contains(&msg), "{:?}", msg);
}
