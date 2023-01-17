// Copyright 2022 Oxide Computer Company

use bitvec::prelude::*;

pub fn add(a: BitVec<u8, Lsb0>, b: BitVec<u8, Lsb0>) -> BitVec<u8, Lsb0> {
    if a.len() != b.len() {
        panic!("bitvec add size mismatch");
    }

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load_be();
    let y: u128 = b.load_be();
    let z = x + y;
    let mut c = BitVec::new();
    c.resize(a.len(), false);
    c.store_be(z);
    c
}

// leaving here in case we have a need for a true arbitrary-width adder.
#[allow(dead_code)]
pub fn add_generic(
    a: BitVec<u8, Lsb0>,
    b: BitVec<u8, Lsb0>,
) -> BitVec<u8, Lsb0> {
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

#[cfg(test)]
mod tests {

    #[test]
    fn bitmath_add() {
        use super::*;
        let mut a = bitvec![mut u8, Lsb0; 0; 16];
        a.store_be(47);
        let mut b = bitvec![mut u8, Lsb0; 0; 16];
        b.store_be(74);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = add(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load_be();
        assert_eq!(cc, 47u128 + 74u128);
    }

    #[test]
    fn bitmath_add_cascade() {
        use super::*;
        let mut a = bitvec![mut u8, Lsb0; 0; 16];
        a.store_be(47);
        let mut b = bitvec![mut u8, Lsb0; 0; 16];
        b.store_be(74);
        let mut c = bitvec![mut u8, Lsb0; 0; 16];
        c.store_be(123);
        let mut d = bitvec![mut u8, Lsb0; 0; 16];
        d.store_be(9876);

        let e = add(a, add(b, add(c, d)));

        let ee: u128 = e.load_be();
        assert_eq!(ee, 47u128 + 74u128 + 123u128 + 9876u128);
    }
}
