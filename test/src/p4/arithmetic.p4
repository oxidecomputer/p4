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
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        transition ipv4;
    }

    state ipv4 {
        pkt.extract(hdr.ipv4);
        hdr.ipv4.identification = hdr.ipv4.identification + 16w20;
        transition accept;
    }

}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        hdr.ipv4.identification = hdr.ipv4.identification - 16w5;
        egress.port = 16w1;
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
