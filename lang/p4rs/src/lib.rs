//! This is the runtime support create for `x4c` generated programs.
//!
//! The main abstraction in this crate is the [`Pipeline`] trait. Rust code that
//! is generated by `x4c` implements this trait. A `main_pipeline` struct is
//! exported by the generated code that implements [`Pipeline`]. Users can wrap
//! the `main_pipeline` object in harness code to provide higher level
//! interfaces for table manipulation and packet i/o.
//!
//! ```rust
//! use p4rs::{packet_in, packet_out, Pipeline};
//! use std::net::Ipv6Addr;
//!
//! struct Handler {
//!     pipe: Box<dyn Pipeline>
//! }
//!
//! impl Handler {
//!     /// Create a new pipeline handler.
//!     fn new(pipe: Box<dyn Pipeline>) -> Self {
//!         Self{ pipe }
//!     }
//!
//!     /// Handle a packet from the specified port. If the pipeline produces
//!     /// an output result, send the processed packet to the output port
//!     /// returned by the pipeline.
//!     fn handle_packet(&mut self, port: u16, pkt: &[u8]) {
//!         let mut input = packet_in::new(pkt);
//!         if let Some((mut out_pkt, out_port)) =
//!             self.pipe.process_packet(port, &mut input) {
//!
//!             let mut out = out_pkt.header_data.clone();
//!             out.extend_from_slice(out_pkt.payload_data);
//!
//!             self.send_packet(out_port, &out);
//!
//!         }
//!     }
//!
//!     /// Add a routing table entry. Packets for the provided destination will
//!     /// be sent out the specified port.
//!     fn add_router_entry(&mut self, dest: Ipv6Addr, port: u16) {
//!         self.pipe.add_table_entry(
//!             "ingress.router.ipv6_routes", // qualified name of the table
//!             "forward_out_port",           // action to invoke on a hit
//!             &dest.octets(),
//!             &port.to_be_bytes(),
//!         );
//!     }
//!
//!     /// Send a packet out the specified port.
//!     fn send_packet(&self, port: u16, pkt: &[u8]) {
//!         // send the packet ...
//!     }
//! }
//! ```
//!
#![allow(incomplete_features)]
#![allow(non_camel_case_types)]

use std::fmt;
use std::net::IpAddr;

pub use error::TryFromSliceError;
use serde::{Deserialize, Serialize};

use bitvec::prelude::*;

pub mod error;
//pub mod hicuts;
//pub mod rice;
pub mod bitmath;
pub mod checksum;
pub mod externs;
pub mod table;

#[derive(Debug)]
pub struct Bit<'a, const N: usize>(pub &'a [u8]);

impl<'a, const N: usize> Bit<'a, N> {
    //TODO measure the weight of returning TryFromSlice error versus just
    //dropping and incrementing a counter. Relying on dtrace for more detailed
    //debugging.
    pub fn new(data: &'a [u8]) -> Result<Self, TryFromSliceError> {
        let required_bytes = if N & 7 > 0 { (N >> 3) + 1 } else { N >> 3 };
        if data.len() < required_bytes {
            return Err(TryFromSliceError(N));
        }
        Ok(Self(&data[..required_bytes]))
    }
}

impl<'a, const N: usize> fmt::LowerHex for Bit<'a, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for x in self.0 {
            fmt::LowerHex::fmt(&x, f)?;
        }
        Ok(())
    }
}

// TODO more of these for other sizes
impl<'a> From<Bit<'a, 16>> for u16 {
    fn from(b: Bit<'a, 16>) -> u16 {
        u16::from_be_bytes([b.0[0], b.0[1]])
    }
}

// TODO more of these for other sizes
impl<'a> std::hash::Hash for Bit<'a, 8> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0[0].hash(state);
    }
}

impl<'a> std::cmp::PartialEq for Bit<'a, 8> {
    fn eq(&self, other: &Self) -> bool {
        self.0[0] == other.0[0]
    }
}

impl<'a> std::cmp::Eq for Bit<'a, 8> {}

/// Every packet that goes through a P4 pipeline is represented as a `packet_in`
/// instance. `packet_in` objects wrap an underlying mutable data reference that
/// is ultimately rooted in a memory mapped region containing a ring of packets.
#[derive(Debug)]
pub struct packet_in<'a> {
    /// The underlying data. Owned by an external, memory-mapped packet ring.
    pub data: &'a [u8],

    /// Extraction index. Everything before `index` has been extracted already.
    /// Only data after `index` is eligble for extraction. Extraction is always
    /// for contiguous segments of the underlying packet ring data.
    pub index: usize,
}

#[derive(Debug)]
pub struct packet_out<'a> {
    pub header_data: Vec<u8>,
    pub payload_data: &'a [u8],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TableEntry {
    pub action_id: String,
    pub keyset_data: Vec<u8>,
    pub parameter_data: Vec<u8>,
}

