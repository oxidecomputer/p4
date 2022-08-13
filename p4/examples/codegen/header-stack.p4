header ipv6_h {
	bit<4>	    version;
	bit<8>	    traffic_class;
	bit<20>	    flow_label;
	bit<16>	    payload_len;
	bit<8>	    next_hdr;
	bit<8>	    hop_limit;
	bit<128>    src;
	bit<128>    dst;
}

header sidecar_h {
    bit<8> sc_code;
    bit<8> sc_ingress;
    bit<8> sc_egress;
    bit<16> sc_ether_type;
    bit<128> sc_payload;
}

header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}
