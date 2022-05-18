#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![allow(non_camel_case_types)]

use std::fmt;
use std::ptr::slice_from_raw_parts_mut;

pub use bits::bit;
pub use bits::bit_slice;
pub use error::TryFromSliceError;

pub mod bits;
pub mod error;
pub mod hicuts;
pub mod hicuts_gen;

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
    pub data: &'a mut [u8],

    /// Extraction index. Everything before `index` has been extracted already.
    /// Only data after `index` is eligble for extraction. Extraction is always
    /// for contiguous segments of the underlying packet ring data.
    pub index: usize,
}

#[derive(Debug)]
pub struct Ethernet<'a> {
    pub dst: Option<&'a mut [u8]>,
    pub src: Option<&'a mut [u8]>,
    pub ethertype: Option<&'a mut [u8]>,
}

/// A fixed length header trait.
pub trait Header<'a> {
    fn new() -> Self;
    fn size() -> usize;
    fn set(&mut self, buf: &'a mut [u8]) -> Result<(), TryFromSliceError>;
}

/// A variable length header trait.
pub trait VarHeader<'a> {
    fn new(buf: &'a mut [u8]) -> Result<Self, TryFromSliceError>
    where
        Self: Sized;
    fn set(&mut self, buf: &'a mut [u8]) -> Result<usize, TryFromSliceError>;
}

impl<'a> Header<'a> for Ethernet<'a> {
    fn new() -> Self {
        Self {
            src: None,
            dst: None,
            ethertype: None,
        }
    }

    fn size() -> usize {
        14
    }

    fn set(&mut self, buf: &'a mut [u8]) -> Result<(), TryFromSliceError> {
        if buf.len() < 14 {
            return Err(TryFromSliceError(buf.len()));
        }
        unsafe {
            self.dst =
                Some(&mut *slice_from_raw_parts_mut(buf.as_mut_ptr(), 6));
            self.src = Some(&mut *slice_from_raw_parts_mut(
                buf.as_mut_ptr().add(6),
                6,
            ));
            self.ethertype = Some(&mut *slice_from_raw_parts_mut(
                buf.as_mut_ptr().add(12),
                2,
            ));
        }
        Ok(())
    }
}

impl<'a> packet_in<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data, index: 0 }
    }

    // TODO: this function signature is a bit unforunate in the sense that the
    // p4 compiler generates call sites based on a p4 `packet_in` extern
    // definition. But based on that definition, there is no way for the
    // compiler to know that this function returns a result that needs to be
    // interrogated. In fact, the signature for packet_in::extract from the p4
    // standard library requires the return type to be `void`, so this signature
    // is in direct contradiction to that.
    pub fn extract<H: Header<'a>>(
        &mut self,
        h: &mut H,
    ) -> Result<(), TryFromSliceError> {
        // The crux of the situation here is we have a reference to mutable
        // data, and we (packet_in) do not own that mutable data, so the only
        // way we can give someone else a mutable reference to that data is by
        // moving it. However, we cannot move a reference out of ourself. So the
        // following does not work.
        //
        //   h.set(self.0);
        //
        // The outcome we are after here is giving the Header (h) shared mutable
        // access to the underlying `packet_in` data. This is not allowed with
        // references. Only one mutable reference to the same data can exist at
        // a time. This is a foundational Rust memory saftey rule to prevent
        // data races.
        //
        //
        // ... but the following trick works, this is what the `slice::split_at`
        // method does. And what we are doing here is actually quite similar. We
        // split the underlying buffer at `self.index + H::size()` and give the
        // caller a mutable reference to the segment `[self.index..H::size()]`
        // retaining `[H::size()..]` ourselves. Anything before `self.index` has
        // already been given out to some other `H` instance.
        //
        let n = H::size();
        let shared_mut = unsafe {
            &mut *std::ptr::slice_from_raw_parts_mut(
                self.data.as_mut_ptr(),
                self.index + n,
            )
        };
        h.set(shared_mut)?;
        self.index += n;
        Ok(())
        //
        // Maybe a Cell is better here? Can we move a reference with a Cell? I
        // don't want to use a RefCell and take the locking hit. This is a hot
        // data path, locking on every packet is not an option.
        //
        // The thought to just use split_at_mut comes up. However, that hits
        // similar lifetime issues as we would need to borrow Self::data for 'a.
        //
        //   let (extracted, remaining) = self.data.split_at_mut(n);
        //   self.data = remaining;
        //   h.set(shared_mut);
    }

    // This is the same as extract except we return a new header instead of
    // modifying an existing one.
    pub fn extract_new<H: Header<'a>>(
        &mut self,
    ) -> Result<H, TryFromSliceError> {
        let n = H::size();
        let shared_mut = unsafe {
            &mut *std::ptr::slice_from_raw_parts_mut(
                self.data.as_mut_ptr(),
                self.index + n,
            )
        };
        self.index += n;
        let mut x = H::new();
        x.set(shared_mut)?;
        Ok(x)
    }
}
