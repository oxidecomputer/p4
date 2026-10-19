#include <core.p4>
#include <softnpu.p4>

// Ports 8 and 9 exercise x4c's implicit bit widening extension.
// P4-16 8.11.1 requires explicit casts between these widths.

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

header result_h {
    bit<16> value;
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
        transition accept;
    }
}

control set_value(inout headers_t hdr, in bit<16> value) {
    apply {
        hdr.result.value = value;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    set_value() setter;

    apply {
        hdr.result.setValid();
        hdr.result.value = 16w0;

        if (ingress.port == 16w0) {
            egress.port[15:0] = 16w1;
        } else if (ingress.port == 16w1) {
            // P4-16 8.25 leaves uninitialized locals unspecified.
            // SoftNPU initializes bit locals to zero.
            bit<16> uninit;
            if (uninit == 16w0) {
                hdr.result.value = 16w1;
            }
            egress.port = 16w1;
        } else if (ingress.port == 16w2) {
            egress.port[8:1] = 8w1;
        } else if (ingress.port == 16w3) {
            bit<16> high = 16w0x0100;
            bit<16> low = 16w0x00ff;
            if (high > low) {
                hdr.result.value = 16w1;
            }
            egress.port = 16w1;
        } else if (ingress.port == 16w4) {
            bit<16> two = 2;
            hdr.result.value = 1;
            hdr.result.value = hdr.result.value + two;
            egress.port = 16w1;
        } else if (ingress.port == 16w5) {
            setter.apply(hdr, 7);
            egress.port = 16w1;
        } else if (ingress.port == 16w6) {
            hdr.result.value[15:8] = 1;
            hdr.result.value[11:4] = 2;
            egress.port = 16w1;
        } else if (ingress.port == 16w7) {
            hdr.result.value = 16w0x0302;
            bit<8> high = hdr.result.value[15:8] << 1;
            if (high == 8w6) {
                hdr.result.value = 16w1 << hdr.result.value[3:0];
            }
            egress.port = 16w1;
        } else if (ingress.port == 16w8) {
            bit<8> narrow = 8w5;
            bit<16> wide = narrow;
            hdr.result.value[15:0] = narrow;
            if (wide == 16w5) {
                hdr.result.value = hdr.result.value + 16w0x0100;
            }
            egress.port = 16w1;
        } else if (ingress.port == 16w9) {
            bit<8> narrow = 8w0xab;
            bit<16> wide = 16w0;
            wide = narrow;
            hdr.result.value = narrow;
            hdr.result.value[15:8] = hdr.result.value[3:0];
            if (hdr.result.value == 16w0x0bab) {
                hdr.result.value = hdr.result.value[7:0];
                setter.apply(hdr, hdr.result.value[7:0]);
                if (hdr.result.value == wide) {
                    if (narrow == 8w0xab) {
                        hdr.result.value = 16w0x01ab;
                    }
                }
            }
            egress.port = 16w1;
        } else if (ingress.port == 16w10) {
            bit<0> empty = 0;
            empty = 1;
            bit<16> wide = empty;
            hdr.result.value = wide + 16w4;
            egress.port = 16w1;
        }
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
}
