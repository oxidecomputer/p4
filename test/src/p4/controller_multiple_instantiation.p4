#include <test/src/p4/core.p4>
#include <test/src/p4/softnpu.p4>

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t { }

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout IngressMetadata ingress,
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
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {
    foo() taco;
    bar() pizza;

    apply {
        taco.apply();
        pizza.apply();
    }
}
