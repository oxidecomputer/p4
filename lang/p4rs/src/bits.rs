use crate::error::TryFromSliceError;
use num::bigint::BigUint;

// This little pile of bit twiddling is ceil(N/8). Basically any power of 8 will
// not have a one in bit positions 0, 1 or 2, so just OR those all together and
// shift any one found to the 0 position and add to implement the "round up".
macro_rules! bytes {
    ($n: expr) => {{
        ($n >> 3) + (($n & 0b1) | (($n & 0b10) >> 1) | (($n & 0b100) >> 2))
    }};
}

pub(crate) use bytes;

#[derive(Debug)]
/// A slice of N bits starting at bit offset O.
pub struct bit_slice<'a, const N: usize, const O: usize = 0>(&'a mut [u8]);

impl<'a, const N: usize, const O: usize> bit_slice<'a, N, O> {
    pub fn new(data: &'a mut [u8]) -> Result<Self, TryFromSliceError> {
        if data.len() < bytes!(N+O) {
            return Err(TryFromSliceError(bytes!(N+O)));
        }
        Ok(Self(&mut data[..bytes!(N+O)]))
    }

    // WARNING: Don't do this on the data path. It copies the contents.
    pub fn to_owned(&self) -> bit<N, O>
    where
        [u8; bytes!(N+O)]: Sized,
    {
        let mut result = bit::<N, O>::new();
        for i in 0..bit::<N,O>::BYTES {
            result.0[i] = self.0[i];
        }
        result
    }
}

/*
impl<'a, const N: usize> Into<BigUint> for Option<bit_slice<'a, N>>
where
    [u8; bytes!(N)]: Sized,
{
    fn into(self) -> BigUint {
        match self {
            Some(s) => s.to_owned().into(),
            None => BigUint::new(),
        }
    }
}
*/

#[derive(Debug, Clone, Copy)]
/// An array of N bits starting at bit offset O.
pub struct bit<const N: usize, const O: usize = 0>([u8; bytes!(N+O)])
where
    [u8; bytes!(N+O)]: Sized;

impl<const N: usize, const O: usize> bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    const BYTES: usize = bytes!(N+O);
    pub const ZERO: Self = Self([u8::MIN; bytes!(N+O)]);

    pub fn new() -> Self {
        Self([0u8; bytes!(N+O)])
    }

    // TODO: it would be nice if these could be made compile time constants, but
    // my rust-fu is failing me for implementing max as a constant
    pub fn min() -> Self {
        Self([u8::MIN; bytes!(N+O)])
    }
    pub fn max() -> Self {
        let mut s = Self([u8::MAX; bytes!(N+O)]);

        // if N is not on a byte boundary, set the highest order bit to the
        // maximum possible value within N
        let r = 0b111&N;
        if r != 0 {
            s.0[Self::BYTES-1] = ((1<<r) - 1) as u8;
        }
        s
    }

    pub fn field(&self, offset: usize, width: usize) -> Field {
        Field(&self.0[offset..offset+width])
    }

}

pub struct Field<'a>(&'a [u8]);

impl From<u8> for bit<8> {
    fn from(x: u8) -> bit<8> {
        bit::<8>([x])
    }
}

impl From<u128> for bit<8> {
    fn from(x: u128) -> bit<8> {
        assert!(x <= u8::MAX as u128);
        bit::<8>([x as u8])
    }
}

impl From<u128> for bit<128> {
    fn from(x: u128) -> bit<128> {
        assert!(x <= u128::MAX);
        bit::<128>(x.to_be_bytes())
    }
}

impl From<i128> for bit<8> {
    fn from(x: i128) -> bit<8> {
        assert!(x <= u8::MAX as i128);
        bit::<8>([x as u8])
    }
}


impl Into<u8> for bit<8> {
    fn into(self) -> u8 {
        self.0[0]
    }
}

impl Into<usize> for bit<8> {
    fn into(self) -> usize {
        self.0[0] as usize
    }
}

impl<const N: usize, const O: usize> Into<BigUint> for bit<N, O> 
where
    [u8; bytes!(N+O)]: Sized,
{
    fn into(self) -> BigUint {

        /*
        // mlsr: mask left shift right

        let mut s = self;
        let m = (N+O) % 8;
        if m != 0 {
            let mask = ((1u8 << m) - 1) << (8-m);
            println!("mask  = {:08b}", mask);
            //println!("value = {:08b}", s.0[bytes!(N+O)-1]);
            println!("value = {:?}", s.0.map(|x| format!("{:08b}", x)));
            //s.0[bytes!(N+O)-1] &= mask;
            //s.0[bytes!(N+O)-1] >>= m;
            s.0[0] &= mask;
            s.0[0] >>= m;
        }
        let mut v = BigUint::from_bytes_be(&s.0);
        v >>= O;
        v
        */

        let mut v = BigUint::from_bytes_be(&self.0);
        println!("{:016b}", v);

        let ugh: Vec::<String> =
            v.to_bytes_be().iter().map(|x| format!("{:08b}", x)).collect();
        println!("v = {:?}", ugh);

        v <<= O;
        println!("{:016b}", v);

        let ugh: Vec::<String> =
            v.to_bytes_be().iter().map(|x| format!("{:08b}", x)).collect();
        println!("v = {:?}", ugh);

        let m = bytes!(N+O)*8 - O - (N+O)%8;
        if m > 0 {
            let mask = (1usize << m) - 1;
            v &= BigUint::from(mask);

            println!("mask  = {:016b}", mask);
            println!("value = {:?}", self.0.map(|x| format!("{:08b}", x)));
        }

        println!("V= {:b}", v);

        v


        

    }
}


