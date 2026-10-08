use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};
use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};

p4_macro::use_p4!(p4 = "test/src/p4/mcast.p4", pipeline_name = "mcast");

/// Build a port bitmap for use as action parameter_data.
/// `byte_len` is the byte width of the P4 `bit<N>` field (N / 8).
///
/// Little-endian encoding: bit N (value 2^N) corresponds to port N,
/// matching how p4rs arithmetic ([`p4rs::bitmath::shl_le`],
/// [`bitvec::field::BitField::load_le`]) interprets bitvec storage.
pub(crate) fn port_bitmap(byte_len: usize, ports: &[u16]) -> Vec<u8> {
    let mut bitmap = vec![0u8; byte_len];
    for &p in ports {
        let byte_idx = (p / 8) as usize;
        let bit_idx = p % 8;
        assert!(byte_idx < byte_len, "port {p} exceeds bitmap width");
        bitmap[byte_idx] |= 1 << bit_idx;
    }
    bitmap
}

fn assert_egress_ports(
    pipeline: &mut main_pipeline,
    ingress: u16,
    expected: &[u16],
) {
    let data = [0u8; 64];

    let mut pkt = packet_in::new(&data);
    let packet_ports: Vec<u16> = pipeline
        .process_packet(ingress, &mut pkt)
        .iter()
        .map(|(_, port)| *port)
        .collect();

    let mut pkt = packet_in::new(&data);
    let header_ports: Vec<u16> = pipeline
        .process_packet_headers(ingress, &mut pkt)
        .iter()
        .map(|(_, port)| *port)
        .collect();

    assert_eq!(packet_ports, expected, "packet (ingress {ingress})");
    assert_eq!(header_ports, expected, "headers (ingress {ingress})");
}

#[test]
fn bitmap_no_self_replication() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // Port 0 is in the bitmap but is also the ingress port.
    let bitmap = port_bitmap(16, &[0, 1, 2]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    npu.run();

    let msg = muffins!();

    // Port 0 should be excluded since it is the ingress port.
    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    assert_eq!(phy0.recv_buffer_len(), 0);
    assert_eq!(phy3.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn metadata_bit_fields_default_to_sized_zeros() {
    let egress = egress_metadata_t::default();

    assert_eq!(egress.bitmap_a.len(), 128);
    assert_eq!(egress.port_bitmap.len(), 128);
    assert_eq!(egress.nexthop_v6.len(), 128);
    assert_eq!(egress.nexthop_v4.len(), 32);
    assert_eq!(egress.port.len(), 16);
    assert!(!egress.bitmap_a.any());
    assert!(!egress.port_bitmap.any());
}

#[test]
fn no_table_match_yields_no_egress() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // Empty bitmap -> no ports set.
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &1u16.to_le_bytes(),
        &port_bitmap(16, &[]),
        0,
    );

    for ingress in [0, 1] {
        assert_egress_ports(&mut pipeline, ingress, &[]);
    }

    Ok(())
}

#[test]
fn empty_bitmap_falls_back_to_broadcast() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);
    let bitmap = port_bitmap(16, &[]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap_broadcast",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    assert_egress_ports(&mut pipeline, 0, &[1, 2, 3]);

    Ok(())
}

#[test]
fn empty_bitmap_falls_back_to_unicast() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);
    pipeline.add_ingress_bitmap_table_entry(
        "forward",
        &0u16.to_le_bytes(),
        &1u16.to_le_bytes(),
        0,
    );

    assert_egress_ports(&mut pipeline, 0, &[1]);

    Ok(())
}

fn assert_filtered_bitmap_has_no_egress(action: &str, bitmap_port: u16) {
    let mut pipeline = main_pipeline::new(4);
    pipeline.add_ingress_bitmap_table_entry(
        action,
        &0u16.to_le_bytes(),
        &port_bitmap(16, &[bitmap_port]),
        0,
    );

    assert_egress_ports(&mut pipeline, 0, &[]);
}

#[test]
fn filtered_bitmap_ingress_only_precedes_broadcast() {
    assert_filtered_bitmap_has_no_egress("set_bitmap_broadcast", 0);
}

#[test]
fn filtered_bitmap_out_of_radix_only_precedes_unicast() {
    assert_filtered_bitmap_has_no_egress("set_bitmap_unicast", 127);
}

