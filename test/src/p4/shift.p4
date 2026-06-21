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
    Replicate() rep;

    action set_bitmap(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            set_bitmap;
        }
        default_action = NoAction;
    }

    apply {
        tbl.apply();
        rep.replicate(egress.bitmap_a);
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        // Test width conversion and shift: bit<16> -> bit<128>, then << and >>.
        bit<128> wide_port = egress.port;
        bit<128> port_mask = 128w1 << wide_port;
        bit<128> hit = egress.bitmap_a & port_mask;
        if (hit == 128w0) {
            egress.drop = true;
        }

        // Round-trip: shift up then back down, and the result should equal 1.
        bit<128> shifted = 128w1 << wide_port;
        bit<128> unshifted = shifted >> wide_port;
        if (unshifted != 128w1) {
            egress.drop = true;
        }
    }
}
