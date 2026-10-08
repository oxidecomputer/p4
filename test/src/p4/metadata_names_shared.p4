#include <core.p4>
#include <softnpu_mcast.p4>

SoftNPU(parse(), ingress(), egress()) main;

header result_h {
    bit<16> ingress_port;
    bit<16> egress_port;
}

struct headers_t {
    result_h result;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t input,
) {
    state start {
        pkt.extract(hdr.result);
        hdr.result.ingress_port = input.port;
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t meta,
    inout egress_metadata_t output,
) {
    apply {
        output.port = 16w3;
        if (meta.port == 16w0) {
            output.port_bitmap = 128w6;
        } else if (meta.port == 16w2) {
            output.broadcast = true;
        }
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t input,
    inout egress_metadata_t meta,
) {
    apply {
        if (hdr.result.ingress_port != input.port) {
            meta.drop = true;
        }
        hdr.result.egress_port = meta.port;
    }
}