impl Into<std::net::IpAddr> for bit<128>
{
    fn into(self) -> std::net::IpAddr {
        std::net::IpAddr::V6(std::net::Ipv6Addr::from(self.0))
    }
}

impl Into<std::net::IpAddr> for bit<32>
{
    fn into(self) -> std::net::IpAddr {
        std::net::IpAddr::V4(std::net::Ipv4Addr::from(self.0))
    }
}

impl std::hash::Hash for bit<8> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0[0].hash(state)
    }
}

/* XXX generic version below should suffice
impl<'a> std::cmp::PartialEq for bit<8> {
    fn eq(&self, other: &Self) -> bool {
        self.0[0] == other.0[0]
    }
}
*/

impl std::cmp::Eq for bit<8> {}

// ordinals -------------------------------------------------------------------

impl<const N: usize, const O: usize> std::cmp::PartialEq for bit::<N, O> 
where
    [u8; bytes!(N+O)]: Sized,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<const N: usize, const O: usize> 
std::cmp::PartialEq<bit_slice<'_, N, O>> for bit::<N, O> 
where
    [u8; bytes!(N+O)]: Sized,
{
    fn eq(&self, other: &bit_slice<N, O>) -> bool {
        self.0.as_slice() == other.0
    }
}

impl<const N: usize, const O: usize>
std::cmp::PartialOrd for bit::<N, O> 
where
    [u8; bytes!(N+O)]: Sized,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<const N: usize, const O: usize>
std::cmp::PartialOrd<bit_slice<'_, N, O>> for bit::<N, O> 
where
    [u8; bytes!(N+O)]: Sized,
{
    fn partial_cmp(&self, other: &bit_slice<N, O>) -> Option<std::cmp::Ordering> {
        Some(self.0.as_slice().cmp(&other.0))
    }
}

// arithmetic -----------------------------------------------------------------
// TODO using bigint for now, later directly operate on bit<N>, we can probably
// heavily optimize for sizes less that 256 with specific implementations for
// specific sizes like impl std::ops::Sub for bit<1> in terms of u8 ....
// bit<100> interms of u128 ... etc

impl<const N: usize, const O: usize> std::ops::Sub for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a - b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Sub<u8> for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn sub(self, other: u8) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&[other]);
        let c = a - b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Add<u8> for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn add(self, other: u8) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&[other]);
        let c = a + b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Add for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a + b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Div for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn div(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a / b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Mul for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a * b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }
}

impl<const N: usize, const O: usize> std::ops::Shr<usize> for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Output = Self;

    fn shr(self, rhs: usize) -> Self::Output {
        let mut b = num::bigint::BigUint::from_bytes_be(&self.0);
        b >>= rhs;
        let mut result = Self::new();
        result.0.copy_from_slice(b.to_bytes_be().as_slice());
        result
    }
}

impl <const N: usize, const O: usize> std::ops::BitAnd for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized
{

    type Output = Self;
    fn bitand(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a & b;
        let mut result = Self::new();
        result.0.copy_from_slice(&c.to_bytes_be().as_slice()[0..N]);
        result
    }

}

impl<const N: usize, const O: usize> IntoIterator for bit<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Item = bit<N, O>;
    type IntoIter = BitIntoIterator<N, O>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            bit: self,
        }
    }
}

pub struct BitIntoIterator<const N: usize, const O: usize>
where
    [u8; bytes!(N+O)]: Sized,
{
    bit: bit<N, O>,
}

impl<const N: usize, const O: usize> Iterator for BitIntoIterator<N, O>
where
    [u8; bytes!(N+O)]: Sized,
{
    type Item = bit<N, O>;
    fn next(&mut self) -> Option<Self::Item> {
        self.bit  = self.bit + 1;
        Some(self.bit)
    }
}



// TODO more of these for other sizes
impl<'a> Into<u16> for bit_slice<'a, 16> {
    fn into(self) -> u16 {
        u16::from_be_bytes([self.0[0], self.0[1]])
    }
}

// TODO more of these for other sizes
impl<'a> std::hash::Hash for bit_slice<'a, 8> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0[0].hash(state);
    }
}

impl<'a> std::cmp::PartialEq for bit_slice<'a, 8> {
    fn eq(&self, other: &Self) -> bool {
        self.0[0] == other.0[0]
    }
}

