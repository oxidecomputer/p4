# P4

This repository contains a work in progress P4 compiler `x4c`. The compiler does
not currently handle the entire P4 language, but is expected to evolve
organically based on the concrete needs of users toward that end.

`x4c` is written in pure Rust and currently compiles P4 programs into Rust
programs. `x4c` generated Rust code implements a `Pipeline` trait that allows
generic harnesses to be written.

To get started with generated Rust code, see the 
[Documentation](https://oxidecomputer.github.io/p4rs/index.html).

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
higher-level runtimes as desired but the user, but such runtimes are outside the
scope of this project.

- I/O handling.

How packets get from the network to pipelines, and from pipelines to the network
is up to harness code consuming compiled pipelines.

## Stretch Goals

- x86 code generation.
- RISC-V code generation.
