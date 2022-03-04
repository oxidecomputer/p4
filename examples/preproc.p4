#include <core.p4>

#define ENTERPRISE 1701

#define STARSHIP { \
        bit<8>  e; \
        bit<16> t; \
        bit<8>  l; \
        bit<8>  r; \
        bit<1>  v; \
    }

const bit<47> enterprise = ENTERPRISE;

header starship STARSHIP
