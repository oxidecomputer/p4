struct ingress_metadata_t {
    bit<16> port;
    bool nat;
    bit<16> nat_id;
}

struct egress_metadata_t {
    bit<16> port;
    bit<128> nexthop;
    bool drop;
}

extern Checksum {
    bit<16> run<T>(in T data);
}
