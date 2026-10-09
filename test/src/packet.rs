use pnet::packet::ipv4::MutableIpv4Packet;
use pnet::packet::ipv6::MutableIpv6Packet;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::udp::MutableUdpPacket;
use pnet::packet::ethernet::{EtherType, MutableEthernetPacket};
use pnet::util::MacAddr;
use std::net::{Ipv4Addr, Ipv6Addr};

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
    dst_port: u16, 
    payload: &[u8]) -> Vec<u8> {

    let mut udp = vec![0u8; 8 + payload.len()];
    {
        let mut udp_pkt = MutableUdpPacket::new(&mut udp).unwrap();
        udp_pkt.set_source(0);
        udp_pkt.set_destination(dst_port);
        udp_pkt.set_length(8 + payload.len() as u16);
        udp_pkt.set_checksum(0);
        udp_pkt.set_payload(payload);
    }

    let mut ipv4_udp = vec![0u8; 20 + udp.len()];
    v4(src, dst, IpNextHeaderProtocols::Udp, &udp, &mut ipv4_udp);
    ipv4_udp
}


pub fn v4_udp_geneve_eth(
    src: Ipv4Addr,
    dst: Ipv4Addr,
    inner_dst_mac: [u8; 6],
    inner_src_mac: [u8; 6],
    inner_ether_type: u16,
    payload: &[u8]) -> Vec<u8> {

    // inner ethernet (14 bytes) + payload -- pnet has a type for this one
    let mut inner_eth = vec![0u8; 14 + payload.len()];
    {
        let mut eth_pkt = MutableEthernetPacket::new(&mut inner_eth).unwrap();
        eth_pkt.set_destination(MacAddr::new(
            inner_dst_mac[0], inner_dst_mac[1], inner_dst_mac[2],
            inner_dst_mac[3], inner_dst_mac[4], inner_dst_mac[5],
        ));
        eth_pkt.set_source(MacAddr::new(
            inner_src_mac[0], inner_src_mac[1], inner_src_mac[2],
            inner_src_mac[3], inner_src_mac[4], inner_src_mac[5],
        ));
        eth_pkt.set_ethertype(EtherType::new(inner_ether_type));
        eth_pkt.set_payload(payload);
    }

    // geneve (8 bytes, no options) -- no pnet type exists for this has to be manually built
    let mut geneve = Vec::with_capacity(8 + inner_eth.len());
    geneve.push(0x00); // version/opt_len
    geneve.push(0x00); // ctrl/crit/reserved
    geneve.extend_from_slice(&0x6558u16.to_be_bytes()); // transparent ethernet bridging
    geneve.extend_from_slice(&[0x00, 0x00, 0x00]); // vni
    geneve.push(0x00); // reserved2
    geneve.extend_from_slice(&inner_eth);

    // udp (8 bytes) -- dst_port fixed to the standard geneve port, 6081
    let udp_len = 8 + geneve.len();
    let mut udp = vec![0u8; udp_len];
    {
        let mut udp_pkt = MutableUdpPacket::new(&mut udp).unwrap();
        udp_pkt.set_source(0);
        udp_pkt.set_destination(6081);
        udp_pkt.set_length(udp_len as u16);
        udp_pkt.set_checksum(0);
        udp_pkt.set_payload(&geneve);
    }

    let mut ipv4_udp = vec![0u8; 20 + udp.len()];
    v4(src, dst, IpNextHeaderProtocols::Udp, &udp, &mut ipv4_udp);
    ipv4_udp
}
