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
        // Read a sub-byte slice from a non-top byte of a 32-bit field.
        // This exercises byte-reversal correctness.
        //
        // dst IP = 239.171.2.3 = 0xEFAB0203.
        // ipv4.dst[23:20] = top nibble of second wire byte = 0xA.
        //
        // Storage is byte-reversed: [0x03, 0x02, 0xAB, 0xEF]. A little-endian
        // load recovers 0xEFAB0203. [23:20] is then the top nibble of storage
        // byte 2 (0xAB) = 0xA. Reading its bottom nibble instead gives 0xB.
        //
        // Header writes record slice results without maintaining IPv4 checksums.
        if (hdr.ipv4.dst[23:20] == 4w0xa) {
            hdr.ipv4.identification = 16w42;
        }

        if (hdr.ipv4.ttl[3:0] == 4w0x5) {
            hdr.ipv4.protocol = 8w0x5b;
        }

        if (hdr.ipv4.dst[19:12] == 8w0xb0) {
            hdr.ipv4.diffserv = 8w0xb0;
        }

        if (hdr.ipv4.frag_offset[11:4] == 8w0xab) {
            hdr.ipv4.hdr_checksum = 16w0xab;
        }

        egress.port = 16w1;
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
