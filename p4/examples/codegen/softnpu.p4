struct ingress_metadata_t {
    bit<16> port;
    bool nat;
    bit<16> nat_id;
    bool drop;
}

struct egress_metadata_t {
    bit<16> port;
    bit<128> nexthop;
    bool drop;
    bool broadcast;
}

extern Checksum {
    bit<16> run<T>(in T data);
}

/* TODO
enum counter_type_t {
    PACKETS,
    BYTES,
    PACKETS_AND_BYTES
}
*/

extern TableEntryCounter {
    /*TODO TableEntryCounter(counter_type_t type);*/
    void count();
}

/*
extern Counter<W, I> {
    Counter(bit<32> size, TODO counter_type_t type);
    void count(in bit<32> index);
}
*/
