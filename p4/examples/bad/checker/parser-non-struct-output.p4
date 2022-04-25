parser bad_parser(
    packet_in pkt,
    out int the_badness,
) {
    state start {
        transition accept;
    }
}
