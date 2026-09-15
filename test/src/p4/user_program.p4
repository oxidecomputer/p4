#include <packet.p4>
#include <dynamic_softnpu.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_t ethernet;
    ipv4_t ipv4;
    udp_t udp;
    geneve_t geneve;
    inner_ethernet_t inner_ethernet;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}

header ipv4_t {
    bit<4>  version;
    bit<4>  ihl;
    bit<8>  diffserv;
    bit<16> total_len;
    bit<16> identification;
    bit<3>  flags;
    bit<13> frag_offset;
    bit<8>  ttl;
    bit<8>  protocol;
    bit<16> hdr_checksum;
    bit<32> src_addr;
    bit<32> dst_addr;
}

header udp_t {
    bit<16> src_port;
    bit<16> dst_port;
    bit<16> len;
    bit<16> checksum;
}

header geneve_t {
    bit<2>  version;
    bit<6>  opt_len;
    bit<1>  ctrl;
    bit<1>  crit;
    bit<6>  reserved;
    bit<16> protocol;
    bit<24> vni;
    bit<8>  reserved2;
}

header inner_ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
) {
    state start {
        pkt.extract(hdr.ethernet);
        if (hdr.ethernet.ether_type == 16w0x0800) {
            transition ipv4;
        }
        transition accept;
    }

    state ipv4 {
        pkt.extract(hdr.ipv4);
        if (hdr.ipv4.protocol == 8w17) {
            transition udp;
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

    state geneve {
        pkt.extract(hdr.geneve);
        transition inner_ethernet;
    }

    state inner_ethernet {
        pkt.extract(hdr.inner_ethernet);
        transition finish;
    }

    state finish {
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
        egress.broadcast = false;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
        }
        default_action = drop;
        const entries = {
            16w0 : forward(16w2);
            16w1 : forward(16w2);
        }
    }

    apply {
        tbl.apply();
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {}