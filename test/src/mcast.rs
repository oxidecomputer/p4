use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};
use p4rs::{packet_in, Pipeline};

p4_macro::use_p4!(p4 = "test/src/p4/mcast.p4", pipeline_name = "mcast");

/// Build a port bitmap for use as action parameter_data.
/// `byte_len` is the byte width of the P4 `bit<N>` field (N / 8).
/// LE encoding: bit N (value 2^N) corresponds to port N, matching
/// how p4rs arithmetic (shl_le, load_le) interprets bitvec storage.
fn port_bitmap(byte_len: usize, ports: &[u16]) -> Vec<u8> {
    let mut bitmap = vec![0u8; byte_len];
    for &p in ports {
        let byte_idx = (p / 8) as usize;
        let bit_idx = p % 8;
        assert!(byte_idx < byte_len, "port {p} exceeds bitmap width");
        bitmap[byte_idx] |= 1 << bit_idx;
    }
    bitmap
}

#[test]
fn bitmap_ports_1_2() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    let bitmap = port_bitmap(16, &[1, 2]);
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

    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0, msg.0)]);

    assert_eq!(phy3.recv_buffer_len(), 0);

    Ok(())
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

    npu.run();

    let msg = muffins!();

    // Port 0 should be excluded since it is the ingress port.
    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    assert_eq!(phy0.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn bitmap_empty() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // Empty bitmap: no ports set.
    let bitmap = port_bitmap(16, &[]);
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

    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;
    assert_eq!(phy0.recv_buffer_len(), 0);
    assert_eq!(phy1.recv_buffer_len(), 0);
    assert_eq!(phy2.recv_buffer_len(), 0);
    assert_eq!(phy3.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn metadata_bit_fields_default_to_sized_zeros() {
    let egress = egress_metadata_t::default();

    assert_eq!(egress.bitmap_a.len(), 128);
    assert_eq!(egress.bitmap_b.len(), 128);
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

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();

    assert_eq!(
        ports,
        Vec::<u16>::new(),
        "an unassigned egress port must not resolve to port 0"
    );

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

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();
    assert_eq!(ports, vec![1, 2, 3]);

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

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();
    assert_eq!(ports, vec![1]);

    Ok(())
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

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet(0, &mut pkt);
    assert!(out.is_empty());

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

    // Port 0 is ingress, should be excluded.
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

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();
    assert_eq!(ports, vec![1, 2, 3]);

    let mut pkt = packet_in::new(&data);
    let out = pipeline.process_packet_headers(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();
    assert_eq!(ports, vec![1, 2, 3]);

    Ok(())
}

#[test]
fn bitmap_ports_beyond_radix_ignored() -> Result<(), anyhow::Error> {
    let mut radix_pipeline = main_pipeline::new(4);

    // Port 127 is the top bitmap bit and outside the
    // radix-4 pipeline; ignore it.
    let bitmap = port_bitmap(16, &[1, 127]);
    radix_pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &bitmap,
        0,
    );

    let data = [0u8; 64];
    let mut pkt = packet_in::new(&data);
    let out = radix_pipeline.process_packet(0, &mut pkt);
    let ports: Vec<u16> = out.iter().map(|(_, port)| *port).collect();
    assert_eq!(ports, vec![1]);

    Ok(())
}
