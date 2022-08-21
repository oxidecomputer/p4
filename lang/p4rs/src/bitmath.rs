use bitvec::prelude::*;

pub fn add(a: BitVec<u8, Msb0>, b: BitVec<u8, Msb0>) -> BitVec<u8, Msb0> {
    if a.len() != b.len() {
        panic!("bitvec add size mismatch");
    }

    // P4 spec says width limits are architecture defined, i here by define
    // softnpu to have an architectural bit-type width limit of 128.
    let x: u128 = a.load();
    let y: u128 = b.load();
    let z = x + y;
    let mut c = BitVec::new();
    c.resize(a.len(), false);
    c.store(z);
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

#[cfg(test)]
mod tests {

    #[test]
    fn bitmath_add() {
        use super::*;
        let mut a = bitvec![mut u8, Msb0; 0; 16];
        a.store(47);
        let mut b = bitvec![mut u8, Msb0; 0; 16];
        b.store(74);

        println!("{:?}", a);
        println!("{:?}", b);
        let c = add(a, b);
        println!("{:?}", c);

        let cc: u128 = c.load();
        assert_eq!(cc, 47u128 + 74u128);
    }
}
