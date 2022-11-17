#include <core.p4>
#include <softnpu.p4>
#include <headers.p4>

SoftNPU(
    parse(),
    ingress()
) main;

struct headers_t {
    ethernet_h ethernet;
    sidecar_h sidecar;
    arp_h arp;
    ipv4_h ipv4;
    ipv6_h ipv6;

    ddm_h ddm;
    // The ddm original p4 code used a header stack, but Intel says this is not
    // efficient on Tofino, and x4c does not currently support header stacks. So
    // the following is an unrolled version. This is not easy on the eyes.
    ddm_element_t ddm0;
    ddm_element_t ddm1;
    ddm_element_t ddm2;
    ddm_element_t ddm3;
    ddm_element_t ddm4;
    ddm_element_t ddm5;
    ddm_element_t ddm6;
    ddm_element_t ddm7;
    ddm_element_t ddm8;
    ddm_element_t ddm9;
    ddm_element_t ddm10;
    ddm_element_t ddm11;
    ddm_element_t ddm12;
    ddm_element_t ddm13;
    ddm_element_t ddm14;
    ddm_element_t ddm15;

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
    inout ingress_metadata_t ingress,
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
        if (hdr.ethernet.ether_type == 16w0x0806) {
            transition arp;
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

    state arp {
        pkt.extract(hdr.arp);
        transition accept;
    }

    state ipv6 {
        pkt.extract(hdr.ipv6);
        if (hdr.ipv6.next_hdr == 8w0xdd) {
            transition ddm;
        }
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

    state ddm {
        pkt.extract(hdr.ddm);
        if (hdr.ddm.header_length >= 8w7) { pkt.extract(hdr.ddm0); }
        if (hdr.ddm.header_length >= 8w11) { pkt.extract(hdr.ddm1); }
        if (hdr.ddm.header_length >= 8w15) { pkt.extract(hdr.ddm2); }
        if (hdr.ddm.header_length >= 8w19) { pkt.extract(hdr.ddm3); }
        if (hdr.ddm.header_length >= 8w23) { pkt.extract(hdr.ddm4); }
        if (hdr.ddm.header_length >= 8w27) { pkt.extract(hdr.ddm5); }
        if (hdr.ddm.header_length >= 8w31) { pkt.extract(hdr.ddm6); }
        if (hdr.ddm.header_length >= 8w35) { pkt.extract(hdr.ddm7); }
        if (hdr.ddm.header_length >= 8w39) { pkt.extract(hdr.ddm8); }
        if (hdr.ddm.header_length >= 8w43) { pkt.extract(hdr.ddm9); }
        if (hdr.ddm.header_length >= 8w47) { pkt.extract(hdr.ddm10); }
        if (hdr.ddm.header_length >= 8w51) { pkt.extract(hdr.ddm11); }
        if (hdr.ddm.header_length >= 8w55) { pkt.extract(hdr.ddm12); }
        if (hdr.ddm.header_length >= 8w59) { pkt.extract(hdr.ddm13); }
        if (hdr.ddm.header_length >= 8w63) { pkt.extract(hdr.ddm14); }
        if (hdr.ddm.header_length >= 8w67) { pkt.extract(hdr.ddm15); }
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
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    Checksum() csum;

    action forward_to_sled(bit<128> target, bit<24> vni, bit<48> mac) {

        ingress.nat = true;

        bit<16> orig_l3_len = 0;
        bit<16> orig_l3_csum = 0;

        // move L2 to inner L2

        hdr.inner_eth = hdr.ethernet;
        hdr.inner_eth.dst = mac;
        hdr.inner_eth.setValid();

        // fix up outer L2
        hdr.ethernet.ether_type = 16w0x86dd;

        // move L3 to inner L3

        if (hdr.ipv4.isValid()) {
            hdr.inner_ipv4 = hdr.ipv4;
            orig_l3_len = hdr.ipv4.total_len;
            hdr.inner_ipv4.setValid();
        }
        if (hdr.ipv6.isValid()) {
            hdr.inner_ipv6 = hdr.ipv6;
            orig_l3_len = hdr.ipv6.payload_len + 16w40;
            hdr.inner_ipv6.setValid();
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
        hdr.ipv4.setInvalid();

        hdr.ipv6.version = 4w6; 
        // original l2 + original l3 + encapsulating udp + encapsulating geneve
        hdr.ipv6.payload_len = 16w14 + orig_l3_len + 16w8 + 16w8; 
        hdr.ipv6.next_hdr = 8w17;
        hdr.ipv6.hop_limit = 8w255;
        // XXX hardcoded boundary services addr
        hdr.ipv6.src = 128w0xfd000099000000000000000000000001;
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
        hdr.geneve.opt_len = 6w0;
        hdr.geneve.ctrl = 1w0;
        hdr.geneve.crit = 1w0;
        hdr.geneve.reserved = 6w0;
        hdr.geneve.protocol = 16w0x6558;

        hdr.geneve.vni = vni;

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
            if(hdr.ipv6.dst[127:112] == ll) {
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
    inout egress_metadata_t egress,
) {
    action rewrite_dst(bit<48> dst) {
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
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action drop() { }

    action forward_v6(bit<16> port, bit<128> nexthop) {
        egress.port = port;
        egress.nexthop_v6 = nexthop;
    }

    action forward_v4(bit<16> port, bit<32> nexthop) {
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
    }

}

control mac_rewrite(
    inout headers_t hdr,
    inout egress_metadata_t egress,
) {

    action rewrite(bit<48> mac) {
        hdr.ethernet.src = mac;
    }

    table mac_rewrite {
        key = { egress.port: exact; }
        actions = { rewrite; }
        default_action = NoAction;
    }

    apply {
        mac_rewrite.apply();
    }

}

control proxy_arp(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action proxy_arp_reply(bit<48> mac) {
        egress.port = ingress.port;
        hdr.ethernet.dst = hdr.ethernet.src;
        hdr.ethernet.src = mac;
        hdr.arp.target_mac = hdr.arp.sender_mac;
        hdr.arp.sender_mac = mac;
        hdr.arp.opcode = 16w2;

        //TODO compiler broken for this, should be able to do this in one line.
        bit<32> tmp = 0;
        tmp = hdr.arp.sender_ip;
        /// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

        hdr.arp.sender_ip = hdr.arp.target_ip;
        hdr.arp.target_ip = tmp;
    }

    table proxy_arp {
        key = { hdr.arp.target_ip: range; }
        actions = { proxy_arp_reply; }
        default_action = NoAction;
    }

    apply {
        proxy_arp.apply();
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    local() local;
    router() router;
    nat_ingress() nat;
    resolver() resolver;
    mac_rewrite() mac;
    proxy_arp() pxarp;

    apply {

        //
        // Check if this is a packet coming from the scrimlet.
        //

        if (hdr.sidecar.isValid()) {

            //  Direct packets to the sidecar port corresponding to the scrimlet
            //  port they came from.
            egress.port = hdr.sidecar.sc_egress;

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

        if (hdr.arp.isValid()) {
            pxarp.apply(hdr, ingress, egress);
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
                    hdr.udp.setInvalid();
                    hdr.tcp.setValid();
                    hdr.inner_tcp.setInvalid();
                }
                if (hdr.inner_udp.isValid()) {
                    hdr.udp = hdr.inner_udp;
                    hdr.udp.setValid();
                    hdr.inner_udp.setInvalid();
                }
                router.apply(hdr, ingress, egress);
                if (egress.port != 16w0) {
                    resolver.apply(hdr, egress);
                }
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
                // TODO simply stating 0 here causes bad conversion, initializes
                // egress.port as a 128 bit value due to
                // StatementGenerator::converter using int_to_bitvec
                egress.port = 16w0;
            }
            return;
        }

        //
        // Otherwise route the packet using the L3 routing table.
        //

        else {
            // check for ingress nat
            nat.apply(hdr, ingress, egress);

            router.apply(hdr, ingress, egress);
            if (egress.port != 16w0) {
                resolver.apply(hdr, egress);
            }
        }

        //
        // Rewrite the mac on the way out the door.
        //

        mac.apply(hdr, egress);
    }
}
