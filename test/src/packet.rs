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
    data: &'a mut [u8],
) -> MutableIpv4Packet<'a> {
    data.fill(0);

    let mut pkt = MutableIpv4Packet::new(data).unwrap();
    pkt.set_source(src);
    pkt.set_destination(dst);
    pkt.set_total_length(20 + payload.len() as u16);
    pkt.set_payload(payload);
    pkt
}
