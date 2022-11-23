struct ingress_metadata_t {
    bit<16> port;
    bool nat; // XXX this should be a program specific thing
    bit<16> nat_id; // XXX this should be a program specific thing
    bool drop;
}

struct egress_metadata_t {
    bit<16> port;
    bit<128> nexthop_v6;
    bit<32> nexthop_v4;
    bool drop;
    bool broadcast;
}

extern Checksum {
    bit<16> run<T>(in T data);
}
