# VLAN Switch

This example presents a simple VLAN switch program. This program allows a single
VLAN id (`vid`) to be set per port. Any packet arriving at a port with a `vid`
set must carry that `vid` in its Ethernet header or it will be dropped. We'll
refer to this as VLAN filtering. If a packet makes it past ingress filtering,
then the forwarding table of the switch is consulted to see what port to send
the packet out. We limit ourselves to a very simple switch here with a static
forwarding table. A MAC learning switch will be presented in a later example.
This switch also does not do flooding for unknown packets, it simply operates on
the lookup table it has. If an egress port is identified via a forwarding table
lookup, then egress VLAN filtering is applied. If the `vid` on the packet is
present on the egress port then the packet is forwarded out that port.

This example is comprised of two programs. A P4 data-plane program and a Rust
control-plane program.

## P4 Data-Plane Program

Let's start by taking a look at the headers for the P4 program.

```p4
header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

header vlan_h {
    bit<3> pcp;
    bit<1> dei;
    bit<12> vid;
    bit<16> ether_type;
}

struct headers_t {
    ethernet_h eth;
    vlan_h vlan;
}
```

An Ethernet frame is normally just 14 bytes with a 6 byte source and destination
plus a two byte ethertype. However, when VLAN tags are present the ethertype is
set to `0x8100` and a VLAN header follows. This header contains a 12-bit `vid`
as well as an ethertype for the header that follows.

A byte-oriented packet diagram shows how these two Ethernet frame variants line
up.

```
                     1
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8
+---------------------------+
|    src    |    dst    |et |
+---------------------------+
+-----------------------------------+
|    src    |    dst    |et |pdv|et |
+------------------------------------
```

The structure is always the same for the first 14 bytes. So we can take
advantage of that when parsing any type of Ethernet frame. Then we can use the
ethertype field to determine if we are looking at a regular Ethernet frame or a
VLAN-tagged Ethernet frame.

```p4
parser parse (
    packet_in pkt,
    out headers_t h,
    inout ingress_metadata_t ingress,
) {
    state start {
        pkt.extract(h.eth);
        if (h.eth.ether_type == 16w0x8100) { transition vlan; } 
        transition accept;
    }
    state vlan {
        pkt.extract(h.vlan);
        transition accept;
    }
}
```

This parser does exactly what we described above. First parse the first 14 bytes
of the packet as an Ethernet frame. Then conditionally parse the VLAN portion of
the Ethernet frame if the ethertype indicates we should do so. In a sense we can
think of the VLAN portion of the Ethernet frame as being it's own independent
header. We are keying our decisions based on the ethertype, just as we would for
layer 3 protocol headers.

Our VLAN switch P4 program is broken up into multiple control blocks. We'll
start with the top level control block and then dive into the control blocks it
calls into to implement the switch.

```p4
control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    vlan() vlan;
    forward() fwd;
    
    apply {
        bit<12> vid = 12w0;
        if (hdr.vlan.isValid()) {
            vid = hdr.vlan.vid;
        }

        // check vlan on ingress
        bool vlan_ok = false;
        vlan.apply(ingress.port, vid, vlan_ok);
        if (vlan_ok == false) {
            egress.drop = true;
            return;
        }

        // apply switch forwarding logic
        fwd.apply(hdr, egress);

        // check vlan on egress
        vlan.apply(egress.port, vid, vlan_ok);
        if (vlan_ok == false) {
            egress.drop = true;
            return;
        }
    }
}
```

The first thing that is happening in this program is the instantiation of a few
other control blocks.

```p4
vlan() vlan;
forward() fwd;
```

We'll be using these control blocks to implement the VLAN filtering and switch
forwarding logic. For now let's take a look at the higher level packet
processing logic of the program in the `apply` block.

The first thing we do is start by assuming there is no `vid` by setting it to
zero. The if the VLAN header is valid we assign the `vid` from the packet header
to our local `vid` variable. The `isValid` header method returns `true` if
`extract` was called on that header. Recall from the parser code above, that
`extract` is only called on `hdr.vlan` if the ethertype on the Ethernet frame is
`0x1800`.

```p4
bit<12> vid = 12w0;
if (hdr.vlan.isValid()) {
    vid = hdr.vlan.vid;
}
```

Next apply VLAN filtering logic. First an indicator variable `vlan_ok` is
initialized to false. Then we pass that indicator variable along with the port
the packet came in on and the `vid` we determined above to the VLAN control
block.

