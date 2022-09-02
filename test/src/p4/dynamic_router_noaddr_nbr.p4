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
    ipv6_h ipv6;
}

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout IngressMetadata ingress,
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
