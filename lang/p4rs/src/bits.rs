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
pub struct bit_slice<'a, const N: usize, const O: usize = 0>(&'a mut [u8]);

impl<'a, const N: usize, const O: usize> bit_slice<'a, N, O> {
    pub fn new(data: &'a mut [u8]) -> Result<Self, TryFromSliceError> {
        if data.len() < bytes!(N) {
            return Err(TryFromSliceError(bytes!(N)));
        }
        Ok(Self(&mut data[..bytes!(N)]))
    }

    // WARNING: Don't do this on the data path. It copies the contents.
    pub fn to_owned(&self) -> bit<N>
    where
        [u8; bytes!(N)]: Sized,
    {
        let mut result = bit::<N>::new();
        for i in 0..bit::<N>::BYTES {
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
pub struct bit<const N: usize>([u8; bytes!(N)])
where
    [u8; bytes!(N)]: Sized;

impl<const N: usize> bit<N>
where
    [u8; bytes!(N)]: Sized,
{
    const BYTES: usize = bytes!(N);
    pub const ZERO: Self = Self([u8::MIN; bytes!(N)]);

    pub fn new() -> Self {
        Self([0u8; bytes!(N)])
    }

    // TODO: it would be nice if these could be made compile time constants, but
    // my rust-fu is failing me for implementing max as a constant
    pub fn min() -> Self {
        Self([u8::MIN; bytes!(N)])
    }
    pub fn max() -> Self {
        let mut s = Self([u8::MAX; bytes!(N)]);

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

impl<const N: usize> Into<BigUint> for bit<N> 
where
    [u8; bytes!(N)]: Sized,
{
    fn into(self) -> BigUint {
        BigUint::from_bytes_be(&self.0)
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

impl<const N: usize> std::cmp::PartialEq for bit::<N> 
where
    [u8; bytes!(N)]: Sized,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<const N: usize> std::cmp::PartialEq<bit_slice<'_, N>> for bit::<N> 
where
    [u8; bytes!(N)]: Sized,
{
    fn eq(&self, other: &bit_slice<N>) -> bool {
        self.0.as_slice() == other.0
    }
}

impl<const N: usize> std::cmp::PartialOrd for bit::<N> 
where
    [u8; bytes!(N)]: Sized,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<const N: usize> std::cmp::PartialOrd<bit_slice<'_, N>> for bit::<N> 
where
    [u8; bytes!(N)]: Sized,
{
    fn partial_cmp(&self, other: &bit_slice<N>) -> Option<std::cmp::Ordering> {
        Some(self.0.as_slice().cmp(&other.0))
    }
}

// arithmetic -----------------------------------------------------------------
// TODO using bigint for now, later directly operate on bit<N>, we can probably
// heavily optimize for sizes less that 256 with specific implementations for
// specific sizes like impl std::ops::Sub for bit<1> in terms of u8 ....
// bit<100> interms of u128 ... etc

impl<const N: usize> std::ops::Sub for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Sub<u8> for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Add<u8> for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Add for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Div for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Mul for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl<const N: usize> std::ops::Shr<usize> for bit<N>
where
    [u8; bytes!(N)]: Sized,
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

impl <const N: usize> std::ops::BitAnd for bit<N>
where
    [u8; bytes!(N)]: Sized
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

impl<const N: usize> IntoIterator for bit<N>
where
    [u8; bytes!(N)]: Sized,
{
    type Item = bit<N>;
    type IntoIter = BitIntoIterator<N>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            bit: self,
        }
    }
}

pub struct BitIntoIterator<const N: usize>
where
    [u8; bytes!(N)]: Sized,
{
    bit: bit<N>,
}

impl<const N: usize> Iterator for BitIntoIterator<N>
where
    [u8; bytes!(N)]: Sized,
{
    type Item = bit<N>;
    fn next(&mut self) -> Option<Self::Item> {
        self.bit  = self.bit + 1;
        Some(self.bit)
    }
}


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
