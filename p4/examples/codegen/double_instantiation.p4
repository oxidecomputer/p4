#include <p4/examples/codegen/core.p4>
#include <p4/examples/codegen/softnpu.p4>

SoftNPU(
    parse(),
    ingress()
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
        resolver.apply(23w47);
    }
}

control bar() {
    resolver() resolver;
    apply { resolver.apply(23w1701); }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    foo() foo;
    bar() bar;

    apply {
        bar.apply();
        foo.apply();
    }
}
