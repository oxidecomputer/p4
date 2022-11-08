# P4

This repository contains a work in progress P4 compiler `x4c`. The compiler does
not currently handle the entire P4 language, but is expected to evolve
organically based on the concrete needs of users toward that end.

`x4c` is written in pure Rust and currently compiles P4 programs into Rust
programs. `x4c` generated Rust code implements a `Pipeline` trait that allows
generic harnesses to be written.

## Getting started

There are two main forms of using the compiler.

1. Generating Rust code from your P4 code using the `x4c` CLI interface.
2. Importing P4 code directly into your rust code with the `use_p4!` macro.

### Compile with `x4c`

To build the `x4c` compiler simply run the following.

```
cargo build --bin x4c
```

There are no non-rust dependencies. Typically, compiling P4 code is as simple as
`x4c <path to p4 code>`. This will generate an `out.rs` file. For more advanced
`x4c` usage see `x4c --help`.

To get started with Rust code `x4c` generates, see the 
[p4rs module documentation](https://oxidecomputer.github.io/p4/p4rs/index.html).

### Using P4 directly in Rust.

To get started with the Rust macro interface, see the
[p4_macro module documentation](http://oxidecomputer.github.io/p4/p4_macro/index.html).

An example of using this approach to generate a shared library is in 
[lang/prog/sidecar-lite](lang/prog/sidecar-lite).
This code can be statically included in other programs. Automatically generated
documentation for the compiled code can be found 
[here](https://oxidecomputer.github.io/p4/sidecar_lite/index.html).
Because this crate is compiled as a shared libary, a `Pipeline` can also be
dynamically loaded using the `_main_pipeline_create` function symbol which
returns a `*mut dyn Pipeline` object.

## Goals

- Execute P4 pipeline logic anywhere Rust can execute.
- Capable of handling real-world traffic.
- Provide runtime insight into program execution through dynamic tracing.
- Emulate real P4 ASICs with enough fidelity to understand pipeline behaviors in
  a broader networked-system context.
- Provide a foundation for prototyping P4-programmable devices as virtual hardware.

## Non-Goals

- Compilation to Rust code for production purposes.

If we decide to go in the production code generation direction, it will be
targeting machine code, not another high-level language. This will allow the
generated code to be optimized in terms of the P4 abstract machine model.

- P4 Runtime Specification support.

The goal here is to compile P4 programs as pipeline objects with simple
low-level interfaces. These low-level interfaces may be wrapped with
higher-level runtimes as desired by the user, but such runtimes are outside the
scope of this project.

- I/O handling.

How packets get from the network to pipelines, and from pipelines to the network
is up to harness code consuming compiled pipelines.

## Stretch Goals

- x86 code generation.
- RISC-V code generation.
