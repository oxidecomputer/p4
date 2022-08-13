header ipv6_t {
	bit<4>	    version;
	bit<8>	    traffic_class;
	bit<20>	    flow_label;
	bit<16>	    payload_len;
	bit<8>	    next_hdr;
	bit<8>	    hop_limit;
	bit<128>    src;
	bit<128>    dst;
}