impl<'a> std::cmp::Eq for bit_slice<'a, 8> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bits_basic() {
        let mut buf: [u8; 16] = [
            0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc,
            0xd, 0xe, 0xf,
        ];

        let bs = bit_slice::<9>(&mut buf[7..]);

        let owned_bs = bs.to_owned();

        assert_eq!(owned_bs.0, [0x7, 0x8]);
    }

    #[test]
    fn max() {
        
        let x = bit::<1>::max();
        assert_eq!(x.0, [0b1u8]);

        let x = bit::<2>::max();
        assert_eq!(x.0, [0b11u8]);

        let x = bit::<3>::max();
        assert_eq!(x.0, [0b111u8]);

        let x = bit::<4>::max();
        assert_eq!(x.0, [0b1111u8]);

        let x = bit::<5>::max();
        assert_eq!(x.0, [0b11111u8]);

        let x = bit::<6>::max();
        assert_eq!(x.0, [0b111111u8]);

        let x = bit::<7>::max();
        assert_eq!(x.0, [0b1111111u8]);

        let x = bit::<8>::max();
        assert_eq!(x.0, [0b11111111u8]);

        let x = bit::<9>::max();
        assert_eq!(x.0, [0b11111111u8, 0b1]);

        let x = bit::<10>::max();
        assert_eq!(x.0, [0b11111111u8, 0b11]);

        let x = bit::<11>::max();
        assert_eq!(x.0, [0b11111111u8, 0b111]);

        let x = bit::<16>::max();
        assert_eq!(x.0, [0b11111111u8, 0b11111111u8]);

        let x = bit::<19>::max();
        assert_eq!(x.0, [0b11111111u8, 0b11111111u8, 0b111u8]);

    }

    #[test]
    fn sub_byte() {

        // The following field is a byte that straddles byte boundaries
        //
        //   bit<8,4>
        //
        // A byte because N=8. The offset O=4 means that the byte starts in the
        // fourth bit of the first byte in memory. Labeling the bits of this
        // field as a-h, the layout in byte-addressable memory looks like the
        // following.
        //
        //   |....abcd|efgh....|
        //
        // In this case we only have 1 byte, so byte order does not matter. But
        // in general what direction we shift the bits in due to an offset
        // depends on the byte ordering. In networking big endian, sometimes
        // called network byte order, is most often (always?) used. With big
        // endian byte order, the first byte of a multibyte integer is the most
        // significant byte.
        //
        // Consider standard programming language hexadecimal notation
        //
        //   0xff0f
        //
        // Most programming languages use little-endian notation for integer
        // literals in hexadecimal notation. This is because most processor
        // instruction sets operate over little endian encoded integers.
        //
        // In big endian notation we would write this number as
        //
        //   0x0fff
        //
        // Now consider this number in binary, in big endian layout. Note that
        // the bits within a byte are always little endian in nature, they start
        // with the least significant bit. When we say "start" in written
        // notation that means the rightmost bit. This comes from how we write
        // decimal numbers is base ten. In the number 123, pronounced one
        // hundred twenty three, the 3 is the least significant digit.
        //
        //   |00001111|11111111|
        //
        // Getting back to our example of a field one byte in length that spans
        // byte boundaries. Let's consider the value
        //
        //   |....1111|1010....|
        //
        // To read this value as a byte we need to shift the contents into a
        // single byte. For this example it does not matter if we shift left or
        // right, as it's just one byte so byte order does not matter. Let's
        // shift right by 4 bytes. Now we have
        //
        //   |........|11111010|
        //
        // Now the first byte has the value 0xfa. Assuming for the moment that
        // the dots are zeros, reading this as a 16-bit value in hex notation we
        // have the value 0x00fa. The big endian interpretation of this would be
        // the decimal value 64000 where the little endian value is 250.
        //
        // It's clear that the big endian interpretation is not what we want
        // here, as the dotted values have nothing to do with the byte
        // straddling byte we are whittling down to. It's also true that if the
        // values in the dotted bit positions are not zeros, then the little
        // endian representation is not correct either. In this situation we
        // could just truncate the second byte and all these problems go away.
        // This is possible because our shift has landed us on a clean byte
        // boundary, but this is not always the case.
        //
        // So in general, what we need to do is two things
        // 
        // 1. Shift in the proper direction according to the byte order.
        // 2. Mask out any bits that are not a part of the value of interest.
        //
        // In the above example this ammounts to shifting left instead of right
        // and masking out the leading bits, resulting in the following
        //
        //   |11111010|........|
        //


        //let mut data: [u8;2] = [ 0b0110_1111, 0b1110_1010 ];
        let mut data: [u8;2] = [ 0b0000_1111, 0b1110_0000 ];
        let x = bit_slice::<8,4>::new(&mut data).unwrap();

        let mut y: BigUint = x.to_owned().into();
        println!("{}", y);
        assert_eq!(y, BigUint::from(0b11111010u8));
    }

}
