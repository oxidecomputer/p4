use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct TryFromSliceError(pub usize);

impl fmt::Display for TryFromSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slice not big enough for {} bits", self.0)
    }
}

impl Error for TryFromSliceError {}
