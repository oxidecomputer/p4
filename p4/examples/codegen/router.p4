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
struct IngressMetadata {
    bit<8> port;
}
struct EgressMetadata {
    bit<8> port;
}

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t {
    ethernet_t ethernet;
    ipv6_t ipv6;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}

header ipv6_t {
	bit<4>		version;
	bit<8>		traffic_class;
	bit<20>		flow_label;
	bit<16>		payload_len;
	bit<8>		next_hdr;
	bit<8>		hop_limit;
	bit<128> src_addr;
	bit<128> dst_addr;
}

parser parse(
    packet_in pkt,
    out headers_t headers,
){
    state start {
        pkt.extract(headers.ethernet);
        pkt.extract(headers.ipv6);
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {

    action drop() { }

    action forward(bit<8> port) {
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
            128w0xfd001000000000000000000000000000 &&& 128w0xffffff00000000000000000000000000 : forward(1);
            //128w0xfd001000000000000000000000000000 : forward(1);

        }
    }

    apply {
        router.apply();
    }

}
