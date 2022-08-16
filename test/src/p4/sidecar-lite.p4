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
    bit<128> nexthop;
    bool drop;
}

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t {
    ethernet_h ethernet;
    sidecar_h sidecar;
    ipv4_h ipv4;
    ipv6_h ipv6;
    tcp_h tcp;
    udp_h udp;

    geneve_h geneve;
    ethernet_h inner_eth;
    ipv4_h inner_ipv4;
    ipv6_h inner_ipv6;
    tcp_h inner_tcp;
    udp_h inner_udp;
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

header ipv6_h {
    bit<4>      version;
    bit<8>      traffic_class;
    bit<20>     flow_label;
    bit<16>     payload_len;
    bit<8>      next_hdr;
    bit<8>      hop_limit;
    bit<128>    src;
    bit<128>    dst;
}

header ipv4_h {
    bit<4>      version;
    bit<4>      ihl;
    bit<8>      diffserv;
    bit<16>     total_len;
    bit<16>     identification;
    bit<3>      flags;
    bit<13>     frag_offset;
    bit<8>      ttl;
    bit<8>      protocol;
    bit<16>     hdr_checksum;
    bit<32>     src;
    bit<32>     dst;
}

header udp_h {
    bit<16> src_port;
    bit<16> dst_port;
    bit<16> hdr_length;
    bit<16> checksum;
}

header tcp_h {
    bit<16> src_port;
    bit<16> dst_port;
    bit<32> seq_no;
    bit<32> ack_no;
    bit<4> data_offset;
    bit<4> res;
    bit<8> flags;
    bit<16> window;
    bit<16> checksum;
    bit<16> urgent_ptr;
}

header icmp_h {
    bit<8> type;
    bit<8> code;
    bit<16> hdr_checksum;
    bit<32> data;
}

header geneve_h {
    bit<2> version;
    bit<6> opt_len;
    bit<1> ctrl;
    bit<1> crit;
    bit<6> reserved;
    bit<16> protocol;
    bit<24> vni;
    bit<8> reserved2;
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
        if (headers.sidecar.sc_ether_type == 16w0x86dd) {
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
    }

    apply {
        local.apply();

        bit<16> ll = 16w0xff02;

        //TODO this is backwards should be
        //if(hdr.ipv6.dst[127:112] == ll) {
        if(hdr.ipv6.dst[15:0] == ll) {
            is_local = true;
        }
    }
    
}

control resolver(
    inout headers_t hdr,
    inout EgressMetadata egress,
) {
    action rewrite_dst(bit<48> dst) {
        //TODO the following creates a code generation error that should get
        //caught at compile time
        //
        //  hdr.ethernet = dst;
        //
        hdr.ethernet.dst = dst;
    }

    action drop() {
        egress.drop = true;
    }

    table resolver {
        key = {
            egress.nexthop: exact;
        }
        actions = { rewrite_dst; drop; }
        default_action = drop;
    }

    apply {
        resolver.apply();
    }
            
}
    

control router(
    inout headers_t hdr,
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {

    resolver() resolver;

    action drop() { }

    action forward(bit<8> port, bit<128> nexthop) {
        egress.port = port;
        egress.nexthop = nexthop;
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
    }

    apply {
        router.apply();
        if (egress.port != 8w0) {
            resolver.apply(hdr, egress);
        }
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

            //  Direct packets to the sidecar port corresponding to the scrimlet
            //  port they came from.
            egress.port = hdr.sidecar.sc_ingress;

            // Decap the sidecar header.
            hdr.sidecar.setInvalid();
            hdr.ethernet.ether_type = 16w0x86dd;

            // No more processing is required for sidecar packets, they simple
            // go out the sidecar port corresponding to the source scrimlet
            // port. No sort of hairpin back to the scrimlet is supported.
            // Similarly sending packets from one scrimlet port out a different
            // sidecar port is also not supported.
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
            hdr.ethernet.ether_type = 16w0x0901;

            //SC_FORWARD_TO_USERSPACE
            hdr.sidecar.sc_code = 8w0x01;
            hdr.sidecar.sc_ingress = ingress.port;
            hdr.sidecar.sc_egress = ingress.port;
            hdr.sidecar.sc_ether_type = 16w0x86dd;
            hdr.sidecar.sc_payload = 128w0x1701d;

            // scrimlet port
            egress.port = 0;
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
