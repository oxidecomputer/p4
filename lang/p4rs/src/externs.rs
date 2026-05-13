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

/// Marker extern for packet replication. The `replicate` method is a
/// no-op at runtime. The pipeline codegen detects calls to this extern
/// and generates the replication loop at the pipeline level (between
/// ingress and egress).
pub struct Replicate {}

impl Replicate {
    pub fn new() -> Self {
        Self {}
    }

    /// Marker call. The bitmap argument is consumed by the pipeline
    /// codegen to drive replication. This method is never invoked at
    /// runtime because the codegen elides it.
    pub fn replicate(&self, _bitmap: &BitVec<u8, Msb0>) {}
}

impl Default for Replicate {
    fn default() -> Self {
        Self::new()
    }
}
