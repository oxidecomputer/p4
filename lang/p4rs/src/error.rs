use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct TryFromSliceError(pub usize);

impl fmt::Display for TryFromSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slice not big enough for {} bits", self.0)
    }
}

impl Error for TryFromSliceError {}
