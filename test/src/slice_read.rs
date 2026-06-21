// Copyright 2026 Oxide Computer Company

use pnet::packet::ipv4::Ipv4Packet;

use crate::softnpu::{Interface4, SoftNpu};

p4_macro::use_p4!(
    p4 = "test/src/p4/slice_read.p4",
    pipeline_name = "slice_read",
);

/// Read a sub-byte slice from a multi-byte field and verify the
/// byte-reversal mapping is correct.
///
/// Without byte-reversal adjustment, the codegen would produce
/// `[28..32]` instead of the correct `[24..28]`.
#[test]
fn slice_read_top_nibble() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);

    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);

    let if0 = Interface4::new(phy0.clone(), "10.0.0.1".parse().unwrap());

    npu.run();

    // dst IP = 239.171.2.3 = 0xEFAB0203.
    // ipv4.dst[23:20] = top nibble of 0xAB = 0xA.
    if0.send(phy1.mac, "239.171.2.3".parse().unwrap(), b"test")?;

    let frames = phy1.recv();
    let frame = &frames[0];
    let ip = Ipv4Packet::new(&frame.payload).unwrap();

    // The P4 compares ipv4.dst[23:20] == 0xA and sets identification=42
    // if true. With correct byte reversal the top nibble of 0xAB is 0xA,
    // so the branch is taken. Without byte-reversal adjustment,
    // [20..24] reads the bottom nibble (0xB) instead, the comparison
    // fails, and identification stays at 0.
    assert_eq!(
        ip.get_identification(),
        42,
        "ipv4.dst[23:20] should be 0xA (top nibble of 0xAB)"
    );

    Ok(())
}
