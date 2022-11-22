//FIXME there are a number of compilation issues with this file
typedef bit<4> PortId;

const PortId REAL_PORT_COUNT = 4w4;
const PortId CPU_INGRESS_PORT = 0xA;
const PortId CPU_EGRESS_PORT = 0xB;
const PortId DROP_PORT = 0xC;

struct ingress_metadata_t {
    PortId port;
}

struct egress_metadata_t {
    PortId port;
}

parser NpuParser<H>(
    packet_in pkt,
    out H parsed_headers
);

control NpuIngress<H>(
    inout H hdr,
    inout ingress_metadata_t ingress_meta,
    inout egress_metadata_t egress_meta,
);

control NpuEgress<H>(
    inout H hdr,
    inout ingress_metadata_t ingress_meta,
    inout egress_metadata_t egress_meta,
);

package SoftNPU<H>(
    NpuParser<H> p,
    NpuIngress<H> ingress,
    NpuEgress<H> ingress,
);
