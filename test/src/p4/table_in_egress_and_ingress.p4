#include <core.p4>
#include <softnpu.p4>
#include <headers.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_h eth;
}

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
){
    state start {
        transition accept;
    }
}

control foo(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }
    action forward(bit<16> port) { egress.port = port; }
    table tbl {
        key = { ingress.port: exact; }
        actions = { drop; forward; }
        default_action = drop;
    }

}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    foo() foo;
    apply {
        foo.apply(hdr, ingress, egress);
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    foo() foo;
    apply {
        foo.apply(hdr, ingress, egress);
    }
}

