#include <core.p4>

header ethernet_t {
    EthernetAddress dst_addr;
    EthernetAddress src_addr;
    bit<16> ether_type;
}

header ipv6_t {
    bit<4> version;
    bit<6> ds_field;
    bit<2> ecn;
    bit<20> flow_label;
    bit<16> len;
    bit<8> next_header;
    bit<8> hop_limit;
    IPv6Address src_addr;
    IPv6Address dst_addr;
}

struct headers_t {
    ethernet_t ethernet;
    ipv6_t ipv6;
}

parser bump_parser(
    packet_in packet,
    out headers_t hdr,
    inout metadata_t meta,
    inout standard_metadata_t standard_metadata,
) {

    //
    // Parse the ethernet header and transition to ipv6 if that is the
    // ethertype
    //

    state parse_ethernet {
        packet.extract(hdr.ethernet);
        transition select(hdr.ethernet.ether_type) {
            0x86dd: parse_ipv6;
            default: accept;
        }
    }

    //
    // Parse the ipv6 header
    //

    state parse_ipv6 {
        packet.extract(hdr.ipv6);
        verify(hdr.ipv6.version == 4w6, error.Ipv6IncorrectVersion);
        transition select(hdr.ipv6.next_header) {
            default: accept;
        }
    }
    
}

control bump_deparser(
    packet_out packet,
    in headers_t hdr,
) {

    apply {
        packet.emit(hdr.ethernet);
        packet.emit(hdr.ipv6);
    }

}

control bum_ingress(
    inout headers_t hdr,
    inout metadata_t meta,
    inout standard_metadata_t standard_metadata
) {

    action bump_action() {
        hdr.ipv6.hop_limit = hdr.ipv6.hop_limit - 1;
        standard_meta.egress_spec = port;
    }

    table router {
        key = {
            hdr.ipv6.dst_addr: lpm;
        }
        actions = {
            bump_action;
        }
        size = 32;
    }

    apply {
        router.apply();
    }

}
