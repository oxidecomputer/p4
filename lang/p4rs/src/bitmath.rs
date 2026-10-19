// Copyright 2026 Oxide Computer Company

use bitvec::prelude::*;
use std::cmp::Ordering;

pub fn add_be(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load_be();
    let y: u128 = b.load_be();
    let z = x.wrapping_add(y);
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_be(z);
    c
}

pub fn add_le(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load_le();
    let y: u128 = b.load_le();
    let z = x.wrapping_add(y);
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_le(z);
    c
}

pub fn sub_be(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());
    let x: u128 = a.load_be();
    let y: u128 = b.load_be();
    let z = x.wrapping_sub(y);
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_be(z);
    c
}

pub fn sub_le(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());
    let x: u128 = a.load_le();
    let y: u128 = b.load_le();
    let z = x.wrapping_sub(y);
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_le(z);
    c
}

// leaving here in case we have a need for a true arbitrary-width adder.
#[allow(dead_code)]
pub fn add_generic(
    a: BitVec<u8, Msb0>,
    b: BitVec<u8, Msb0>,
) -> BitVec<u8, Msb0> {
    if a.len() != b.len() {
        panic!("bitvec add size mismatch");
    }
    let mut c = BitVec::new();
    c.resize(a.len(), false);

    for i in (1..a.len()).rev() {
        let y = c[i];
        let x = a[i] ^ b[i];
        if !(a[i] | b[i]) {
            continue;
        }
        c.set(i, x ^ y);
        let mut z = (a[i] && b[i]) | y;
        for j in (1..i).rev() {
            if !z {
                break;
            }
            z = c[j];
            c.set(j, true);
        }
    }

    c
}

pub fn mod_be(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load_be();
    let y: u128 = b.load_be();
    let z = x % y;
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_be(z);
    c
}

/// Left shift `a` by `b` positions, little-endian byte order.
/// Result width matches `a`. Shifts by the operand width or more produce zero.
pub fn shl_le(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    if a.is_empty() {
        return a;
    }
    let len = a.len();
    let x: u128 = a.load_le();
    let y: u128 = if b.is_empty() { 0 } else { b.load_le() };
    let z = if y >= len as u128 { 0 } else { x << (y as u32) };
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_le(z);
    c
}

/// Right shift `a` by `b` positions, little-endian byte order.
/// Result width matches `a`. Shifts by the operand width or more produce zero.
pub fn shr_le(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    if a.is_empty() {
        return a;
    }
    let len = a.len();
    let x: u128 = a.load_le();
    let y: u128 = if b.is_empty() { 0 } else { b.load_le() };
    let z = if y >= len as u128 { 0 } else { x >> (y as u32) };
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_le(z);
    c
}

pub fn mod_le(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    let len = usize::max(a.len(), b.len());

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load_le();
    let y: u128 = b.load_le();
    let z = x % y;
    let mut c = BitVec::new();
    c.resize(len, false);
    c.store_le(z);
    c
}

/// An unsigned ordering view of a little-endian `bit<W>` value.
///
/// Implements the unsigned comparisons on `bit<W>` from P4-16 section [8.6].
/// [`BitVec`]'s own ordering compares bits lexicographically, which is wrong
/// for byte-reversed multi-byte fields.
///
/// Note: an empty bit-string compares as zero.
///
/// [8.6]: https://p4.org/wp-content/uploads/sites/53/2024/10/P4-16-spec-v1.2.5.html#sec-bit-ops
pub struct Unsigned<'a>(pub &'a BitSlice<u8, Msb0>);

impl Unsigned<'_> {
    fn value(&self) -> u128 {
        if self.0.is_empty() {
            0
        } else {
            self.0.load_le()
        }
    }
}

impl Ord for Unsigned<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value().cmp(&other.value())
    }
}

impl PartialOrd for Unsigned<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Unsigned<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Unsigned<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_orders_by_value() {
        let mut a = bitvec![u8, Msb0; 0; 16];
        a.store_le(0x0100u16);
        let mut b = bitvec![u8, Msb0; 0; 16];
        b.store_le(0x00ffu16);
        let mut one = bitvec![u8, Msb0; 0; 16];
        one.store_le(1u16);
        let zero = bitvec![u8, Msb0; 0; 16];
        let empty = BitVec::<u8, Msb0>::new();

