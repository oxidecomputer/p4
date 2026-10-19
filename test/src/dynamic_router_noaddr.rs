use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};
use std::net::Ipv6Addr;

p4_macro::use_p4!(
    p4 = "test/src/p4/dynamic_router_noaddr.p4",
    pipeline_name = "dynamic_router_noaddr",
);

fn v6_packet(dst: Ipv6Addr) -> [u8; 54] {
    let mut buf = [0u8; 54];
    buf[..6].copy_from_slice(&[0x02, 0, 0, 0, 0, 2]);
    buf[6..12].copy_from_slice(&[0x02, 0, 0, 0, 0, 1]);
    buf[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
    buf[14] = 0x60;
    buf[21] = 64;
    buf[22..38].copy_from_slice(&[0xfd; 16]);
    buf[38..54].copy_from_slice(&dst.octets());
    buf
}

fn assert_egress_ether_types(
    pipeline: &mut main_pipeline,
    buf: &[u8],
    expected: &[(u16, u16)],
) {
    let mut pkt = packet_in::new(buf);
    let packets: Vec<(u16, u16)> = pipeline
        .process_packet(2, &mut pkt)
        .into_iter()
        .map(|(packet, port)| {
            let ether_type = u16::from_be_bytes([
                packet.header_data[12],
                packet.header_data[13],
            ]);
            (port, ether_type)
        })
        .collect();

    let mut pkt = packet_in::new(buf);
    let headers: Vec<(u16, u16)> = pipeline
        .process_packet_headers(2, &mut pkt)
        .into_iter()
        .map(|(headers, port)| {
            (port, headers.ethernet.ether_type.load_le::<u16>())
        })
        .collect();

    assert_eq!(packets, expected, "packet");
    assert_eq!(headers, expected, "headers");
}

#[test]
fn local_dst_goes_to_scrimlet() {
    let mut pipeline = main_pipeline::new(4);
    let dst: Ipv6Addr = "fd00:1000::1".parse().unwrap();
    pipeline.add_ingress_local_local_entry(
        "set_local",
        &u128::from(dst).to_le_bytes(),
        &[],
        0,
    );

    assert_egress_ether_types(&mut pipeline, &v6_packet(dst), &[(0, 0x0901)]);
}

#[test]
fn remote_dst_is_routed() {
    let mut pipeline = main_pipeline::new(4);
    let prefix: Ipv6Addr = "fd00:2000::".parse().unwrap();
    let mut key = prefix.octets().to_vec();
    key.push(24);
    pipeline.add_ingress_router_router_entry(
        "forward",
        &key,
        &3u16.to_le_bytes(),
        0,
    );

    let dst: Ipv6Addr = "fd00:2000::1".parse().unwrap();
    assert_egress_ether_types(&mut pipeline, &v6_packet(dst), &[(3, 0x86dd)]);
}
