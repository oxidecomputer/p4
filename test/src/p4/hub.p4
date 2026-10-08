#include <core.p4>
#include <softnpu.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
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
        egress.broadcast = true;
    }

    action broadcast_drop() {
        egress.broadcast = true;
        egress.drop = true;
    }

    action unicast_drop() {
        egress.port = 16w1;
        egress.drop = true;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
            broadcast_drop;
            unicast_drop;
        }
        default_action = drop;
        const entries = {
            16w0 : forward(16w1);
            16w1 : forward(16w0);
            16w2 : broadcast_drop();
            16w3 : unicast_drop();
        }
    }

    apply {
        tbl.apply();
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}
