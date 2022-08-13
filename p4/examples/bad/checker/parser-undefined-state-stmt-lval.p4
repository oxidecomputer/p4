// XXX import from core.p4
extern packet_in {
    void extract<T>(out T headerLvalue);
    void extract<T>(out T variableSizeHeader, in bit<32> varFieldSizeBits);
    T lookahead<T>();
    bit<32> length();  // This method may be unavailable in some architectures
    void advance(bit<32> bits);
}

struct headers_t {
    ethernet_t ethernet;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}

parser test(
    packet_in pkt,
    out headers_t headers,
) {
    state start {
        pkt.extract(headers.ethernet);
        pkt.extractX(headers.ethernet);
        pktX.extract(headers.ethernet);
        pkt.extract(headersX.ethernet);
        pkt.extract(headers.ethernetX);
    }
}
