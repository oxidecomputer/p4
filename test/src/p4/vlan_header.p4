header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

header vlan_h {
    bit<3> pcp;
    bit<1> dei;
    bit<12> vid;
    bit<16> ether_type;
}

header sidecar_h {
    bit<8> sc_code;
    bit<8> sc_pad;
    bit<16> sc_ingress;
    bit<16> sc_egress;
    bit<16> sc_ether_type;
    bit<128> sc_payload;
}

header ipv4_h {
    bit<4>      version;
    bit<4>      ihl;
    bit<8>      diffserv;
    bit<16>     total_len;
    bit<16>     identification;
    bit<3>      flags;
    bit<13>     frag_offset;
    bit<8>      ttl;
    bit<8>      protocol;
    bit<16>     hdr_checksum;
    bit<32>     src;
    bit<32>     dst;
}

struct headers_t {
    ethernet_h ethernet;
    vlan_h vlan;
    sidecar_h sidecar;
    ipv4_h ipv4;
}