```p4
bool vlan_ok = false;
vlan.apply(ingress.port, vid, vlan_ok);
if (vlan_ok == false) {
    egress.drop = true;
    return;
}
```

Let's take a look at the VLAN control block. The first thing to note here is the
direction of parameters. The `port` and `vid` parameters are `in` parameters,
meaning that the control block can only read from them. The `match` parameter is
an `out` parameter meaning the control block can only write to it. Consider this
in the context of the code above. There we are passing in the `vlan_ok` to the
control block with the expectation that the control block will modify the value
of the variable. The `out` direction of this control block parameter is what
makes that possible.

```p4
control vlan(
    in bit<16> port,
    in bit<12> vid,
    out bool match,
) {
    action no_vid_for_port() {
        match = true;
    }

    action filter(bit<12> port_vid) { 
        if (port_vid == vid) { match = true; } 
    }
    
    table port_vlan {
        key             = { port: exact; }
        actions         = { no_vid_for_port; filter; }
        default_action  = no_vid_for_port;
    }

    apply { port_vlan.apply(); }
}
```

Let's look at this control block starting from the `table` declaration. The
`port_vlan` table has the `port` id as the single key element. There are two
possible actions `no_vid_for_port` and `filter`. The `no_vid_for_port` fires
when there is no match for the `port` id. That action unconditionally sets
`match` to true. The logic here is that if there is no VLAN configure for a port
e.g., the port is not in the table, then there is no need to do any VLAN
filtering and just pass the packet along. 

The `filter` action takes a single parameter `port_vid`. This value is populated
by the table value entry corresponding to the `port` key. There are no static
table entries in this P4 program, they are provided by a control plane program
which we'll get to in a bit. The `filter` logic tests if the `port_vid` that has
been configured by the control plane matches the `vid` on the packet. If the
test passes then `match` is set to true meaning the packet can continue
processing.

Popping back up to the top level control block. If `vlan_ok` was not set to
`true` in the `vlan` control block, then we drop the packet. Otherwise we
continue on to further processing - forwarding.

Here we are passing the entire header and egress metadata structures into the
`fwd` control block which is an instantiation of the `forward` control block
type.

```p4
fwd.apply(hdr, egress);
```

Lets take a look at the `forward` control block.

```p4
control forward(
    inout headers_t hdr,
    inout egress_metadata_t egress,
) {
    action drop() {}
    action forward(bit<16> port) { egress.port = port; }

    table fib {
        key             = { hdr.eth.dst: exact; }
        actions         = { drop; forward; }
        default_action  = drop;
    }

    apply { fib.apply(); }
}
```

This simple control block contains a table that maps Ethernet addresses to
ports. The single element key contains an Ethernet destination and the matching
action `forward` contains a single 16-bit port value.  When the Ethernet
destination matches an entry in the table, the egress metadata destination for
the packet is set to the port id that has been set for that table entry.

Note that in this control block both parameters have an `inout` direction,
meaning the control block can both read from and write to these parameters.
Like the `vlan` control block above, there are no static entries here. Entries
for the table in this control block are filled in by a control-plane program.

Popping back up the stack to our top level control block, the remaining code we
have is the following.

```p4
vlan.apply(egress.port, vid, vlan_ok);
if (vlan_ok == false) {
    egress.drop = true;
    return;
}
```

This is pretty much the same as what we did at the beginning of the apply block.
Except this time, we are passing in the egress port instead of the ingress port.
We are checking the VLAN tags not only for the ingress port, but also for the
egress port.

