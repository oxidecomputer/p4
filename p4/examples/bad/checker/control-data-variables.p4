// Valid P4 control-scope data variables rejected by the x4c target.

header h_t {
    bit<8> f;
}

struct data_t {
    bit<8> f;
}

control ingress(out bit<16> port) {
    bit<16> selected = 16w1;
    bit<8> scratch;
    int<8> signed_value;
    bool enabled = true;
    h_t h;
    data_t data;

    action assign() {
        port = selected;
    }

    table route {
        key = {
            selected: exact;
        }
        actions = {
            assign;
        }
        default_action = assign;
    }

    apply {
        port = selected;
        assign();
        route.apply();
    }
}
