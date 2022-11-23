# Hello World

Let's start out our introduction of P4 with the obligatory hello world program.

## Parsing

The first bit of programmable logic packets hit in a P4 program is a parser.
Parsers do the following.

1. Describe a state machine packet parsing.
2. Extract raw data into headers with typed fields.
3. Decide if a packet should be accepted or rejected based on parsed structure.

In the code below, we can see that parsers are defined somewhat like functions
in general-purpose programming languages. They take a set of parameters and have
a curly-brace delimited block of code that acts over those parameters.

Each of the parameters has an optional direction that we see here as `out` or
`inout`, a type, and a name. We'll get to data types in the next section for now
let's focus on what this parser code is doing.

The parameters shown here are typical of what you will see for P4 parsers. The
exact set of parameters varies depending on ASIC the P4 code is being compiled
for. But in general, there will always need to be - a packet, set of headers to
extract packet data into and a bit of metadata about the packet that the ASIC
has collected.

```p4
parser parse (
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(headers.ethernet);
        transition finish;
    }

    state finish {
        transition accept;
    }
}
```

Parsers are made up of a set of states and transitions between those states.
**_Parsers must always include a `start` state_**. Our start state extracts an
Ethernet header from the incoming packet and places it in to the `headers_t`
parameter passed to the parser. We then transition to the `finish` state where
we simply transition to the implicit `accept` state. We could have just
transitioned to `accept` from `start`, but wanted to show transitions between
user-defined states in this example.

Transitioning to the `accept` state means that the packet will be passed to a
control block for further processing. Control blocks will be covered a few
sections from now. Parsers can also transition to the implicit `reject` state.
This means that the packet will be dropped and not go on to any further
processing.

## Data Types

There are two primary data types in P4, `struct` and `header` types.

### Structs

Structs in P4 are similar to structs you'll find in general purpose programming
languages such as C, Rust, and Go. They are containers for data with typed data
fields. They can contain basic data types, headers as well as other structs.

Let's take a look at the structs in use by our hello world program.

The first is a structure containing headers for our program to extract packet
data into. This `headers_t` structure is simple and only contains one header.
However, there may be an arbitrary number of headers in the struct. We'll
discuss headers in the next section.

```p4
struct headers_t {
    ethernet_t ethernet;
}
```

The next structure is a bit of metadata provided to our parser by the ASIC. In
our simple example this just includes the port that the packet came from. So if
our code is running on a four port switch, the `port` field would take on a
value between `0` and `3` depending on which port the packet came in on.

```p4
struct ingress_metadata_t {
    bit<16> port;
}
```

As the name suggests `bit<16>` is a 16-bit value. In P4 the `bit<N>` type
commonly represents unsigned integer values. We'll get more into the primitive
data types of P4 later.

### Headers

Headers are the result of parsing packets. They are similar in nature to
structures with the following differences.

1. Headers may not contain headers.
2. Headers have a set of methods `isValid()`, `setValid()`, and `setValid()`
   that provide a means for parsers and control blocks to coordinate on the
   parsed structure of packets as they move through pipelines.

Let's take a look at the `ethernet_h` header in our hello world example.

```p4
header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}
```