You can find this program in it's entirety
[here](https://github.com/oxidecomputer/p4/blob/main/book/code/src/bin/vlan-switch.p4).

## Rust Control-Plane Program

The main purpose of the Rust control plane program is to manage table entries in
the P4 program. In addition to table management, the program we'll be showing
here also instantiates and runs the P4 code over a virtual ASIC to demonstrate
the complete system working.

We'll start top down again. Here is the beginning of our Rust program.

```rust
use tests::expect_frames;
use tests::softnpu::{RxFrame, SoftNpu, TxFrame};

p4_macro::use_p4!(
    p4 = "book/code/src/bin/vlan-switch.p4",
    pipeline_name = "vlan_switch"
);

fn main() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(2);

    let m1 = [0x33, 0x33, 0x33, 0x33, 0x33, 0x33];
    let m2 = [0x44, 0x44, 0x44, 0x44, 0x44, 0x44];

    init_tables(&mut pipeline, m1, m2);
    run_test(pipeline, m2)
}
```

After imports, the first thing we are doing is calling the `use_p4!` macro. This
translates our P4 program into Rust and expands the `use_p4!` macro in place to
the generated Rust code. This results in the `main_pipeline` type that we see
instantiated in the first line of the `main` program. Then we define a few MAC
addresses that we'll get back to later. The remainder of the `main` code
performs the two functions described above. The `init_tables` function acts as a
control plane for our P4 code, setting up the VLAN and forwarding tables. The
`run_test` code executes our instantiated pipeline over a virtual ASIC, sends
some packets through it, and makes assertions about the results.

### Control Plane Code

Let's jump into the control plane code.

```rust
fn init_tables(pipeline: &mut main_pipeline, m1: [u8;6], m2: [u8;6]) {
    // add static forwarding entries
    pipeline.add_ingress_fwd_fib_entry("forward", &m1, &0u16.to_be_bytes());
    pipeline.add_ingress_fwd_fib_entry("forward", &m2, &1u16.to_be_bytes());

    // port 0 vlan 47
    pipeline.add_ingress_vlan_port_vlan_entry(
        "filter",
        0u16.to_be_bytes().as_ref(),
        47u16.to_be_bytes().as_ref(),
    );

    // sanity check the table
    let x = pipeline.get_ingress_vlan_port_vlan_entries();
    println!("{:#?}", x);

    // port 1 vlan 47
    pipeline.add_ingress_vlan_port_vlan_entry(
        "filter",
        1u16.to_be_bytes().as_ref(),
        47u16.to_be_bytes().as_ref(),
    );

}
```

The first thing that happens here is the forwarding tables are set up. We add
two entries one for each MAC address. The first MAC address maps to the first
port and the second MAC address maps to the second port.

We are using table modification methods from the Rust code that was generated
from our P4 code. A valid question is, how do I know what these are? There are
two ways.

#### Determine Based on P4 Code Structure

The naming is deterministic based on the structure of the p4 program. Table
modification functions follow the pattern
`<operation>_<control_path>_<table_name>_entry`. Where `operation` one of the
following.

- `add`
- `remove`
- `get`. 

The `control_path` is based on the names of control instances starting from the
top level ingress controller. In our P4 program, the forwarding table is named
`fwd` so that is what we see in the function above. If there is a longer chain
of controller instances, the instance names are underscore separated. Finally
the `table_name` is the name of the table in the control block. This is how we
arrive at the method name above.

```rust
pipeline.add_fwd_fib_entry(...)
```

#### Use `cargo doc`

Alternatively you can just run `cargo doc` to have Cargo generate documentation
for your crate that contains the P4-generated Rust code. This will emit Rust
documentation that includes documentation for the generated code.

For example, in the main p4 repository that contains the vlan switch example
code, when you run `cargo doc` you'll see something like this

```
$ cargo doc
[snip]
 Documenting x4c_error_codes v0.1.0 (/Users/ry/src/p4/x4c_error_codes)
 Documenting clap v3.2.23
 Documenting tests v0.1.0 (/Users/ry/src/p4/test)
 Documenting sidecar-lite v0.1.0 (/Users/ry/src/p4/lang/prog/sidecar-lite)
 Documenting p4-macro-test v0.1.0 (/Users/ry/src/p4/lang/p4-macro-test)
 Documenting x4c-book v0.1.0 (/Users/ry/src/p4/book/code)
 Documenting x4c v0.1.0 (/Users/ry/src/p4/x4c)
    Finished dev [unoptimized + debuginfo] target(s) in 15.87s
   Generated /Users/ry/src/p4/target/doc/p4_macro/index.html
   Generated /Users/ry/src/p4/target/doc/p4_macro_test/index.html
   Generated /Users/ry/src/p4/target/doc/p4_rust/index.html
   Generated /Users/ry/src/p4/target/doc/p4rs/index.html
   Generated /Users/ry/src/p4/target/doc/sidecar_lite/index.html
   Generated /Users/ry/src/p4/target/doc/tests/index.html
   Generated /Users/ry/src/p4/target/doc/x4c/index.html
   Generated /Users/ry/src/p4/target/doc/hello_world/index.html
   Generated /Users/ry/src/p4/target/doc/vlan_switch/index.html
   Generated /Users/ry/src/p4/target/doc/x4c_error_codes/index.html
```

If you open the file `target/doc/vlan_switch/index.html`. You'll see several
struct and function definitions. In particular, if you click on the
`main_pipeline` struct, you'll see methods associated with the main pipeline
like `add_ingress_fwd_fib_entry` that allow you to modify pipeline table state.

Now back to the control plane code above. You'll also notice that we are adding
key values and parameter values to the P4 tables as byte slices. At the time of
writing, `x4c` is not generating high-level table manipulation APIs so we have
to pass everything in as binary serialized data.

The semantics of these data buffers are the following.

1. Both key data and match action data (parameters) are passed in in-order.
2. Numeric types are serialized in big-endian byte order.
3. If a set of keys or a set of parameters results in a size that does not land
   on a byte-boundary, i.e. 12 bytes like we have in this example, the length of
   the buffer is rounded up to the nearest byte boundary.

After adding the forwarding entries, VLAN table entries are added in the same
manner. A VLAN with the `vid` of `47` is added to the first and second ports.
Note that we also use a table access method to get all the entries of a table
and print them out to convince ourselves our code is doing what we intend.

### Test Code

Now let's take a look at the test portion of our code.

```rust
fn run_test(
    pipeline: main_pipeline,
    m2: [u8; 6],
    m3: [u8; 6],
) -> Result<(), anyhow::Error> {
    // create and run the softnpu instance
    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy1 = npu.phy(0);
    let phy2 = npu.phy(1);
    npu.run();

    // send a packet we expect to make it through
    phy1.send(&[TxFrame::newv(m2, 0, b"blueberry", 47)])?;
    expect_frames!(phy2, &[RxFrame::newv(phy1.mac, 0x8100, b"blueberry", 47)]);

    // send 3 packets, we expect the first 2 to get filtered by vlan rules
    phy1.send(&[TxFrame::newv(m2, 0, b"poppyseed", 74)])?; // 74 != 47
    phy1.send(&[TxFrame::new(m2, 0, b"banana")])?; // no tag
    phy1.send(&[TxFrame::newv(m2, 0, b"muffin", 47)])?;
    phy1.send(&[TxFrame::newv(m3, 0, b"nut", 47)])?; // no forwarding entry
    expect_frames!(phy2, &[RxFrame::newv(phy1.mac, 0x8100, b"muffin", 47)]);

    Ok(())
}
```

The first thing we do here is create a `SoftNpu` virtual ASIC instance with 2
ports that will execute the pipeline we configured with entries in the previous
section. We get references to each ASIC port and run the ASIC.

Next we send a few packets through the ASIC to validate that our P4 program is
doing what we expect given how we have configured the tables.

The first test passes through a packet we expect to make it through the VLAN
filtering. The next test sends 4 packets in the ASIC, but we expect our P4
program to filter 3 of them out.

- The first packet has the wrong `vid`.
- The second packet has no `vid`.
- The third packet should make it through.
- The fourth packet has no forwarding entry.

#### Running the test

When we run this program we see the following

```bash
$ cargo run --bin vlan-switch
    Finished dev [unoptimized + debuginfo] target(s) in 0.11s
     Running `target/debug/vlan-switch`
[
    TableEntry {
        action_id: "filter",
        keyset_data: [
            0,
            0,
        ],
        parameter_data: [
            0,
            47,
        ],
    },
]
[phy2] blueberry
drop
drop
drop
[phy2] muffin
```

The first thing we see is our little sanity check dumping out the VLAN table
after adding a single entry. This has what we expect, mapping the port `0` to
the `vid` `47`.

Next we start sending packets through the ASIC. There are two frame constructors
in play here. `TxFrame::newv` creates an Ethernet frame with a VLAN header and
`TxFrame::new` creates just a plane old Ethernet frame. The first argument to
each frame constructor is the destination MAC address. The second argument is
the ethertype to use and the third argument is the Ethernet payload.

Next we see that our blueberry packet made it through as expected. Then we see
three packets getting dropped as we expect. And finally we see the muffin packet
coming through as expected.

You can find this program in it's entirety
[here](https://github.com/oxidecomputer/p4/blob/main/book/code/src/bin/vlan-switch.rs).
