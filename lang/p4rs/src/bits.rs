use crate::error::TryFromSliceError;

// This little pile of bit twiddling is ceil(N/8). Basically any power of 8 will
// not have a one in bit positions 0, 1 or 2, so just OR those all together and
// shift any one found to the 0 position and add to implement the "round up".
macro_rules! bytes {
    (N) => {
        {(N>>3)+((N&0b1)|((N&0b10)>>1)|((N&0b100)>>2))}
    };
}

#[derive(Debug)]
pub struct bit_slice<'a, const N: usize>(&'a mut [u8]);

impl<'a, const N: usize> bit_slice<'a, N> 
{
    pub fn new(data: &'a mut [u8]) -> Result<Self, TryFromSliceError>  {
        if data.len() < bytes!(N) {
            return Err(TryFromSliceError(bytes!(N)))
        }
        Ok(Self(&mut data[..bytes!(N)]))
    }

    // WARNING: Don't do this on the data path. It copies the contents.
    pub fn to_owned(&self) -> bit::<N> 
    where [u8; bytes!(N)]: Sized
    {
        let mut result = bit::<N>::new();
        for i in 0..bit::<N>::BYTES {
            result.0[i] = self.0[i];
        }
        result
    }
}

#[derive(Debug)]
pub struct bit<const N: usize>([u8;bytes!(N)])
where [u8;bytes!(N)]: Sized;

impl<const N: usize> bit<N>
where [u8;bytes!(N)]: Sized
{
    const BYTES: usize = bytes!(N);
    
    pub fn new() -> Self {
        Self([0u8; bytes!(N)])
    }
}

impl From<u8> for bit<8> {
    fn from(x: u8) -> bit<8> {
        bit::<8>([x])
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

impl std::hash::Hash for bit<8> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0[0].hash(state)
    }
}

impl<'a> std::cmp::PartialEq for bit<8> {
    fn eq(&self, other: &Self) -> bool {
        self.0[0] == other.0[0]
    }
}

impl std::cmp::Eq for bit<8> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bits_basic() {
        let mut buf: [u8;16] = [
            0x0,0x1,0x2,0x3,0x4,0x5,0x6,0x7,
            0x8,0x9,0xa,0xb,0xc,0xd,0xe,0xf,
        ];

        let bs = bit_slice::<9>(&mut buf[7..]);

        let owned_bs = bs.to_owned();

        assert_eq!(owned_bs.0, [0x7, 0x8]);
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
