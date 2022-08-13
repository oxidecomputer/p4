# P4

This repository contains a P4 toolchain. The toolchain includes.

- A Rust library crate for lexing, parsing and checking P4 code.
- Code generation for:
  - [x] Rust.
  - [ ] Docs a la `cargo doc`.
  - [ ] RedHawk/RISC-V
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

    let mut eth = ethernet_t::new();
    eth.set(&buf).unwrap();

    println!("dst: {:x?}", eth.dst_addr.as_raw_slice());
    println!("src: {:x?}", eth.src_addr.as_raw_slice());
    let ethertype = u16::from_be_bytes(
        eth.ether_type.as_raw_slice().try_into().unwrap()
    );
    println!("ethertype: {:x?}", ethertype);
}

```

which will output this

```
$ ./p4-macro-test
dst: [11, 22, 33, 44, 55, 66]
src: [77, 88, 99, aa, bb, cc]
ethertype: 86dd
```

### CLI compilation

The CLI interface can be used like a normal compiler. When using the Rust
target, the generated code will implement the `Pipeline` interface that is
suitable for placing on packet i/o paths for processing.

```
$ ./target/debug/x4c --help
x4c 0.1

USAGE:
    x4c [OPTIONS] <FILENAME> [TARGET]

ARGS:
    <FILENAME>    File to compile
    <TARGET>      What target to generate code for [default: rust] [possible values: rust,
                  red-hawk, docs]

OPTIONS:
        --check          Just check code, do not compile
    -h, --help           Print help information
    -o, --out <OUT>      Filename to write generated code to [default: out.rs]
        --show-ast       Show parsed abstract syntax tree
        --show-hlir      Show high-level intermediate representation info
        --show-pre       Show parsed preprocessor info
        --show-tokens    Show parsed lexical tokens
    -V, --version        Print version information
```
