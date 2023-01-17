// Copyright 2022 Oxide Computer Company

use bitvec::prelude::*;

pub struct Checksum {}

impl Checksum {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(
        &self,
        elements: &[&dyn crate::checksum::Checksum],
    ) -> BitVec<u8, Lsb0> {
        let mut csum: u16 = 0;
        for e in elements {
            let c: u16 = e.csum().load();
            csum += c;
        }
        let mut result = bitvec![u8, Lsb0; 0u8, 16];
        result.store(csum);
        result
    }
}

impl Default for Checksum {
    fn default() -> Self {
        Self::new()
    }
}
