use crate::softnpu::{RxFrame, SoftNpu, TxFrame};
use crate::{expect_frames, muffins};

p4_macro::use_p4!(
    p4 = "test/src/p4/action_param_offset.p4",
    pipeline_name = "action_param_offset",
);

fn forward_param(vid: u16, port: u16) -> Vec<u8> {
    let mut buf = vid.to_le_bytes().to_vec();
    buf.extend_from_slice(&port.to_le_bytes());
    buf
}

// A `bit<12>` action parameter followed by a `bit<16>` one.
//
// x4c lays out action data as ceil(n/8) little-endian bytes per parameter
// (in declaration order). This means that a VID above 255 comes through
// intact and the port is read from the two bytes after the VID.
#[test]
fn action_param_offset() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    pipeline.add_ingress_retag_entry(
        "forward",
        &0u16.to_le_bytes(),
        &forward_param(300, 2),
        0,
    );
    pipeline.add_ingress_retag_entry(
        "forward",
        &1u16.to_le_bytes(),
        &forward_param(4094, 3),
        0,
    );
    // Three bytes for a four-byte action is ignored rather than panicking.
    pipeline.add_ingress_retag_entry(
        "forward",
        &3u16.to_le_bytes(),
        &[0u8; 3],
        0,
    );
    assert_eq!(pipeline.get_ingress_retag_entries().len(), 2);

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    npu.run();

    let et = 0;
    let msg = muffins!();

    phy0.send(&[TxFrame::newv(phy2.mac, et, msg.0, 10)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy0.mac, et, msg.0, 300)]);

    phy1.send(&[TxFrame::newv(phy3.mac, et, msg.1, 20)])?;
    expect_frames!(phy3, &[RxFrame::newv(phy1.mac, et, msg.1, 4094)]);

    Ok(())
}
