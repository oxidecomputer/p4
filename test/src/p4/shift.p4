#include <core.p4>
#include <softnpu_mcast.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

header result_h {
    bit<128> port_mask;
    bit<128> unshifted;
}

struct headers_t {
    result_h result;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
) {
    state start {
        pkt.extract(hdr.result);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action set_bitmap(bit<128> bitmap) {
        egress.bitmap_a = bitmap;
    }

    table bitmap_table {
        key = {
            ingress.port: exact;
        }
        actions = {
            set_bitmap;
            NoAction;
        }
        default_action = NoAction;
    }

    apply {
        bitmap_table.apply();
        egress.port_bitmap = egress.bitmap_a;
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        hdr.result.port_mask = 128w1 << egress.port;
        hdr.result.unshifted = hdr.result.port_mask >> egress.port;
    }
}
