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
    bool nat;
    bit<16> l4_dst_port;
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
    icmp_h icmp;
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
    bit<16> len;
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
    out headers_t hdr,
    inout IngressMetadata ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        if (hdr.ethernet.ether_type == 16w0x0800) {
            transition ipv4;
        }
        if (hdr.ethernet.ether_type == 16w0x86dd) {
            transition ipv6;
        }
        if (hdr.ethernet.ether_type == 16w0x0901) {
            transition sidecar;
        }
        transition reject;
    }

    state sidecar {
        pkt.extract(hdr.sidecar);
        if (hdr.sidecar.sc_ether_type == 16w0x86dd) {
            transition ipv6;
        }
        if (hdr.sidecar.sc_ether_type == 16w0x0800) {
            transition ipv4;
        }
        transition reject;
    }

    state ipv6 {
        pkt.extract(hdr.ipv6);
        if (hdr.ipv6.next_hdr == 8w1) {
            transition icmp;
        }
        if (hdr.ipv6.next_hdr == 8w17) {
            transition udp;
        }
        if (hdr.ipv6.next_hdr == 8w6) {
            transition tcp;
        }
        transition accept;
    }

    state icmp {
        pkt.extract(hdr.icmp);
        ingress.l4_dst_port = hdr.icmp.data[15:0];
        transition accept;
    }

    state ipv4 {
        pkt.extract(hdr.ipv4);
        if (hdr.ipv4.protocol == 8w17) {
            transition udp;
        }
        if (hdr.ipv4.protocol == 8w6) {
            transition tcp;
        }
        transition accept;
    }

    state udp {
        pkt.extract(hdr.udp);
        ingress.l4_dst_port = hdr.udp.dst_port;
        if (hdr.udp.dst_port == 16w6081) {
            transition geneve;
        }
        transition accept;
    }

    state tcp {
        pkt.extract(hdr.tcp);
        ingress.l4_dst_port = hdr.tcp.dst_port;
        transition accept;
    }

    state geneve {
        pkt.extract(hdr.geneve);
        transition inner_eth;
    }

    state inner_eth {
        pkt.extract(hdr.inner_eth);
        if (hdr.inner_eth.ether_type == 16w0x0800) {
            transition inner_ipv4;
        }
        if (hdr.inner_eth.ether_type == 16w0x86dd) {
            transition inner_ipv6;
        }
        transition reject;
    }
    
    state inner_ipv4 {
        pkt.extract(hdr.ipv4);
        if (hdr.inner_ipv4.protocol == 8w17) {
            transition inner_udp;
        }
        if (hdr.inner_ipv4.protocol == 8w6) {
            transition inner_tcp;
        }
        transition accept;
    }

    state inner_ipv6 {
        pkt.extract(hdr.inner_ipv6);
        if (hdr.inner_ipv6.next_hdr == 8w17) {
            transition inner_udp;
        }
        if (hdr.inner_ipv6.next_hdr == 8w6) {
            transition inner_tcp;
        }
        transition accept;
    }

    state inner_udp {
        pkt.extract(hdr.inner_udp);
        transition accept;
    }

    state inner_tcp {
        pkt.extract(hdr.inner_tcp);
        transition accept;
    }

}

control nat_ingress(
    inout headers_t hdr,
    inout IngressMetadata ingress,
) {

    action forward_to_sled(bit<128> target) {

        bit<16> orig_l3_len = 0;

        // move L2 to inner L2

        hdr.inner_eth = hdr.ethernet;

        // move L3 to inner L3

        if (hdr.ipv4.isValid()) {
            hdr.inner_ipv4 = hdr.ipv4;
            orig_l3_len = hdr.ipv4.total_len;
        }
        if (hdr.ipv6.isValid()) {
            hdr.inner_ipv6 = hdr.ipv6;
            orig_l3_len = hdr.ipv6.payload_len + 16w40;
        }


        // move L4 to inner L4

        if (hdr.tcp.isValid()) {
            hdr.inner_tcp = hdr.tcp;
            hdr.inner_tcp.setValid();
            hdr.tcp.setInvalid();
        }
        if (hdr.udp.isValid()) {
            hdr.inner_udp = hdr.udp;
            hdr.inner_udp.setValid();
        }

        // set up outer l3

        // original l2 + original l3 + encapsulating udp + encapsulating geneve
        hdr.ipv6.payload_len = 16w14 + orig_l3_len + 16w8 + 16w8; 
        hdr.ipv6.next_hdr = 8w17;
        hdr.ipv6.hop_limit = 8w255;
        hdr.ipv6.src = 128w0; // TODO set to boundary services addr
        hdr.ipv6.dst = target;
        hdr.ipv6.setValid();

        // set up outer udp
        hdr.udp.src_port = 16w6081;
        hdr.udp.dst_port = 16w6081;
        hdr.udp.len = hdr.ipv6.payload_len;
        hdr.udp.checksum = 16w0; //TODO
        hdr.udp.setValid();

        // set up geneve
        hdr.geneve.version = 2w0;
        hdr.geneve.opt_len = 2w0;
        hdr.geneve.ctrl = 1w0;
        hdr.geneve.crit = 1w0;
        hdr.geneve.reserved = 6w0;
        hdr.geneve.protocol = hdr.inner_eth.ether_type;
        hdr.geneve.vni = 24w99; // XXX hard coded implicit boundary services vni
        hdr.geneve.reserved2 = 8w0;
        hdr.geneve.setValid();

    }

    table nat_v4 {
        key = {
            hdr.ipv4.dst: exact;
            ingress.l4_dst_port: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_v6 {
        key = {
            hdr.ipv6.dst: exact;
            ingress.l4_dst_port: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_icmp_v6 {
        key = {
            hdr.ipv6.dst: exact;
            ingress.l4_dst_port: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_icmp_v4 {
        key = {
            hdr.ipv4.dst: exact;
            ingress.l4_dst_port: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    apply {
        if (hdr.ipv4.isValid()) {
            if (hdr.icmp.isValid()) {
                nat_icmp_v4.apply();
            } else {
                nat_v4.apply();
            }
        }
        if (hdr.ipv6.isValid()) {
            if (hdr.icmp.isValid()) {
                nat_icmp_v6.apply();
            } else {
                nat_v6.apply();
            }
        }
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

            // check if this packet is destined to boundary services sourced
            // from within the rack.
            if (hdr.geneve.isValid()) {

                // TODO also check for boundary services VNI

                // strip the geneve header and try to route
                hdr.geneve.setInvalid();
                router.apply(hdr, ingress, egress);
            }

            // check if this packet is destined to boundary services from
            // outside the rack.

            else {
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
        }

        //
        // Otherwise route the packet using the L3 routing table.
        //

        else {
            // XXX? should be covered by sidecar check above
            // if the packet came from the scrimlet invalidate the header
            // sidecar header before routing.
            if (ingress.port == 8w1) {
                hdr.sidecar.setInvalid();
            }
            router.apply(hdr, ingress, egress);
        }
    }
}
