use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};

p4_macro::use_p4!(p4 = "test/src/p4/shift.p4", pipeline_name = "shift");

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

/// Verify that << (shift) compiles and runs correctly in egress.
#[test]
fn shift_in_egress() -> Result<(), anyhow::Error> {
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

    npu.run();

    let msg = muffins!();
    phy0.send(&[TxFrame::new(phy1.mac, 0, msg.0)])?;

    expect_frames!(phy1, &[RxFrame::new(phy0.mac, 0, msg.0)]);
    expect_frames!(phy2, &[RxFrame::new(phy0.mac, 0, msg.0)]);

    // Port 3 is not in the bitmap. The shift-based check in egress
    // should drop its copy.
    let phy3 = npu.phy(3);
    assert_eq!(
        phy3.recv_buffer_len(),
        0,
        "port 3 should be dropped by bitmap check"
    );

    Ok(())
}

/// Width conversion and shift correctness for a higher port number.
/// This replicates to port 3 only, verifying the shift mask is correct
/// for non-trivial bit positions.
#[test]
fn shift_higher_port() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    let bitmap = port_bitmap(16, &[3]);
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
    phy0.send(&[TxFrame::new(phy3.mac, 0, msg.0)])?;

    expect_frames!(phy3, &[RxFrame::new(phy0.mac, 0, msg.0)]);

    // Ports 1 and 2 are not in the bitmap.
    assert_eq!(phy1.recv_buffer_len(), 0, "port 1 should be dropped");
    assert_eq!(phy2.recv_buffer_len(), 0, "port 2 should be dropped");

    Ok(())
}
