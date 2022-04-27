# P4

This repository contains a P4 toolchain. The toolchain includes.

- A Rust library crate for lexing, parsing and checking P4 code.
- Code generation for Rust. RISC-V assembly anticipated.
- A command-line P4 compiler called `x4c`.
- Macros for using P4 directly from Rust.

## Usage

### Use P4 directly from Rust

Given this P4

```p4
parser parsadillo(packet_in pkt, out headers_t headers){
    state start { transition accept; }
}

struct headers_t {
    ethernet_t ethernet;
}

header ethernet_t {
    bit<48> dst_addr;
    bit<48> src_addr;
    bit<16> ether_type;
}
```

we can do this

```rust
p4_macro::use_p4!("ether.p4");

fn main() {
    let buf = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, // dst mac
        0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, // src mac
        0x86, 0xdd, // ipv6 ethertype
    ];

    let eth = ethernet_t::new(&buf).unwrap();

    println!("dst: {:x?}", eth.dst_addr);
    println!("src: {:x?}", eth.src_addr);
    let ethertype: u16 = eth.ether_type.into();
    println!("ethertype: {:x?}", ethertype);

}
```

which will output this

```
$ ./p4-macro-test
dst: Bit([11, 22, 33, 44, 55, 66])
src: Bit([77, 88, 99, aa, bb, cc])
ethertype: 86dd
```
