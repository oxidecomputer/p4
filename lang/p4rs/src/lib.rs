#![allow(incomplete_features)]
#![allow(non_camel_case_types)]

use std::fmt;
use std::net::IpAddr;

pub use error::TryFromSliceError;

use bitvec::prelude::*;

pub mod error;
//pub mod hicuts;
//pub mod rice;
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
impl<'a> Into<u16> for Bit<'a, 16> {
    fn into(self) -> u16 {
        u16::from_be_bytes([self.0[0], self.0[1]])
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
pub struct packet_in<'a> {
    /// The underlying data. Owned by an external, memory-mapped packet ring.
    pub data: &'a [u8],

    /// Extraction index. Everything before `index` has been extracted already.
    /// Only data after `index` is eligble for extraction. Extraction is always
    /// for contiguous segments of the underlying packet ring data.
    pub index: usize,
}

pub struct packet_out<'a> {
    pub header_data: Vec<u8>,
    pub payload_data: &'a [u8],
}

pub trait Pipeline {
    /// Process a packet for the specified port optionally producing an output
    /// packet and output port number.
    fn process_packet<'a>(
        &mut self,
        port: u8,
        pkt: &mut packet_in<'a>,
    ) -> Option<(packet_out<'a>, u8)>;
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
    let mut aligned = x.clone();
    aligned.force_align();
    let buf = aligned.as_raw_slice();
    match buf.len() {
        0 => "∅".into(),
        1 => {
            let v = buf[0];
            format!("0x{:02x}", v)
        }
        2 => {
            let v = u16::from_be_bytes(buf.try_into().unwrap());
            format!("0x{:04x}", v)
        }
        4 => {
            let v = u32::from_be_bytes(buf.try_into().unwrap());
            format!("0x{:08x}", v)
        }
        8 => {
            let v = u64::from_be_bytes(buf.try_into().unwrap());
            format!("0x{:016x}", v)
        }
        16 => {
            let v = u128::from_be_bytes(buf.try_into().unwrap());
            format!("{:032x}", v)
        }
        _ => {
            let v = buf
                .iter()
                .map(|x| format!("{:02x}", x))
                .collect::<Vec<String>>()
                .join("");
            format!("{}", v)
        }
    }
}

pub fn extract_exact_key(
    keyset_data: &Vec<u8>,
    offset: usize,
    len: usize,
) -> table::Key {
    table::Key::Exact(num::BigUint::from_bytes_be(
        &keyset_data[offset..offset + len],
    ))
}

pub fn extract_ternary_key(
    _keyset_data: &Vec<u8>,
    _offset: usize,
    _len: usize,
) -> table::Key {
    todo!();
}

pub fn extract_lpm_key(
    keyset_data: &Vec<u8>,
    offset: usize,
    _len: usize,
) -> table::Key {
    let (addr, len) = match keyset_data.len() {
        // IPv4
        5 => {
            let data: [u8; 4] = keyset_data.as_slice()[offset..offset + 4]
                .try_into()
                .unwrap();
            (IpAddr::from(data), keyset_data[offset + 4])
        }
        // IPv6
        17 => {
            let data: [u8; 16] = keyset_data.as_slice()[offset..offset + 16]
                .try_into()
                .unwrap();
            (IpAddr::from(data), keyset_data[offset + 16])
        }
        x => {
            panic!("add router table entry: unknown action id {}, ignoring", x);
        }
    };

    table::Key::Lpm(table::Prefix { addr, len })
}

pub fn extract_bool_action_parameter(
    parameter_data: &Vec<u8>,
    offset: usize,
) -> bool {
    parameter_data[offset] == 1
}

pub fn extract_bit_action_parameter(
    parameter_data: &Vec<u8>,
    offset: usize,
    size: usize,
) -> BitVec<u8, Msb0> {
    let size = size >> 3;
    BitVec::from_slice(&parameter_data[offset..offset + size])
}
