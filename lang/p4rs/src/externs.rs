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
    ) -> BitVec<u8, Msb0> {
        let mut csum: u16 = 0;
        for e in elements {
            let c: u16 = e.csum().load();
            csum += c;
        }
        let mut result = bitvec![u8, Msb0; 0u8, 16];
        result.store(csum);
        result
    }
}

impl Default for Checksum {
    fn default() -> Self {
        Self::new()
    }
}

///
/// 

pub trait EgressPort {
    fn set_resubmit_port(&mut self, port: BitVec<u8, Msb0>);
}

pub struct ResubmitExec {}

impl ResubmitExec {
    pub fn new() -> Self {
        Self {}
    }

    pub fn jump<T: EgressPort>(
        &self,
        egress: &mut T,
        program_id: BitVec<u8, Msb0>,
    ) {
        const RESUBMIT_BASE: u16 = 0x8000;
        let pid: u16 = program_id.load_le();
        let mut x = bitvec![mut u8, Msb0; 0; 16];
        x.store_le(RESUBMIT_BASE.wrapping_add(pid));
        egress.set_resubmit_port(x);
    }
}

impl Default for ResubmitExec {
    fn default() -> Self {
        Self::new()
    }
}
