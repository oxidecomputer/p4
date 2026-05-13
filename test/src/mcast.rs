use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};

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
    pipeline.add_ingress_tbl_entry(
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
    pipeline.add_ingress_tbl_entry(
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
    pipeline.add_ingress_tbl_entry(
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
fn bitmap_precedence_over_broadcast() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // Bitmap with only port 1. The bitmap check runs before broadcast,
    // so even though broadcast might be set elsewhere, bitmap wins
    // when port_bitmap has bits set.
    let bitmap = port_bitmap(16, &[1]);
    pipeline.add_ingress_tbl_entry(
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
    assert_eq!(phy2.recv_buffer_len(), 0);
    assert_eq!(phy3.recv_buffer_len(), 0);

    Ok(())
}

#[test]
fn bitmap_all_ports() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    // All ports set, equivalent to broadcast.
    let bitmap = port_bitmap(16, &[0, 1, 2, 3]);
    pipeline.add_ingress_tbl_entry(
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
