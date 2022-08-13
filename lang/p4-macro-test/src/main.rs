p4_macro::use_p4!("lang/p4-macro-test/src/ether.p4");

fn main() {
    let buf = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // dst mac
        0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, // src mac
        0x86, 0xdd, // ipv6 ethertype
    ];

    let mut eth = ethernet_t::new();
    eth.set(&buf).unwrap();

    println!("dst: {:x?}", eth.dst_addr);
    println!("src: {:x?}", eth.src_addr);
    let ethertype =
        u16::from_be_bytes(eth.ether_type.as_raw_slice().try_into().unwrap());
    println!("ethertype: {:x?}", ethertype);
}
