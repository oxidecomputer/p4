// Copyright 2026 Oxide Computer Company

use crate::softnpu::{Interface4, SoftNpu};

p4_macro::use_p4!(
    p4 = "test/src/p4/slice_assign.p4",
    pipeline_name = "slice_assign",
);

/// Verify bit-slice assignment derives a multicast MAC from ipv4.dst
/// per RFC 1112 section 6.4, using byte-aligned slices on the LHS.
#[test]
fn slice_assign_mcast_mac() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);

    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);

    let if0 = Interface4::new(phy0.clone(), "10.0.0.1".parse().unwrap());

    npu.run();

    // Use 239.129.2.3 so bit 23 of the IP (MSB of second byte = 0x81)
    // is set, exercising the [23:23] = 0 clear.
    if0.send(phy1.mac, "239.129.2.3".parse().unwrap(), b"test")?;

    let frames = phy1.recv();
    let frame = &frames[0];

    // RFC 1112: 01:00:5e + lower 23 bits of dst IP.
    // dst IP = 239.129.2.3, ipv4.dst[23:16] = 0x81.
    // After clearing bit 23: 0x81 & 0x7f = 0x01.
    // Expected MAC: 01:00:5e:01:02:03
    assert_eq!(
        frame.dst,
        [0x01, 0x00, 0x5e, 0x01, 0x02, 0x03],
        "multicast MAC with bit 23 cleared"
    );

    // Same-field aliased assignment: ipv4.dst[3:0] = ipv4.dst[31:28].
    // dst IP = 0xEF810203, top nibble = 0xE.
    // After assignment: bottom nibble becomes 0xE, so last byte = 0x0E.
    let dst_ip = &frame.payload[16..20]; // ipv4.dst in the IPv4 header
    assert_eq!(
        dst_ip[3], 0x0E,
        "same-field alias: bottom nibble should be top nibble (0xE)"
    );

    // Single-bit set: ethernet.src[0:0] = 1w1.
    // Bit 0 is the LSB of the last byte of src MAC.
    // The original src MAC's last byte gets bit 0 set.
    assert_eq!(
        frame.src[5] & 0x01,
        0x01,
        "single-bit set: LSB of src MAC last byte"
    );

    Ok(())
}
