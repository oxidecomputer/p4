#include <core.p4>
#include <softnpu_mcast.p4>

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
    Replicate() replicator;

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
    }

    action set_bitmap(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
    }

    action set_bitmap_broadcast(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
        egress.broadcast = true;
    }

    action set_bitmap_drop(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
        egress.drop = true;
    }

    table bitmap_table {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
            set_bitmap;
            set_bitmap_broadcast;
            set_bitmap_drop;
        }
        default_action = drop;
    }

    apply {
        bitmap_table.apply();
        replicator.replicate(egress.bitmap_a | egress.bitmap_b);
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        if (ingress.nat == true) {
            egress.drop = true;
        }
        ingress.nat = true;
    }
}
