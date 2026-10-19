#include <core.p4>
#include <softnpu.p4>

SoftNPU(parse(), ingress(), egress()) main;

header result_h {
    bit<128> count;
    bit<32> left_literal;
    bit<32> right_literal;
    bit<32> left_narrow;
    bit<32> right_narrow;
    bit<32> left_wide;
    bit<32> right_wide;
    bit<8> left_large_literal;
    bit<8> right_large_literal;
    bit<8> left_linear;
    bit<8> right_linear;
    bit<8> left_right_linear;
    bit<8> right_left_linear;
    bit<8> add_then_shift;
    bit<8> shift_by_sum;
    bit<8> grouped_shift;
    bit<8> grouped_comparison;
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
    apply {
        bit<8> narrow = hdr.result.count[7:0];
        hdr.result.left_literal = 32w0x81234567 << 1;
        hdr.result.right_literal = 32w0x81234567 >> 1;
        hdr.result.left_narrow = 32w0x81234567 << narrow;
        hdr.result.right_narrow = 32w0x81234567 >> narrow;
        hdr.result.left_wide = 32w0x81234567 << hdr.result.count;
        hdr.result.right_wide = 32w0x81234567 >> hdr.result.count;
        hdr.result.left_large_literal = 8w1 << 256;
        hdr.result.right_large_literal =
            8w128 >> 1267650600228229401496703205376;
        hdr.result.left_linear = 8w1 << 8w1 << 8w2;
        hdr.result.right_linear = 8w128 >> 8w1 >> 8w2;
        hdr.result.left_right_linear = 8w3 << 8w1 >> 8w2;
        hdr.result.right_left_linear = 8w7 >> 8w1 << 8w1;
        hdr.result.add_then_shift = 8w1 + 8w2 << 8w1;
        hdr.result.shift_by_sum = 8w1 << 8w1 + 8w1;
        hdr.result.grouped_shift = 8w1 << (8w1 << 8w2);
        if ((8w1 == 8w1) == true) {
            hdr.result.grouped_comparison = 8w1;
        } else {
            hdr.result.grouped_comparison = 8w0;
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
