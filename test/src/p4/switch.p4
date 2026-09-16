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
    bit<16> length;
    bit<16> checksum;
}

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);

        transition select(hdr.ethernet.ether_type) {
            16w0x0800: parse_ipv4;
            default: finish;
        }
    }

    state parse_ipv4 {
        pkt.extract(hdr.ipv4);

        transition select(hdr.ipv4.protocol) {
            8w17: parse_udp;
            default: finish;
        }
    }

    state parse_udp {
        pkt.extract(hdr.udp);
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

    // Set the user program in the ingress metadata to the program id in table
    // action entry.
    action set_user_program_id(bit<16> program_id) {
        ingress.user_program = program_id;
    }

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
        egress.broadcast = false;
    }

    // This table associates geneve packets with a user program id.
    table geneve_pkt {
        key = {
            hdr.udp.dst_port: exact;
        }
        actions = {
            set_user_program_id;
        }
        default_action = NoAction;

        const entries = {
            16w6081 : set_user_program_id(16w10);
        }
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
            16w0 : forward(16w1);
            16w1 : forward(16w0);
        }
    }

    apply {
        geneve_pkt.apply();

        if (ingress.user_program != 0w16) {
            resubmit_exec(ingress.user_program);
        } else {
            tbl.apply();
        }
    }

}

control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply {
        
    }
}