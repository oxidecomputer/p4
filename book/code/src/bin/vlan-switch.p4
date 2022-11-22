#include <headers.p4>
#include <softnpu.p4>
#include <core.p4>

struct headers_t {
    ethernet_h eth;
    vlan_h vlan;
}

parser parse (
    packet_in pkt,
    out headers_t h,
    inout ingress_metadata_t ingress,
) {
    state start {
        pkt.extract(h.eth);
        if (h.eth.ether_type == 16w0x8100) { transition vlan; } 
        transition accept;
    }
    state vlan {
        pkt.extract(h.vlan);
        transition accept;
    }
}

control vlan(
    in bit<16> port,
    in bit<12> vid,
    out bool match,
) {
    action no_vid_for_port() {
        match = true;
    }

    action filter(bit<12> port_vid) { 
        if (port_vid == vid) { match = true; } 
    }
    
    table port_vlan {
        key             = { port: exact; }
        actions         = { no_vid_for_port; filter; }
        default_action  = no_vid_for_port;
    }

    apply { port_vlan.apply(); }
}

control forward(
    inout headers_t hdr,
    inout egress_metadata_t egress,
) {
    action drop() {}
    action forward(bit<16> port) { egress.port = port; }

    table fib {
        key             = { hdr.eth.dst: exact; }
        actions         = { drop; forward; }
        default_action  = drop;
    }

    apply { fib.apply(); }
}
    

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    vlan() vlan;
    forward() fwd;
    
    apply {
        bit<12> vid = 12w0;
        if (hdr.vlan.isValid()) {
            vid = hdr.vlan.vid;
        }

        // check vlan on ingress
        bool vlan_ok = false;
        vlan.apply(ingress.port, vid, vlan_ok);
        if (vlan_ok == false) {
            ingress.drop = true;
            return;
        }

        // apply switch forwarding logic
        fwd.apply(hdr, egress);

        // check vlan on egress
        vlan.apply(egress.port, vid, vlan_ok);
        if (vlan_ok == false) {
            ingress.drop = true;
            return;
        }
    }
}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}

SoftNPU(
    parse(), 
    ingress(),
    egress()
) main;
