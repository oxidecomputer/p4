#include <core.p4>
#include <dynamic_softnpu.p4>

SoftNPU(
    parse(),
    ingress(),
    egress()
) main;

struct headers_t {
    ethernet_t ethernet;
    ipv4_t ipv4;
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

parser parse(
    packet_in pkt,
    out headers_t hdr,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(hdr.ethernet);
        transition parse_ipv4;
    }

    state parse_ipv4 {
        pkt.extract(hdr.ipv4);
        transition accept;
    }
}

control configure_programs(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    ResubmitExec() rex;

    action set_user_program_id(bit<16> program_id) {
        ingress.user_program = program_id;
    }

    table src_ipv4_pkt {
        key = {
            hdr.ipv4.src_addr: lpm;
        }
        actions = {
            set_user_program_id;
        }
        default_action = NoAction;
    }

    apply {
        ingress.user_program = 16w0;
        src_ipv4_pkt.apply();

        if (ingress.user_program != 16w0) {
            rex.jump(egress, ingress.user_program);
        }
    }
}

control default_traffic_flow(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
    }

    table program {
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
        program.apply();
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    configure_programs() configure_programs;
    default_traffic_flow() default_traffic_flow;

    apply {
        configure_programs.apply(hdr, ingress, egress);

        if (ingress.user_program == 16w0) {
            default_traffic_flow.apply(hdr, ingress, egress);
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