#[test]
fn drop_precedes_nonempty_bitmap() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);
    let bitmap = port_bitmap(16, &[1, 2]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap_drop",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    assert_egress_ports(&mut pipeline, 0, &[]);

    Ok(())
}

#[test]
fn bitmap_precedence_over_broadcast() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // Bitmap with only port 1. The bitmap check runs before broadcast,
    // so even though broadcast might be set elsewhere, bitmap wins
    // when port_bitmap has bits set.
    let bitmap = port_bitmap(16, &[1]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap_broadcast",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    npu.run();

    let msg = muffins!();

    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    assert_eq!(phy2.recv_buffer_len(), 0);
    assert_eq!(phy3.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn bitmap_all_ports() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // All ports set, equivalent to broadcast.
    let bitmap = port_bitmap(16, &[0, 1, 2, 3]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    npu.run();

    let msg = muffins!();

    // Port 0 is ingress and should be excluded.
    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy3, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    assert_eq!(phy0.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn per_replica_ingress_metadata_is_isolated() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);
    let bitmap = port_bitmap(16, &[1, 2, 3]);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    assert_egress_ports(&mut pipeline, 0, &[1, 2, 3]);

    Ok(())
}

#[test]
fn bitmap_ports_beyond_radix_ignored() -> Result<(), anyhow::Error> {
    let mut radix_pipeline = main_pipeline::new(4);

    // Port 127 is the top bitmap bit and outside the radix-4
    // pipeline -> ignore it.
    let bitmap = port_bitmap(16, &[1, 127]);
    radix_pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    assert_egress_ports(&mut radix_pipeline, 0, &[1]);

    Ok(())
}

fn unicast_mac(port: u16) -> [u8; 6] {
    let id = 0x20 + port as u8;
    [0x02, 0x00, id, id, 0x00, 0x02]
}

fn replica_mac(port: u16) -> [u8; 6] {
    let id = 0x10 + port as u8;
    [0x02, 0x00, id, id, 0x00, 0x02]
}

fn add_mac_rewrite_entries(pipeline: &mut main_pipeline) {
    for port in 0u16..4 {
        pipeline.add_ingress_mac_mac_rewrite_entry(
            "rewrite",
            &port.to_le_bytes(),
            &unicast_mac(port),
            0,
        );
        pipeline.add_egress_mac_mac_rewrite_entry(
            "rewrite",
            &port.to_le_bytes(),
            &replica_mac(port),
            0,
        );
    }
}

fn assert_src_macs(pipeline: &mut main_pipeline, expected: &[(u16, [u8; 6])]) {
    let data = [0u8; 64];

    let mut pkt = packet_in::new(&data);
    let packets: Vec<(u16, [u8; 6])> = pipeline
        .process_packet(0, &mut pkt)
        .into_iter()
        .map(|(packet, port)| {
            (port, packet.header_data[6..12].try_into().unwrap())
        })
        .collect();

    let mut pkt = packet_in::new(&data);
    let headers: Vec<(u16, [u8; 6])> = pipeline
        .process_packet_headers(0, &mut pkt)
        .into_iter()
        .map(|(headers, port)| {
            let src = headers.ethernet.src_addr.load_le::<u64>().to_be_bytes();
            (port, src[2..].try_into().unwrap())
        })
        .collect();

    assert_eq!(packets, expected, "packet");
    assert_eq!(headers, expected, "headers");
}

#[test]
fn unicast_rewrites_mac_in_ingress() {
    let mut pipeline = main_pipeline::new(4);
    add_mac_rewrite_entries(&mut pipeline);
    pipeline.add_ingress_bitmap_table_entry(
        "forward",
        &0u16.to_le_bytes(),
        &2u16.to_le_bytes(),
        0,
    );

    assert_src_macs(&mut pipeline, &[(2, unicast_mac(2))]);
}

#[test]
fn replicas_rewrite_mac_in_egress() {
    let mut pipeline = main_pipeline::new(4);
    add_mac_rewrite_entries(&mut pipeline);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &port_bitmap(16, &[0, 2, 3]),
        0,
    );

    assert_src_macs(&mut pipeline, &[(2, replica_mac(2)), (3, replica_mac(3))]);
}
