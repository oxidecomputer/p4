parser bad_parser(
    packet_in pkt,
    out muffins_t the_muffins,
) {
    state start {
        transition accept;
    }
}

struct headers_t {
    ethernet_t ethernet;
}

header ethernet_t {
    EthernetAddress dst_addr;
    EthernetAddress src_addr;
    bit<16> ether_type;
}
