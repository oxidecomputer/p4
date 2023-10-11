header hdr {
    bit<8>  e;
    bit<16> t;
    bit<8>  l;
    bit<8>  r;
    bit<1>  v;
}

struct Header_t {
    hdr h;
}

struct Meta_t {}

struct standard_metadata_t {
    bit<16> egress_spec;
}

control ingress(
        inout Header_t h,
        inout Meta_t m,
        inout standard_metadata_t standard_meta
) {
    action index(bit<16> mask) {
        bit<16> csum = 0;
        bit<16> offset = 0;
        offset = csum & mask;
    }
    action a() { standard_meta.egress_spec = 16w0; }
    action a_with_control_params(bit<16> x) { standard_meta.egress_spec = x; }

    table t_exact_ternary {

        key = {
            h.h.e : exact;
            h.h.t : ternary;
        }

        actions = {
            a;
            a_with_control_params;
        }

        default_action = a;

        const entries = {
            (0x01, 0x1111 &&& 0xF   ) : a_with_control_params(1);
            (0x02, 0x1181           ) : a_with_control_params(2);
            (0x03, 0x1111 &&& 0xF000) : a_with_control_params(3);
            (0x04, 0x1211 &&& 0x02F0) : a_with_control_params(4);
            (0x04, 0x1311 &&& 0x02F0) : a_with_control_params(5);
            //TODO (0x06, _                ) : a_with_control_params(6);
        }
    }

}
