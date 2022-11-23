# Compile and Run

In the previous section we put together a hello world P4 program. In this
section we run that program over a software ASIC called SoftNpu. One of the
capabilities of the `x4c` compiler is using P4 code directly from Rust code
and we'll be doing that in this example.

Below is a Rust program that imports the P4 code developed in the last section,
loads it onto a SoftNpu ASIC instance, and sends some packets through it.  We'll
be looking at this program piece-by-piece in the remainder of this section.

All of the programs in this book are available as buildable programs in the
[oxidecomputer/p4](https://github.com/oxidecomputer/p4) repository in the
`book/code` directory.

```rust
use tests::softnpu::{RxFrame, SoftNpu, TxFrame};
use tests::{expect_frames};

p4_macro::use_p4!(p4 = "book/code/src/bin/hello-world.p4", pipeline_name = "hello");

fn main() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);
    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy1 = npu.phy(0);
    let phy2 = npu.phy(1);

    npu.run();

    phy1.send(&[TxFrame::new(phy2.mac, 0, b"hello")])?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0, b"hello")]);

    phy2.send(&[TxFrame::new(phy1.mac, 0, b"world")])?;
    expect_frames!(phy1, &[RxFrame::new(phy2.mac, 0, b"world")]);

    Ok(())
}
```

The program starts with a few Rust imports.

```rust
use tests::softnpu::{RxFrame, SoftNpu, TxFrame};
use tests::{expect_frames};
```

This first line is the SoftNpu implementation that lives in the `test` crate of
the `oxidecomputer/p4` repository. The second is a helper macro that allows us
to make assertions about frames coming from a SoftNpu "physical" port (referred
to as a phy).

The next line is using the `x4c` compiler to translate P4 code into Rust code
and dumping that Rust code into our program. The macro literally expands into
the Rust code emitted by the compiler for the specified P4 source file.

```rust
p4_macro::use_p4!(p4 = "book/code/src/bin/hello-world.p4", pipeline_name = "hello");
```

The main artifact this produces is a Rust `struct` called `main_pipeline` which is used
in the code that comes next.

```rust
let pipeline = main_pipeline::new(2);
let mut npu = SoftNpu::new(2, pipeline, false);
let phy1 = npu.phy(0);
let phy2 = npu.phy(1);
```

This code is instantiating a pipeline object that encapsulates the logic of our
P4 program. Then a SoftNpu ASIC is constructed with two ports and our pipeline
program. SoftNpu objects provide a `phy` method that takes a port index to get a
reference to a port that is attached to the ASIC. These port objects are used to
send and receive packets through the ASIC, which uses our compiled P4 code to
process those packets.

Next we run our program on the SoftNpu ASIC.

```rust
npu.run();
```

However, this does not actually do anything until we pass some packets through
it, so lets do that.

```rust
phy1.send(&[TxFrame::new(phy2.mac, 0, b"hello")])?;
```

This code transmits an Ethernet frame through the first port of the ASIC with a
payload value of `"hello"`. The `phy2.mac` parameter of the `TxFrame` sets the
destination MAC address and the `0` for the second parameter is the ethertype
used in the outgoing Ethernet frame.

Based on the logic in our P4 program, we would expect this packet to come out
the second port. Let's test that.

```rust
expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0, b"hello")]);
```

This code reads a packet from the second ASIC port `phy2` (blocking until there
is a packet available) and asserts the following.

- The Ethernet payload is the byte string `"hello"`.
- The source MAC address is that of `phy1`.
- The ethertype is `0`.

To complete the hello world program, we do the same thing in the opposite
direction. Sending the byte string `"world"` as an Ethernet payload into port 2
and assert that it comes out port 1.

```rust
phy2.send(&[TxFrame::new(phy1.mac, 0, b"world")])?;
expect_frames!(phy1, &[RxFrame::new(phy2.mac, 0, b"world")]);
```

The `expect_frames` macro will also print payloads and the port they came from.

When we run this program we see the following.

```bash
$ cargo run --bin hello-world
   Compiling x4c-book v0.1.0 (/home/ry/src/p4/book/code)
    Finished dev [unoptimized + debuginfo] target(s) in 2.05s
     Running `target/debug/hello-world`
[phy2] hello
[phy1] world
```

## SoftNpu and Target `x4c` Use Cases.

The example above shows using `x4c` compiled code is a setting that is only
really useful for testing the logic of compiled pipelines and demonstrating how
P4 and `x4c` compiled pipelines work. This begs the question of what the target
use cases for `x4c` actually are. It also raises question, why build `x4c` in the
first place? Why not use the established reference compiler `p4c` and its
associated reference behavioral model `bmv2`?

_A key difference between `x4c` and the `p4c` ecosystem is how compilation
and execution concerns are separated. `x4c` generates free-standing pipelines
that can be used by other code, `p4c` generates JSON that is interpreted and run
by `bmv2`_.

The example above shows how the generation of free-standing runnable pipelines
can be used to test the logic of P4 programs in a lightweight way. We went from
P4 program source to actual packet processing using nothing but the Rust
compiler and package manager. The program is executable in an operating system
independent way and is a great way to get CI going for P4 programs.

The free-standing pipeline approach is not limited to self-contained use cases
with packets that are generated and consumed in-program. `x4c` generated code
conforms to a well defined
[`Pipeline`](https://oxidecomputer.github.io/p4/p4rs/index.html)
interface that can be used to run pipelines anywhere `rustc` compiled code can
run. Pipelines are even dynamically loadable through `dlopen` and the like.

The `x4c` authors have used `x4c` generated pipelines to create virtual ASICs
inside hypervisors that transit real traffic between virtual machines, as well
as P4 programs running inside zones/containers that implement NAT and tunnel
encap/decap capabilities. The mechanics of I/O are deliberately outside the
scope of `x4c` generated code. Whether you want to use DLPI, XDP, libpcap,
PF\_RING, DPDK, etc., is up to you and the harness code you write around your
pipelines!

The win with `x4c` is flexibility. You can compile a free-standing P4 pipeline
and use that pipeline wherever you see fit. The near-term use for `x4c` focuses
on development and evaluation environments. If you are building a system around
P4 programmable components, but it's not realistic to buy all the
switches/routers/ASICs at the scale you need for testing/development, `x4c` is an
option. `x4c` is also a good option for running packets through your pipelines
in a lightweight way in CI.
