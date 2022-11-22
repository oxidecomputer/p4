// XXX import from core.p4
extern packet_in {
    void extract<T>(out T headerLvalue);
    void extract<T>(out T variableSizeHeader, in bit<32> varFieldSizeBits);
    T lookahead<T>();
    bit<32> length();  // This method may be unavailable in some architectures
    void advance(bit<32> bits);
}

// XXX import from core.p4
extern packet_out {
    void emit<T>(in T hdr);
}

// XXX import from softnpu.p4
struct ingress_metadata_t {
    bit<16> port;
    bool nat;
    bit<16> nat_id;
    bool drop;
}

struct egress_metadata_t {
    bit<16> port;
    bit<128> nexthop_v6;
    bit<32> nexthop_v4;
    bool drop;
    bool broadcast;
}

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_t ethernet;
    ipv6_t ipv6;
}

header ethernet_t {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

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

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(headers.ethernet);
        pkt.extract(headers.ipv6);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
    }

    table router {
        key = {
            hdr.ipv6.dst: lpm;
        }
        actions = {
            drop;
            forward;
        }
        default_action = drop;
        const entries = {

            // fd00:1000::/24
            128w0xfd001000000000000000000000000000 &&&
            128w0xffffff00000000000000000000000000 :
            forward(16w0);

            // fd00:2000::/24
            128w0xfd002000000000000000000000000000 &&&
            128w0xffffff00000000000000000000000000 :
            forward(16w1);

        }
    }

    apply {
        router.apply();
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}
