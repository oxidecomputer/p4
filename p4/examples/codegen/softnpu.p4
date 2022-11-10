struct IngressMetadata {
    bit<16> port;
    bool nat;
    bit<16> nat_id;
}

struct EgressMetadata {
    bit<16> port;
    bit<128> nexthop;
    bool drop;
}

extern Checksum {
    bit<16> run<T>(in T data);
}
