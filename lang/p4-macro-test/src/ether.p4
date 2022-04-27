parser parsadillo(packet_in pkt, out headers_t headers){
    state start { transition accept; }
}

struct headers_t {
    ethernet_t ethernet;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}
