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

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    apply {
        if (hdr.ethernet.isValid()) {
            //egress.port = 16w1;
        }
        if (hdr.ipv6.isValid()) {
            //egress.port = 16w1;
        }
        if (hdr.udp.isValid()) {
            //egress.port = 16w1;
        }
        if (hdr.geneve.isValid()) {
            // strip the geneve header and try to route
            hdr.geneve.setInvalid();
            hdr.ethernet = hdr.inner_eth;
            hdr.inner_eth.setInvalid();
            if (hdr.inner_ipv4.isValid()) {
                hdr.ipv4 = hdr.inner_ipv4;
                //hdr.ipv4.version = 4w4;
                //hdr.ipv4.ihl = 4w5;
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
            egress.port = 16w1;
        }
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}
