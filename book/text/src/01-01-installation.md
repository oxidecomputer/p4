# Installation

## Rust

The first thing we'll need to do is install Rust. We'll be using a tool called
[rustup](https://rustup.rs/). On Unix/Linux like platforms, simply run the
following from your terminal. For other platforms see the rustup docs.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

It may be necessary to restart your shell session after installing Rust.

## `x4c`

Now we will install the `x4c` compiler using the rust `cargo` tool.

```bash
cargo install --git https://github.com/oxidecomputer/p4 x4c
```

You should now be able to run `x4c`.

```
x4c --help
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

That's it! We're now ready to dive into P4 code.
