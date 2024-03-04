// Copyright 2022 Oxide Computer Company

use bitvec::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

#[derive(Clone, Debug)]
pub struct TableEntryCounter {
    pub entries: Arc<Mutex<HashMap<Vec<u8>, u128>>>,
    pub key: Option<Vec<u8>>,
}

impl Default for TableEntryCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TableEntryCounter {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            key: None,
        }
    }

    pub fn set_key(&self, value: Vec<u8>) -> Self {
        let mut ctr = self.clone();
        ctr.key = Some(value);
        ctr
    }

    pub fn count(&self) {
        let key = match &self.key {
            Some(k) => k,
            None => return,
        };
        let mut entries = self.entries.lock().unwrap();
        match entries.get_mut(key) {
            Some(e) => *e += 1,
            None => {
                entries.insert(key.clone(), 1);
            }
        }
    }
}
