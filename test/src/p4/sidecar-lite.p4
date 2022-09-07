#include <test/src/p4/core.p4>
#include <test/src/p4/softnpu.p4>
#include <test/src/p4/headers.p4>

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t {
    ethernet_h ethernet;
    sidecar_h sidecar;
    ipv4_h ipv4;
    ipv6_h ipv6;
    icmp_h icmp;
    tcp_h tcp;
    udp_h udp;

    geneve_h geneve;
    ethernet_h inner_eth;
    ipv4_h inner_ipv4;
    ipv6_h inner_ipv6;
    tcp_h inner_tcp;
    udp_h inner_udp;
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
        if (hdr.ipv6.next_hdr == 8w58) {
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
        ingress.nat_id = hdr.icmp.data[15:0];
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
        ingress.nat_id = hdr.udp.dst_port;
        if (hdr.udp.dst_port == 16w6081) {
            transition geneve;
        }
        transition accept;
    }

    state tcp {
        pkt.extract(hdr.tcp);
        ingress.nat_id = hdr.tcp.dst_port;
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
        pkt.extract(hdr.inner_ipv4);
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

    Checksum() csum;

    action forward_to_sled(bit<128> target) {

        ingress.nat = true;

        bit<16> orig_l3_len = 0;
        bit<16> orig_l3_csum = 0;

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
            orig_l3_csum = hdr.udp.checksum;
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

        hdr.udp.checksum = csum.run({
            hdr.ipv6.src,
            hdr.ipv6.dst,
            orig_l3_len + 16w14 + 16w8 + 16w8, // orig + eth + udp + geneve
            8w17, // udp next header
            16w6081, 16w6081, // geneve src/dst port
            orig_l3_len + 16w14 + 16w8 + 16w8, // orig + eth + udp + geneve
            orig_l3_csum,
        });

    }

    table nat_v4 {
        key = {
            hdr.ipv4.dst: exact;
            ingress.nat_id: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_v6 {
        key = {
            hdr.ipv6.dst: exact;
            ingress.nat_id: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_icmp_v6 {
        key = {
            hdr.ipv6.dst: exact;
            ingress.nat_id: range;
        }
        actions = { forward_to_sled; }
        default_action = NoAction;
    }

    table nat_icmp_v4 {
        key = {
            hdr.ipv4.dst: exact;
            ingress.nat_id: range;
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

    table local_v6 {
        key = {
            hdr.ipv6.dst: exact;
        }
        actions = {
            local;
            nonlocal;
        }
        default_action = nonlocal;
    }

    table local_v4 {
        key = {
            hdr.ipv4.dst: exact;
        }
        actions = {
            local;
            nonlocal;
        }
        default_action = nonlocal;
    }

    apply {
        if(hdr.ipv6.isValid()) {
            local_v6.apply();
            bit<16> ll = 16w0xff02;
            //TODO this is backwards should be
            //if(hdr.ipv6.dst[127:112] == ll) {
            if(hdr.ipv6.dst[15:0] == ll) {
                is_local = true;
            }
        }
        if(hdr.ipv4.isValid()) {
            local_v4.apply();
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

    table resolver_v4 {
        key = {
            egress.nexthop_v4: exact;
        }
        actions = { rewrite_dst; drop; }
        default_action = drop;
    }

    table resolver_v6 {
        key = {
            egress.nexthop_v6: exact;
        }
        actions = { rewrite_dst; drop; }
        default_action = drop;
    }

    apply {
        if (hdr.ipv4.isValid()) {
            resolver_v4.apply();
        }
        if (hdr.ipv6.isValid()) {
            resolver_v6.apply();
        }
    }
            
}
    

control router(
    inout headers_t hdr,
    inout IngressMetadata ingress,
    inout EgressMetadata egress,
) {

    resolver() resolver;

    action drop() { }

    action forward_v6(bit<8> port, bit<128> nexthop) {
        egress.port = port;
        egress.nexthop_v6 = nexthop;
    }

    action forward_v4(bit<8> port, bit<32> nexthop) {
        egress.port = port;
        egress.nexthop_v4 = nexthop;
    }

    table router_v6 {
        key = {
            hdr.ipv6.dst: lpm;
        }
        actions = {
            drop;
            forward_v6;
        }
        default_action = drop;
    }

    table router_v4 {
        key = {
            hdr.ipv4.dst: lpm;
        }
        actions = {
            drop;
            forward_v4;
        }
        default_action = drop;
    }

    apply {
        if (hdr.ipv4.isValid()) {
            router_v4.apply();
        }
        if (hdr.ipv6.isValid()) {
            router_v6.apply();
        }
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
    nat_ingress() nat;

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

                // TODO also need check for boundary services VNI?

                // strip the geneve header and try to route
                hdr.geneve.setInvalid();
                hdr.ethernet = hdr.inner_eth;
                hdr.inner_eth.setInvalid();
                if (hdr.inner_ipv4.isValid()) {
                    hdr.ipv4 = hdr.inner_ipv4;
                    hdr.ipv4.setValid();
                    hdr.ipv6.setInvalid();
                    hdr.inner_ipv4.setInvalid();
                }
                if (hdr.inner_ipv6.isValid()) {
                    hdr.ipv6 = hdr.inner_ipv6;
                    hdr.ipv6.setValid();
                    hdr.ipv4.setInvalid();
                    hdr.inner_ipv6.setInvalid();
                }
                if (hdr.inner_tcp.isValid()) {
                    hdr.tcp = hdr.inner_tcp;
                    hdr.tcp.setValid();
                    hdr.inner_tcp.setInvalid();
                }
                if (hdr.inner_udp.isValid()) {
                    hdr.udp = hdr.inner_udp;
                    hdr.udp.setValid();
                    hdr.inner_udp.setInvalid();
                }
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

            nat.apply(hdr, ingress);

            if (ingress.nat != true) {
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
}
