// Copyright 2026 Oxide Computer Company

#include <core.p4>
#include <softnpu.p4>
#include <headers.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_h ethernet;
    ipv4_h ipv4;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        transition ipv4;
    }

    state ipv4 {
        pkt.extract(hdr.ipv4);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        // Derive multicast dst MAC from ipv4.dst (RFC 1112 section 6.4).
        hdr.ethernet.dst[47:24] = 24w0x01005e;
        hdr.ethernet.dst[23:16] = hdr.ipv4.dst[23:16];
        hdr.ethernet.dst[15:0] = hdr.ipv4.dst[15:0];
        hdr.ethernet.dst[23:23] = 1w0;

        // Copy ipv4.dst top nibble into its own bottom nibble,
        // exercising same-field aliased slice assignment.
        hdr.ipv4.dst[3:0] = hdr.ipv4.dst[31:28];

        // Set a single bit to exercise [n:n] = 1w1.
        hdr.ethernet.src[0:0] = 1w1;

        egress.port = 16w1;
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
