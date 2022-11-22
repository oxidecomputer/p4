#include <core.p4>
#include <softnpu.p4>
#include <headers.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_h ethernet;
    sidecar_h sidecar;
    ipv6_h ipv6;
}

parser parse(
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
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

        if(hdr.ipv6.dst[127:112] == ll) {
            is_local = true;
        }
    }
    
}

control router(
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
    }

    apply {
        router.apply();
    }

}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
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

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}