This header represents a layer-2
[Ethernet frame](https://en.wikipedia.org/wiki/Ethernet_frame).
The leading octet is not present as this will be removed by most ASICs. What
remains is the MAC source and destination fields which are each 6 octets / 48
bits and the ethertype which is 2 octets.

Note also that the payload is not included here. This is important. P4 programs
typically operate on packet headers and not packet payloads. In upcoming
examples we'll go over header stacks that include headers at higher layers like
IP, ICMP and TCP.

In the parsing code above, when `pkt.extract(headers.ethernet)` is called, the
values `dst`, `src` and `ether_type` are populated from packet data and the
method `setValid()` is implicitly called on the `headers.ethernet` header.

### Control Blocks

Control blocks are where logic goes that decides what will happen to packets
that are parsed successfully. Similar to a parser block, control blocks look a
lot like functions from general purpose programming languages. The signature
(the number and type of arguments) for this control block is a bit different
than the parser block above.

The first argument `hdr`, is the output of the parser block. Note in the parser
signature there is a `out headers_t headers` parameter, and in this control
block there is a `inout headers_t hdr` parameter. The `out` direction in the
parser means that the parser writes to this parameter. The `inout` direction
in the control block means that the control both reads and writes to this
parameter.

The `ingress_metadata_t` parameter is the same parameter we saw in the parser
block.  The `egress_metadata_t` is similar to `ingress_metadata_t`. However,
our code uses this parameter to inform the ASIC about how it should treat packets
on egress. This is in contrast to the `ingress_metdata_t` parameter that is used
by the ASIC to inform our program about details of the packet's ingress.

```p4
control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
        }
        default_action = drop;
        const entries = {
            16w0 : forward(16w1);
            16w1 : forward(16w0);
        }
    }

    apply {
        tbl.apply();
    }

}
```

Control blocks are made up of tables, actions and apply blocks. When packet
headers enter a control block, the `apply` block decides what tables to run the
parameter data through. Tables are described in terms if keys and actions. A
`key` is an ordered sequence of fields that can be extracted from any of the
control parameters. In the example above we are using the `port` field from the
`ingress` parameter to decide what to do with a packet. We are not even
investigating the packet headers at all! We can indeed use header fields in
keys, and an example of doing so will come later.

When a table is applied, and there is an entry in the table that matches the key
data, the action corresponding to that key is executed. In our example we have
pre-populated our table with two entries. The first entry says, if the ingress
port is `0`, forward the packet to port `1`. The second entry says if the ingress
port is `1`, forward the packet to port `0`. These odd looking prefixes on our
numbers are width specifiers. So `16w0` reads: the value `0` with a width of 16
bits.

Every action that is specified in a table entry must be defined within the
control. In our example, the `forward` action is defined above. All this action
does is set the `port` field on the egress metadata to the provided value.

The example table also has a default action of `drop`. This action fires for all
invocations of the table over key data that has no matching entry. So for our
program, any packet coming from a port that is not `0` or `1` will be dropped.

The apply block is home to generic procedural code. In our example it's very
simple and only has an `apply` invocation for our table. However, arbitrary
logic can go in this block, we could even implement the logic of this control
without a table!

```p4
apply {
    if (ingress.port == 16w0) {
        egress.port = 16w1;
    }
    if (ingress.port == 16w1) {
        egress.port = 16w0;
    }
}
```

Which then begs the question, why have a special table construct at all. Why not
just program everything using logical primitives? Or let programmers define
their own data structures like general purpose programming languages do?

Setting the performance arguments aside for the moment, there is something
mechanically special about tables. They can be updated from outside the P4
program. In the program above we have what are called constant entries defined
directly in the P4 program. This makes presenting a simple program like this
very straight forward, but it is not the way tables are typically populated. The
focus of P4 is on data plane programming e.g., given a packet from the wire what
do we do with it? I prime example of this is packet routing and forwarding.

Both routing and forwarding are typically implemented in terms of lookup tables.
Routing is commonly implemented by longest prefix matching on the destination
address of an IP packet and forwarding is commonly implemented by exact table
lookups on layer-2 MAC addresses. How are those lookup tables populated though.
There are various different answers there. Some common ones include routing
protocols like OSPF, or BGP. Address resolution protocols like ARP and NDP. Or
even more simple answers like an administrator statically adding a route to the
system.

All of these activities involve either a stateful protocol of some sort or
direct interaction with a user. Neither of those things is possible in the P4
programming language. It's just about processing packets on the wire and the
mechanisms for keeping state between packets is extremely limited.

What P4 implementations **_do_** provide is a way for programs written in
general purpose programming languages that **_are_** capable of stateful
protocol implementation and user interaction - to modify the tables of a running
P4 program through a runtime API. We'll come back to runtime APIs soon. For now
the point is that the table abstraction allows P4 programs to remain focused on
simple, mostly-stateless packet processing tasks that can be implemented at high
packet rates and leave the problem of table management to the general purpose
programming languages that interact with P4 programs through shared table
manipulation.

## Package

The final bit to show for our hello world program is a package instantiation.
A package is like a constructor function that takes a parser and a set of
control blocks. Packages are typically tied to the ASIC your P4 code will be
executing on. In the example below, we are passing our parser and single control
block to the `SoftNPU` package. Packages for more complex ASICs may take many
control blocks as arguments.

```p4
SoftNPU(
    parse(),
    ingress()
) main;
```

## Full Program

Putting it all together, we have a complete P4 hello world program as follows.

```p4
struct headers_t {
    ethernet_h ethernet;
}

struct ingress_metadata_t {
    bit<16> port;
    bool drop;
}

struct egress_metadata_t {
    bit<16> port;
    bool drop;
    bool broadcast;
}

header ethernet_h {
    bit<48> dst;
    bit<48> src;
    bit<16> ether_type;
}

parser parse (
    packet_in pkt,
    out headers_t headers,
    inout ingress_metadata_t ingress,
){
    state start {
        pkt.extract(headers.ethernet);
        transition finish;
    }

    state finish {
        transition accept;
    }
}

control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

    action drop() { }

    action forward(bit<16> port) {
        egress.port = port;
    }

    table tbl {
        key = {
            ingress.port: exact;
        }
        actions = {
            drop;
            forward;
        }
        default_action = drop;
        const entries = {
            16w0 : forward(16w1);
            16w1 : forward(16w0);
        }
    }

    apply {
        tbl.apply();
    }

}

// We do not use an egress controller in this example, but one is required for
// SoftNPU so just use an empty controller here.
control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {

}

SoftNPU(
    parse(),
    ingress(),
    egress(),
) main;
```

This program will take any packet that has an Ethernet header showing up on port
`0`, send it out port `1`, and vice versa. All other packets will be dropped.

In the next section we'll compile this program and run some packets through it!
