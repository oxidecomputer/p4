extern Checksum {
    bit<16> run<T>(in T data);
}

struct headers_t {
    ethernet_h eth;
    ipv6_h ipv6;
    udp_h udp;
}

header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

header ipv6_h {
    bit<4>      version;
    bit<8>      traffic_class;
    bit<20>     flow_label;
    bit<16>     payload_len;
    bit<8>      next_hdr;
    bit<8>      hop_limit;
    bit<128>    src;
    bit<128>    dst;
}

header udp_h {
    bit<16> src_port;
    bit<16> dst_port;
    bit<16> len;
    bit<16> checksum;
}

control ingress(inout headers_t hdr) {

    Checksum() csum;

    apply {
        csum.run({
            hdr.eth.dst,
            hdr.ipv6.src,
            hdr.udp,
        });
    }
    
}