        assert!(Unsigned(&a) > Unsigned(&b));
        assert!(Unsigned(&empty) < Unsigned(&one));
        assert!(Unsigned(&empty) == Unsigned(&zero));
    }

    #[test]
    fn bitmath_add_be() {
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_be(47);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_be(74);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = add_be(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_be();
        assert_eq!(cc, 47u128 + 74u128);
    }

    #[test]
    fn bitmath_add_le() {
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_le(47);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_le(74);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = add_le(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_le();
        assert_eq!(cc, 47u128 + 74u128);
    }

    #[test]
    fn bitmath_sub_be() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_be(74);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_be(47);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = sub_be(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_be();
        assert_eq!(cc, 74u128 - 47u128);
    }

    #[test]
    fn bitmath_sub_le() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_le(74);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_le(47);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = sub_le(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_le();
        assert_eq!(cc, 74u128 - 47u128);
    }

    #[test]
    fn bitmath_add_mixed_size() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 8];
        a.store_be(0xAB);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_be(0xCDE);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = add_be(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_be();
        assert_eq!(cc, 0xABu128 + 0xCDEu128);
    }

    #[test]
    fn bitmath_add_cascade() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_be(47);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_be(74);
        let mut c = bitvec![mut u8, Msb0; 0; 16];
        c.store_be(123);
        let mut d = bitvec![mut u8, Msb0; 0; 16];
        d.store_be(9876);

        let e = add_be(a, add_be(b, add_be(c, d)));

        let ee: u128 = e.load_be();
        assert_eq!(ee, 47u128 + 74u128 + 123u128 + 9876u128);
    }

    #[test]
    fn bitmath_add_nest() {
        use super::*;
        let mut orig_l3_len = bitvec![mut u8, Msb0; 0; 16usize];
        orig_l3_len.store_le(0xe9u128);
        let x = add_le(
            {
                let mut x = bitvec![mut u8, Msb0; 0; 16usize];
                x.store_le(14u128);
                x
            },
            add_le(
                orig_l3_len.clone(),
                add_le(
                    {
                        let mut x = bitvec![mut u8, Msb0; 0; 16usize];
                        x.store_le(8u128);
                        x
                    },
                    {
                        let mut x = bitvec![mut u8, Msb0; 0; 16usize];
                        x.store_le(8u128);
                        x
                    },
                ),
            ),
        );

        let y: u128 = x.load_le();
        assert_eq!(y, 0xe9 + 14 + 8 + 8);
    }

    #[test]
    fn bitmath_mod() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_be(47);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_be(7);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = mod_be(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_be();
        assert_eq!(cc, 47u128 % 7u128);
    }

    #[test]
    fn bitmath_shl_le() {
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_le(1u128);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_le(4u128);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = shl_le(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_le();
        assert_eq!(cc, 1u128 << 4);
    }

    #[test]
    fn bitmath_shr_le() {
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_le(0x8000u128);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_le(4u128);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = shr_le(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_le();
        assert_eq!(cc, 0x8000u128 >> 4);
    }

    #[test]
    fn bitmath_shifts_at_width_le() {
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store_le(1u128);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store_le(16u128);

        assert_eq!(shl_le(a.clone(), b.clone()).load_le::<u16>(), 0);
        assert_eq!(shr_le(a, b).load_le::<u16>(), 0);
    }

    #[test]
    fn bitmath_shifts_by_zero_width_count() {
        let mut value = bitvec![u8, Msb0; 0; 16];
        value.store_le(0x8123u16);

        assert_eq!(shl_le(value.clone(), BitVec::new()), value);
        assert_eq!(shr_le(value.clone(), BitVec::new()), value);
    }

    #[test]
    fn bitmath_shifts_zero_width_value() {
        let mut count = bitvec![u8, Msb0; 0; 8];
        count.store_le(1u8);

        for count in [BitVec::new(), count] {
            assert!(shl_le(BitVec::new(), count.clone()).is_empty());
            assert!(shr_le(BitVec::new(), count).is_empty());
        }
    }

    #[test]
    fn bitmath_shl_shr_roundtrip_le() {
        let mut a = bitvec![mut u8, Msb0; 0; 32];
        a.store_le(42u128);
        let mut b = bitvec![mut u8, Msb0; 0; 32];
        b.store_le(7u128);

        println!("{:?}", a);
        println!("{:?}", b);
        let shifted = shl_le(a, b.clone());
        println!("{:?}", shifted);
        let back = shr_le(shifted, b);
        println!("{:?}", back);

        let result: u128 = back.load_le();
        assert_eq!(result, 42u128);
    }
}
