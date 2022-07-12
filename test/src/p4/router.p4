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
    sidecar_t sidecar;
    ipv6_t ipv6;
}

header sidecar_t {
    bit<8> sc_code;
    bit<8> sc_ingress;
    bit<8> sc_egress;
    bit<16> sc_ether_type;
    bit<128> sc_payload;
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
){
    state start {
        pkt.extract(headers.ethernet);
        if (headers.ethernet.ether_type == 16w0x86dd) {
            transition ipv6;
        }
        if (headers.ethernet.ether_type == 16w0x0901) {
            transition sidecar;
        }
        transition reject;
    }

    state sidecar {
        pkt.extract(headers.sidecar);
        if (headers.sidecar.sc_ether_type == 16w0x0901) {
            transition ipv6;
        }
        transition reject;
    }

    state ipv6 {
        pkt.extract(headers.ipv6);
        transition accept;
    }

}

control local(
    inout headers_t hdr,
    out bool is_local,
) {

    action nonlocal() {
        is_local = false;
    }

    action local() {
        is_local = true;
    }

    table local {
        key = {
            hdr.ipv6.dst: exact;
        }
        actions = {
            local;
            nonlocal;
        }
        default_action = nonlocal;
        const entries = {
            //fe80::1de1:deff:fe01:701c
            //128w0xfe800000000000001de1defffe01701c : local();
            128w0x1c7001feffdee11d00000000000080fe : local();

            //fe80::1de1:deff:fe01:701d
            //128w0xfe800000000000001de1defffe01701d : local();
            128w0x1d7001feffdee11d00000000000080fe : local();
        }
    }

    apply {
        local.apply();
        //TODO, backwards indexing behaving badly
        //bit<16> ll = 0xff02;
        //if(hdr.ipv6.dst[127:112] == ll) {
        //    is_local = true;
        //}
    }
    
}

control router(
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
            128w0xfd001000000000000000000000000000 &&&
            128w0xffffff00000000000000000000000000 :
            forward(1);

            // fd00:2000::/24
            128w0xfd002000000000000000000000000000 &&&
            128w0xffffff00000000000000000000000000 :
            forward(2);

        }
    }

    apply {
        router.apply();
    }

}

control ingress(
    inout headers_t hdr,
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {
    local() local;
    router() router;

    apply {

        //
        // Check if this is a packet coming from the scrimlet.
        //

        if (hdr.sidecar.isValid()) {
            egress.port = hdr.sidecar.sc_egress;
            hdr.sidecar.setInvalid();
            return;
        }

        //
        // If the packet has a local destination, create the sidecar header and
        // send it to the scrimlet.
        //

        bool local_dst = false;
        local.apply(hdr, local_dst);

        if (local_dst) {
            hdr.sidecar.setValid();

            //SC_FORWARD_TO_USERSPACE
            hdr.sidecar.sc_code = 8w0x01;
            hdr.sidecar.sc_ingress = ingress.port;
            hdr.sidecar.sc_ether_type = 16w0x86dd;
            hdr.sidecar.sc_payload = 128w0x1701d;

            // scrimlet port
            egress.port = 3;
        }

        //
        // Otherwise route the packet using the L3 routing table.
        //

        else {
            // if the packet came from the scrimlet invalidate the header
            // sidecar header so.
            if (ingress.port == 8w1) {
                hdr.sidecar.setInvalid();
            }
            router.apply(hdr, ingress, egress);
        }
    }
}
