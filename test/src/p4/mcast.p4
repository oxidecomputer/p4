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

control mac_rewrite(
    inout headers_t hdr,
    inout egress_metadata_t egress,
) {
    action rewrite(bit<48> mac) {
        hdr.ethernet.src_addr = mac;
    }

    table mac_rewrite {
        key = { egress.port: exact; }
        actions = { rewrite; NoAction; }
        default_action = NoAction;
    }

    apply {
        mac_rewrite.apply();
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    mac_rewrite() mac;

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

    action set_bitmap_unicast(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
        egress.port = 16w1;
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
            set_bitmap_unicast;
            set_bitmap_drop;
        }
        default_action = drop;
    }

    apply {
        bitmap_table.apply();
        egress.port_bitmap = egress.bitmap_a;
        if (egress.port_bitmap == 128w0) {
            mac.apply(hdr, egress);
        }
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    mac_rewrite() mac;

    apply {
        if (egress.port_bitmap != 128w0) {
            mac.apply(hdr, egress);
        }

        // Detect ingress metadata leaking between output copies.
        if (ingress.nat == true) {
            egress.drop = true;
        }
        ingress.nat = true;
    }
}