pub trait Pipeline: Send {
    /// Process a packet for the specified port optionally producing an output
    /// packet and output port number.
    fn process_packet<'a>(
        &mut self,
        port: u16,
        pkt: &mut packet_in<'a>,
    ) -> Option<(packet_out<'a>, u16)>;

    //TODO use struct TableEntry?
    /// Add an entry to a table identified by table_id.
    fn add_table_entry(
        &mut self,
        table_id: &str,
        action_id: &str,
        keyset_data: &[u8],
        parameter_data: &[u8],
    );

    /// Remove an entry from a table identified by table_id.
    fn remove_table_entry(&mut self, table_id: &str, keyset_data: &[u8]);

    /// Get all the entries in a table.
    fn get_table_entries(&self, table_id: &str) -> Option<Vec<TableEntry>>;

    /// Get a list of table ids
    fn get_table_ids(&self) -> Vec<&str>;
}

/// A fixed length header trait.
pub trait Header {
    fn new() -> Self;
    fn size() -> usize;
    fn set(&mut self, buf: &[u8]) -> Result<(), TryFromSliceError>;
    fn set_valid(&mut self);
    fn set_invalid(&mut self);
    fn is_valid(&self) -> bool;
    fn to_bitvec(&self) -> BitVec<u8, Msb0>;
}

impl<'a> packet_in<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0 }
    }

    // TODO: this function signature is a bit unforunate in the sense that the
    // p4 compiler generates call sites based on a p4 `packet_in` extern
    // definition. But based on that definition, there is no way for the
    // compiler to know that this function returns a result that needs to be
    // interrogated. In fact, the signature for packet_in::extract from the p4
    // standard library requires the return type to be `void`, so this signature
    // cannot return a result without the compiler having special knowledge of
    // functions that happen to be called "extract".
    pub fn extract<H: Header>(&mut self, h: &mut H) {
        //TODO what if a header does not end on a byte boundary?
        let n = H::size();
        let start = if self.index > 0 { self.index >> 3 } else { 0 };
        match h.set(&self.data[start..start + (n >> 3)]) {
            Ok(_) => {}
            Err(e) => {
                //TODO better than this
                println!("packet extraction failed: {}", e);
            }
        }
        self.index += n;
        h.set_valid();
    }

    // This is the same as extract except we return a new header instead of
    // modifying an existing one.
    pub fn extract_new<H: Header>(&mut self) -> Result<H, TryFromSliceError> {
        let n = H::size();
        let start = if self.index > 0 { self.index >> 3 } else { 0 };
        self.index += n;
        let mut x = H::new();
        x.set(&self.data[start..start + (n >> 3)])?;
        Ok(x)
    }
}

//XXX: remove once classifier defined in terms of bitvecs
pub fn bitvec_to_biguint(bv: &BitVec<u8, Msb0>) -> num::BigUint {
    let u = num::BigUint::from_bytes_be(bv.as_raw_slice());
    //println!("{:x?} -> {:x}", bv.as_raw_slice(), u);
    u
}

pub fn bitvec_to_ip6addr(bv: &BitVec<u8, Msb0>) -> std::net::IpAddr {
    let arr: [u8; 16] = bv.as_raw_slice().try_into().unwrap();
    std::net::IpAddr::V6(std::net::Ipv6Addr::from(arr))
}

#[repr(C, align(16))]
pub struct AlignedU128(pub u128);

pub fn int_to_bitvec(x: i128) -> BitVec<u8, Msb0> {
    //let mut bv = BitVec::<u8, Msb0>::new();
    let mut bv = bitvec![mut u8, Msb0; 0; 128];
    bv.store(x);
    bv
}

pub fn dump_bv(x: &BitVec<u8, Msb0>) -> String {
    if x.is_empty() {
        "∅".into()
    } else {
        let v: u128 = x.load_be();
        format!("{:x}", v)
    }
}

pub fn extract_exact_key(
    keyset_data: &[u8],
    offset: usize,
    len: usize,
) -> table::Key {
    table::Key::Exact(num::BigUint::from_bytes_be(
        &keyset_data[offset..offset + len],
    ))
}

pub fn extract_range_key(
    keyset_data: &[u8],
    offset: usize,
    len: usize,
) -> table::Key {
    table::Key::Range(
        num::BigUint::from_bytes_be(&keyset_data[offset..offset + len]),
        num::BigUint::from_bytes_be(
            &keyset_data[offset + len..offset + len + len],
        ),
    )
}

pub fn extract_ternary_key(
    _keyset_data: &[u8],
    _offset: usize,
    _len: usize,
) -> table::Key {
    todo!();
}

pub fn extract_lpm_key(
    keyset_data: &[u8],
    offset: usize,
    _len: usize,
) -> table::Key {
    let (addr, len) = match keyset_data.len() {
        // IPv4
        5 => {
            let data: [u8; 4] =
                keyset_data[offset..offset + 4].try_into().unwrap();
            (IpAddr::from(data), keyset_data[offset + 4])
        }
        // IPv6
        17 => {
            let data: [u8; 16] =
                keyset_data[offset..offset + 16].try_into().unwrap();
            (IpAddr::from(data), keyset_data[offset + 16])
        }
        x => {
            panic!("add router table entry: unknown action id {}, ignoring", x);
        }
    };

    table::Key::Lpm(table::Prefix { addr, len })
}

pub fn extract_bool_action_parameter(
    parameter_data: &[u8],
    offset: usize,
) -> bool {
    parameter_data[offset] == 1
}

pub fn extract_bit_action_parameter(
    parameter_data: &[u8],
    offset: usize,
    size: usize,
) -> BitVec<u8, Msb0> {
    let size = size >> 3;
    BitVec::from_slice(&parameter_data[offset..offset + size])
}
