#include <p4/examples/codegen/core.p4>
#include <p4/examples/codegen/softnpu.p4>

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t {
    ethernet_t ethernet;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout IngressMetadata ingress,
){
    state start {
        pkt.extract(headers.ethernet);
        transition finish;
    }

    state finish {
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {

    action drop() { }

    action forward(bit<8> port) {
        egress.port = port;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
        }
        default_action = drop;
        const entries = {
            8w0 : forward(1);
            8w1 : forward(0);
        }
    }

    apply {
        tbl.apply();
    }

}
