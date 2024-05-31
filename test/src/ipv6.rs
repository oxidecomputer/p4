p4_macro::use_p4!("test/src/p4/sidecar-lite.p4");

#[test]
fn test_ipv6_parse() -> anyhow::Result<()> {
    let mut data = [0u8; 40];
    // version = 6
    // traffic class = 127 (0x7f)
    // flow label = 699050 (0xaaaaa)
    data[0] = 0b0110_1111;
    data[1] = 0b0111_1010;
    //data[1] = 0b1010_1111;
    data[2] = 0b1010_1010;
    data[3] = 0b1010_1010;
    // payload len 18247 (0x4747)
    data[4] = 47;
    data[5] = 47;
    // next_hdr
    data[6] = 99;
    // hop_limit
    data[7] = 10;
    // src fd00::1
    data[8] = 0xfd;
    data[9] = 0x00;
    data[10] = 0x00;
    data[11] = 0x00;
    data[12] = 0x00;
    data[13] = 0x00;
    data[14] = 0x00;
    data[15] = 0x00;
    data[16] = 0x00;
    data[17] = 0x00;
    data[18] = 0x00;
    data[19] = 0x00;
    data[20] = 0x00;
    data[21] = 0x00;
    data[22] = 0x00;
    data[23] = 0x01;
    // dst fd00::2
    data[24] = 0xfd;
    data[25] = 0x00;
    data[26] = 0x00;
    data[27] = 0x00;
    data[28] = 0x00;
    data[29] = 0x00;
    data[30] = 0x00;
    data[31] = 0x00;
    data[32] = 0x00;
    data[33] = 0x00;
    data[34] = 0x00;
    data[35] = 0x00;
    data[36] = 0x00;
    data[37] = 0x00;
    data[38] = 0x00;
    data[39] = 0x02;

    let mut v6 = ipv6_h::new();
    v6.set(&data).unwrap();

    let ver: u8 = v6.version.to_owned().load_le();
    assert_eq!(ver, 6);

    let tc: u8 = v6.traffic_class.to_owned().load_le();
    assert_eq!(tc, 127);

    let bv = v6.to_bitvec();
    let readback = bv.into_vec();
    assert_eq!(data.to_vec(), readback);

    Ok(())
}
