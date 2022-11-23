#include <core.p4>
#include <softnpu.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t { }

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
){
    state start { transition accept; }
}

control resolver(
    in bit<32> x,
    out bool resolved,
) {
    action resolve() {
        resolved = true;
    }
    table arp {
        key = { x: exact; }
        actions = { resolve; }
        default_action = NoAction;
    }
    apply { arp.apply(); }
}

control foo() {
    resolver() resolver;
    apply {
        bool resolved;
        resolver.apply(32w47, resolved);
    }
}

control bar() {
    resolver() resolver;
    resolver() taco;
    apply {
        bool resolved;
        resolver.apply(32w1701, resolved);
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    foo() taco;
    bar() pizza;

    apply {
        taco.apply();
        pizza.apply();
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}
