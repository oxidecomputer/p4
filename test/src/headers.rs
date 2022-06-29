use num::bigint::BigUint;
p4_macro::use_p4!("p4/examples/codegen/ipv6_header.p4");

#[test]
fn ipv6_header_read_write() {

    /* TODO - bitvec interface

    //     1        0
    //      .... ....
    // |10100111|11110110|
    // |         
    //           ........
    // |00001010|01111111| >> 4
    //
    // |a7|f6|
    //   1  0
    //           ........
    // |0a|7f| >> 4
    //   1  0
    // 
    //                      1                   2                   3
    // |0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1|
    // |---------------------------------------------------------------|
    // |       0       |       1       |       2       |       3       | 
    // |---------------------------------------------------------------|
    // |  ver  |   traf cls    |             flow lbl                  |
    // |---------------------------------------------------------------|
    // |0 1 1 0 1 1 1 1 1 1 1 0 0 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0|
    //
    //  0  1
    // |f6|a7|
    //
    //   8      0
    // 0b11111111
    //
    //

    let mut data = [0u8; 40];
    // version = 6
    // traffic class = 127 (0x7f)
    // flow label = 699050 (0xaaaaa)
    //data[0] = 0b0110_0111;
    data[0] = 0b1111_0110;
    //data[1] = 0b1111_1010;
    data[1] = 0b1010_0111;
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

    let mut v6 = ipv6_t::new();
    v6.set(&mut data).unwrap();

    let version: BigUint = v6.version.unwrap().to_owned().into();
    println!("version = {}", version);
    assert_eq!(version, BigUint::from(6u8));

    let traffic_class: BigUint = v6.traffic_class.unwrap().to_owned().into();
    println!("traffic class = {}", traffic_class);
    assert_eq!(traffic_class, BigUint::from(127u8));
    */


}
