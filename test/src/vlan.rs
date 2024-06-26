p4_macro::use_p4!("test/src/p4/vlan_header.p4");

#[test]
fn test_vlan_parse() -> anyhow::Result<()> {
    let mut data = [0u8; 4];
    data[0] = 0x0;
    data[1] = 0x47;
    let mut pkt = vlan_h::new();
    pkt.set(&data).unwrap();
    let vid: u16 = pkt.vid.to_owned().load_le();
    assert_eq!(vid, 0x47);
    let bv = pkt.to_bitvec();
    let readback = bv.into_vec();
    assert_eq!(data.to_vec(), readback);

    let mut data = [0u8; 4];
    data[0] = 0x77;
    data[1] = 0x47;
    let mut pkt = vlan_h::new();
    pkt.set(&data).unwrap();
    let vid: u16 = pkt.vid.to_owned().load_le();
    assert_eq!(vid, 0x747);
    let bv = pkt.to_bitvec();
    let readback = bv.into_vec();
    assert_eq!(data.to_vec(), readback);

    Ok(())
}
