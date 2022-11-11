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
`x4c` usage see `x4c --help`. Generated rust programs do have a few cargo
dependencies, see [this Cargo.toml](lang/prog/sidecar-lite/Cargo.toml) to see
what the current requirements are.

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

## Design

There are several major components to this repository each described below.

### Compiler Front End

This includes the following.

- preprocessor
- lexer
- parser
- high-level intermediate representation (hlir)
- abstract syntax tree (ast)

This code lives in the [p4](p4) directory. The lexers and parsers are hand
written with the intent of providing maximum flexibility to provide the best
possible front end user experience.

### Code Generation

Code generators take in an ast and hlir and generate runnable code. Currently
there is only one code generator that produces Rust code in
[codegen/rust/src](codegen/rust/src).

The rust code generator is broken down into sub-generators that focus on
particular P4 language elements such as control blocks, parsers, headers etc.
The Rust code generation mechanisms heavily leverage the 
[quote](https://github.com/dtolnay/quote) crate.

Generated code is not intended to be completely standalone. There is a support
library [p4rs](lang/p4rs) that is used by all generated programs. This library
contains common types and functions used by generated code.

### Command Line Compiler

The [`x4c`](x4c) program provides a command line interface for compiling P4
programs. Currently an `out.rs` file is generated on a successful compilation
run. In the future when more targets are supported, output will be target
specific.

## Contributing

Contributions are welcome. This is still early days and there is lots of ground
to cover in P4 spec coverage, type checking, static analysis and more.

Incremental advances and bug fixes are welcome via issues or pull requests.
Please make sure new code passes existing [tests](test) before submitting a PR
for review. If you are adding new functionality, please add to an existing test
or create a new test that exercises that functionality. All PRs must pass CI
before being accepted.

For large contributions such as design changes or new compiler targets, please
reach out to discuss.
