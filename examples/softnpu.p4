#include <core.p4>

typedef bit<4> PortId;

const PortId REAL_PORT_COUNT = 4w4;
const PortId CPU_INGRESS_PORT = 0xA;
const PortId CPU_EGRESS_PORT = 0xB;
const PortId DROP_PORT = 0xC;

struct IngressMetadata {
    PortId ingress_port;
}

struct EgressMetadata {
    PortId ingress_port;
}

parser NpuParser<H>(
    packet_in pkt,
    out H parsed_headers
);
