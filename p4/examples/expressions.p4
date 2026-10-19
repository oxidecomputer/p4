control ingress() {
    apply {
        bit<8> x = 8w1;
        bool result = x << 1 == 8w2;
    }
}
