typedef bit<4> PortId;

const PortId REAL_PORT_COUNT = 4w4;
const PortId CPU_INGRESS_PORT = 0xA;
const PortId CPU_EGRESS_PORT = 0xB;
const PortId DROP_PORT = 0xC;

struct IngressMetadata {
    PortId port;
}

struct EgressMetadata {
    PortId port;
}

parser NpuParser<H>(
    packet_in pkt,
    out H parsed_headers
);

control NpuDeparser<H>(
    packet_out packet,
    in headers_t hdr
);

control NpuIngress<H>(
    inout H hdr,
    inout EgressMetadata ingress_meta,
    inout EgressMetadata egress_meta,
);

control NpuEgress<H>(
    inout H hdr,
    in EgressMetadata ingress_meta,
    inout EgressMetadata egress_meta,
);

package SoftNPU<H>(
    NpuParser<H> p,
    NpuIngress<H> ingress,
    NpuEgress<H> egress,
    NpuDeparser<H> deparser
);
