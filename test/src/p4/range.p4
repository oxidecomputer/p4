#include <core.p4>
#include <softnpu.p4>
#include <headers.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_h ethernet;
    ipv4_h ipv4;
    ipv6_h ipv6;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        if (hdr.ethernet.ether_type == 16w0x0800) {
            transition ipv4;
        }
        if (hdr.ethernet.ether_type == 16w0x86dd) {
            transition ipv6;
        }
        transition reject;
    }

    state ipv4 {
        pkt.extract(hdr.ipv4);
        transition accept;
    }

    state ipv6 {
        pkt.extract(hdr.ipv6);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action forward(bit<16> port) {
        egress.port = port;
    }

    table power_ranger {
        key = {
            hdr.ipv4.dst: range;
        }
        actions = {
            forward;
        }
        default_action = NoAction;
    }

    apply {
        if(hdr.ipv4.isValid()) {
            power_ranger.apply();
        }
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
