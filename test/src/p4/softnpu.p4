struct IngressMetadata {
    bit<8> port;
    bool nat;
    bit<16> nat_id;
}

struct EgressMetadata {
    bit<8> port;
    bit<128> nexthop;
    bool drop;
}

extern Checksum {
    bit<16> run<T>(in T data);
}
