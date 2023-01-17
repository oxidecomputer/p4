use crate::expect_frames;
use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use pnet::packet::ethernet::EtherType;
use pnet::packet::ethernet::MutableEthernetPacket;
use pnet::packet::ip::IpNextHeaderProtocol;
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet};
use pnet::packet::ipv6::MutableIpv6Packet;
use pnet::packet::udp::{MutableUdpPacket, UdpPacket};
use pnet::packet::Packet;
use pnet::util::MacAddr;
use std::net::{Ipv4Addr, Ipv6Addr};

p4_macro::use_p4!(p4 = "test/src/p4/decap.p4", pipeline_name = "decap",);

#[test]
fn geneve_decap() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);
    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);

    npu.run();

    let src: Ipv6Addr = "fd00::2".parse().unwrap();
    let dst: Ipv6Addr = "fd00::1".parse().unwrap();
    let inner_src: Ipv4Addr = "10.0.0.1".parse().unwrap();
    let inner_dst: Ipv4Addr = "8.8.8.8".parse().unwrap();

    /*
     * Create a header stack
     * eth
     * ipv6
     * udp
     * geneve
     * inner_eth
     * inner_ipv4
     * inner_udp
     */

    // start from bottom up
    let payload = b"muffins";
    let mut n = 8 + payload.len();
    let mut inner_udp_data: Vec<u8> = vec![0; n];

    let mut inner_udp = MutableUdpPacket::new(&mut inner_udp_data).unwrap();
    inner_udp.set_source(47);
    inner_udp.set_destination(74);
    inner_udp.set_payload(payload);
    inner_udp.set_checksum(99);

    n += 20;
    let mut inner_ip_data: Vec<u8> = vec![0; n];

    let mut inner_ip = MutableIpv4Packet::new(&mut inner_ip_data).unwrap();
    inner_ip.set_version(4);
    inner_ip.set_source(inner_src);
    inner_ip.set_header_length(5);
    inner_ip.set_destination(inner_dst);
    inner_ip.set_next_level_protocol(IpNextHeaderProtocol::new(17));
    inner_ip.set_total_length(20 + inner_udp_data.len() as u16);
    inner_ip.set_payload(&inner_udp_data);

    n += 14;
    let mut eth_data: Vec<u8> = vec![0; n];
    let mut eth = MutableEthernetPacket::new(&mut eth_data).unwrap();
    eth.set_destination(MacAddr::new(0x11, 0x11, 0x11, 0x22, 0x22, 0x22));
    eth.set_source(MacAddr::new(0x33, 0x33, 0x33, 0x44, 0x44, 0x44));
    eth.set_ethertype(EtherType(0x0800));
    eth.set_payload(&inner_ip_data);

    n += 8;
    let mut geneve_data: Vec<u8> =
        vec![0x00, 0x00, 0x65, 0x58, 0x11, 0x11, 0x11, 0x00];
    geneve_data.extend_from_slice(&eth_data);

    n += 8;
    let mut udp_data: Vec<u8> = vec![0; n];
    let mut udp = MutableUdpPacket::new(&mut udp_data).unwrap();
    udp.set_source(100);
    udp.set_destination(6081);
    udp.set_checksum(0x1701);
    udp.set_payload(&geneve_data);

    n += 40;
    let mut ip_data: Vec<u8> = vec![0; n];
    let mut ip = MutableIpv6Packet::new(&mut ip_data).unwrap();
    ip.set_source(src);
    ip.set_version(6);
    ip.set_destination(dst);
    ip.set_payload_length(udp_data.len() as u16);
    ip.set_payload(&udp_data);
    ip.set_next_header(IpNextHeaderProtocol::new(17));

    // outer eth is tacked on by phy::send

    phy0.send(&[TxFrame::new(phy1.mac, 0x86dd, &ip_data)])?;

    let fs = phy1.recv();
    let f = &fs[0];

    let mut decapped_ip = Ipv4Packet::new(&f.payload).unwrap();
    //let mut decapped_udp = UdpPacket::new(decapped_ip.payload()).unwrap();

    println!("Decapped IP: {:#?}", decapped_ip);
    //println!("Decapped UDP: {:#?}", decapped_udp);

    assert_eq!(
        Ipv4Packet::new(&inner_ip_data.clone()).unwrap(),
        decapped_ip
    );
    /*
    assert_eq!(
        UdpPacket::new(&inner_udp_data.clone()).unwrap(),
        decapped_udp
    );
    */

    Ok(())
}
