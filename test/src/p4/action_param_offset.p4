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
    vlan_h vlan;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        if (hdr.ethernet.ether_type == 16w0x8100) {
            transition vlan;
        }
        transition reject;
    }

    state vlan {
        pkt.extract(hdr.vlan);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action forward(bit<12> vid, bit<16> port) {
        hdr.vlan.vid = vid;
        egress.port = port;
    }

    table retag {
        key = { ingress.port: exact; }
        actions = { forward; NoAction; }
        default_action = NoAction;
    }

    apply {
        retag.apply();
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
