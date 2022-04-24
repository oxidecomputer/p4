parser bad_parser(
    packet_in packet,
    out headers_t hdr,
) {
    state parse_something {
        transition accept;
    }
}
