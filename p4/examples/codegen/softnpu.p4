struct IngressMetadata {
    bit<8> port;
}

struct EgressMetadata {
    bit<8> port;
}

parser NpuParser<H>(
    packet_in pkt,
    out H parsed_headers
);

control NpuIngress<H>(
    inout H hdr,
    inout IngressMetadata ingress_meta,
    inout EgressMetadata egress_meta,
);

package SoftNPU<H>(
    NpuParser<H> p,
    NpuIngress<H> ingress,
);
