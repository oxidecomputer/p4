///! Rust language support for P4
use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct TryFromSliceError(usize);

impl fmt::Display for TryFromSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slice not big enough for {} bits", self.0)
    }
}

impl Error for TryFromSliceError {}

#[derive(Debug)]
pub struct Bit<'a, const N: usize>(&'a [u8]);

impl<'a, const N: usize> Bit<'a, N> {

    pub fn new(data: &'a [u8]) -> Result<Self, TryFromSliceError>  {
        let required_bytes = if N & 7 > 0 {
            (N >> 3) + 1
        } else {
            N >> 3
        };
        if data.len() < required_bytes {
            return Err(TryFromSliceError(N))
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

impl<'a> Into<u16> for Bit<'a, 16> {

    fn into(self) -> u16 {
        u16::from_be_bytes([self.0[0], self.0[1]])
    }

}
