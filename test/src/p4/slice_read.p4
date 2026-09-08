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
        // Correctly reversed: storage is [0x03, 0x02, 0xAB, 0xEF].
        //   reversed_slice_range(23, 20, 32) maps to bitvec [16..20],
        //   which is the top nibble of storage byte 2 (0xAB) = 0xA.
        //
        // Without reversal, this will generate [20..24], which is the bottom
        // nibble of storage byte 2 (0xAB) = 0xB.
        if (hdr.ipv4.dst[23:20] == 4w0xa) {
            hdr.ipv4.identification = 16w42;
        }

        if (hdr.ipv4.ttl[3:0] == 4w0x5) {
            hdr.ipv4.protocol = 8w0x5b;
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
