extern Checksum {
    bit<16> run<T>(in T data);
}

control helper() {
    apply { }
}

control ingress() {
    Checksum() checksum;
    helper() child;

    action use_local() {
        bit<8> action_local = 8w1;
    }

    apply {
        bit<8> apply_local = 8w1;
        use_local();
        child.apply();
    }
}
