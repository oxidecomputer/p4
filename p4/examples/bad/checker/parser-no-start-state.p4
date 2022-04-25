// This is a bad parser example. All parsers must include a "start" state which
// this one does not have. It should through a semantic checker error.

parser bad_parser(
    packet_in packet,
    out headers_t hdr,
) {
    state parse_something {
        transition accept;
    }
}
