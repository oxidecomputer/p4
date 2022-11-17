#include <core.p4>

struct headers_t {
    ethernet_h ethernet;
}

struct ingress_metadata_t {
    bit<16> port;
}

struct egress_metadata_t {
    bit<16> port;
    bool drop;
}

header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

parser parse (
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
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
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }

    action forward(bit<16> port) {
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
            16w0 : forward(16w1);
            16w1 : forward(16w0);
        }
    }

    apply {
        tbl.apply();
    }

}

SoftNPU(
    parse(),
    ingress()
) main;
