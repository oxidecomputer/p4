use pnet::packet::ipv4::MutableIpv4Packet;
use pnet::packet::ipv6::MutableIpv6Packet;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::udp::MutableUdpPacket;
use std::net::{Ipv4Addr, Ipv6Addr};
// use pnet::packet::ethernet::{EtherType, MutableEthernetPacket};
// use pnet::util::MacAddr;

pub fn v6<'a>(
    src: Ipv6Addr,
    dst: Ipv6Addr,
    payload: &[u8],
    data: &'a mut [u8],
) -> MutableIpv6Packet<'a> {
    data.fill(0);

    let mut pkt = MutableIpv6Packet::new(data).unwrap();
    pkt.set_source(src);
    pkt.set_destination(dst);
    pkt.set_payload_length(payload.len() as u16);
    pkt.set_payload(payload);
    pkt
}

pub fn v4<'a>(
    src: Ipv4Addr,
    dst: Ipv4Addr,
    protocol: IpNextHeaderProtocol,
    payload: &[u8],
    data: &'a mut [u8],
) -> MutableIpv4Packet<'a> {
    data.fill(0);

    let mut pkt = MutableIpv4Packet::new(data).unwrap();
    pkt.set_version(4);
    pkt.set_header_length(5);
    pkt.set_ttl(64);
    //pkt.set_protocol(protocol);
    pkt.set_next_level_protocol(protocol);
    pkt.set_source(src);
    pkt.set_destination(dst);
    pkt.set_total_length(20 + payload.len() as u16);
    pkt.set_payload(payload);
    pkt
}

pub fn v4_udp(
    src: Ipv4Addr, 
    dst: Ipv4Addr,
    src_port: u16, 
    dst_port: u16, 
    payload: &[u8]) -> Vec<u8> {

    let mut udp = vec![0u8; 8 + payload.len()];
    {
        let mut udp_pkt = MutableUdpPacket::new(&mut udp).unwrap();
        udp_pkt.set_source(src_port);
        udp_pkt.set_destination(dst_port);
        udp_pkt.set_length(8 + payload.len() as u16);
        udp_pkt.set_checksum(0);
        udp_pkt.set_payload(payload);
    }

    let mut ipv4_udp = vec![0u8; 20 + udp.len()];
    v4(src, dst, IpNextHeaderProtocols::Udp, &udp, &mut ipv4_udp);
    ipv4_udp
}
