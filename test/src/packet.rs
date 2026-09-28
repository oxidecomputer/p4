use pnet::packet::ipv4::MutableIpv4Packet;
use pnet::packet::ipv6::MutableIpv6Packet;
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
    payload: &[u8],
    data: &'a mut [u8,]
) -> MutableIpv4Packet<'a> {
    data.fill(0);

    let mut pkt = MutableIpv4Packet::new(data).unwrap();
    pkt.set_source(src);
    pkt.set_destination(dst);
    pkt.set_total_length(20 + payload.len() as u16);
    pkt.set_payload(payload);
    pkt
}

/// Builds ipv4 (protocol = UDP) + udp + payload -- the "eth/ipv4/udp/payload"
/// shape, minus the ethernet header itself (TxFrame/Interface4::send adds
/// that separately, same as every other packet in this file).
pub fn v4_udp(src: Ipv4Addr, dst: Ipv4Addr, dst_port: u16, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();

    // ipv4 (20 bytes)
    buf.push(0x45); // version = 4, ihl = 5
    buf.push(0x00); // diffserv
    buf.extend_from_slice(&[0x00, 0x00]); // total_len
    buf.extend_from_slice(&[0x00, 0x00]); // identification
    buf.extend_from_slice(&[0x00, 0x00]); // flags/frag_offset
    buf.push(0x40); // ttl
    buf.push(17); // protocol = UDP
    buf.extend_from_slice(&[0x00, 0x00]); // header checksum
    buf.extend_from_slice(&src.octets());
    buf.extend_from_slice(&dst.octets());

    // udp (8 bytes)
    buf.extend_from_slice(&0u16.to_be_bytes()); // src_port
    buf.extend_from_slice(&dst_port.to_be_bytes()); // dst_port
    buf.extend_from_slice(&0u16.to_be_bytes()); // len
    buf.extend_from_slice(&0u16.to_be_bytes()); // checksum

    buf.extend_from_slice(payload);
    buf
}

/// Builds ipv4 (protocol = UDP) + udp (dst_port = 6081) + geneve + inner
/// ethernet + payload -- the "eth/ipv4/udp/geneve/eth/payload" shape, minus
/// the OUTER ethernet header. Written independently of `v4_udp` so each
/// function fully describes one packet shape on its own.
pub fn v4_udp_geneve_eth(
    src: Ipv4Addr,
    dst: Ipv4Addr,
    inner_dst_mac: [u8; 6],
    inner_src_mac: [u8; 6],
    inner_ether_type: u16,
    payload: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::new();

    // ipv4 (20 bytes)
    buf.push(0x45);
    buf.push(0x00);
    buf.extend_from_slice(&[0x00, 0x00]); // total_len
    buf.extend_from_slice(&[0x00, 0x00]); // identification
    buf.extend_from_slice(&[0x00, 0x00]); // flags/frag_offset
    buf.push(0x40); // ttl
    buf.push(17); // protocol = UDP
    buf.extend_from_slice(&[0x00, 0x00]); // header checksum
    buf.extend_from_slice(&src.octets());
    buf.extend_from_slice(&dst.octets());

    // udp (8 bytes) -- dst_port fixed to the standard geneve port
    buf.extend_from_slice(&0u16.to_be_bytes()); // src_port
    buf.extend_from_slice(&6081u16.to_be_bytes()); // dst_port
    buf.extend_from_slice(&0u16.to_be_bytes()); // len
    buf.extend_from_slice(&0u16.to_be_bytes()); // checksum

    // geneve (8 bytes)
    buf.push(0x00); // version/opt_len
    buf.push(0x00); // ctrl/crit/reserved
    buf.extend_from_slice(&0x6558u16.to_be_bytes()); // protocol: transparent ethernet bridging
    buf.extend_from_slice(&[0x00, 0x00, 0x00]); // vni
    buf.push(0x00); // reserved2

    // inner ethernet (14 bytes)
    buf.extend_from_slice(&inner_dst_mac);
    buf.extend_from_slice(&inner_src_mac);
    buf.extend_from_slice(&inner_ether_type.to_be_bytes());

    buf.extend_from_slice(payload);
    buf
}