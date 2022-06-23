use std::net::IpAddr;
use std::collections::HashSet;

use num::bigint::BigUint;
use num::ToPrimitive;

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Key {
    Exact(BigUint),
    Range(BigUint, BigUint),
    Ternary(Ternary),
    Lpm(Prefix),
}

impl Default for Key {
    fn default() -> Self {
        Self::Ternary(Ternary::default())
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Ternary {
    DontCare,
    Value(BigUint),
    Masked(BigUint, BigUint),
}

impl Default for Ternary {
    fn default() -> Self {
        Self::DontCare
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct Prefix {
    pub addr: IpAddr,
    pub len: u8,
}

pub struct Table<const D: usize> {
    pub entries: HashSet<TableEntry<D>>,
}

impl<const D: usize> Table<D> {
    pub fn new() -> Self {
        Self{ entries: HashSet::new() }
    }

    pub fn match_selector(&self, keyset: &[BigUint; D]) -> Vec<TableEntry<D>> {
        let mut result = Vec::new();
        for entry in &self.entries {
            if keyset_matches(keyset, &entry.key) {
                result.push(entry.clone());
            }
        }
        let sorted = sort_entries(result);
        sorted 
    }
}

pub fn sort_entries<const D: usize>(mut entries: Vec<TableEntry<D>>) -> Vec<TableEntry<D>> {

    if entries.is_empty() {
        return entries;
    }

    // First determine how to sort. All the entries have the same keyset
    // structure, so it's sufficient to iterate over the keys of the first
    // entry. The basic logic we follow here is
    //
    // - If we find an lpm key sort on the prefix lenght of that dimension.
    // - Otherwise simply sort on priority.
    //
    // Notes:
    //
    // It's assumed that multiple lpm keys do not exist in a single keyset.
    // This is an implicit assumption in BVM2 
    //
    //      https://github.com/p4lang/behavioral-model/issues/698
    //
    for (i, k) in entries[0].key.iter().enumerate() {
        match k {
            Key::Lpm(_) => {
                let mut entries = prune_entries_by_lpm(i, &entries);
                sort_entries_by_priority(&mut entries);
                return entries;
            }
            _ => continue,
        }
    }

    sort_entries_by_priority(&mut entries);
    entries


}

// TODO - the data structures here are quite dumb. The point of this table
// implemntation is not efficiency, it's a simple bruteish apprach that is easy
// to move around until we nail down the, tbh, rather undefined (in terms of p4
// spec)semantics of what the relative priorities between match types in a
// common keyset are.
pub fn prune_entries_by_lpm<const D: usize>(
    d: usize,
    entries: &Vec<TableEntry<D>>,
) -> Vec<TableEntry<D>> {

    let mut longest_prefix = 0u8;

    for e in entries {
        match &e.key[d] {
            Key::Lpm(x) => {
                if x.len > longest_prefix {
                    longest_prefix = x.len
                }
            }
            _ => {}
        }
    }

    let mut result = Vec::new();
    for e in entries {
        match &e.key[d] {
            Key::Lpm(x) => {
                if x.len == longest_prefix {
                    result.push(e.clone())
                }
            }
            _ => {}
        }
    }

    result

}

pub fn sort_entries_by_priority<const D: usize>(entries: &mut Vec<TableEntry<D>>) {

    entries.sort_by(|a, b| -> std::cmp::Ordering {
        b.priority.cmp(&a.priority)
    });

}

pub fn key_matches(selector: &BigUint, key: &Key) -> bool {
    match key {
        Key::Exact(x) => {
            selector == x
        }
        Key::Range(begin, end) => {
            selector >= begin && selector <= end
        }
        Key::Ternary(t) => {
            match t {
                Ternary::DontCare => true,
                Ternary::Value(x) => selector == x,
                Ternary::Masked(x, m) => selector & m == x & m
            }
        }
        Key::Lpm(p) => {
            match p.addr {
                IpAddr::V6(addr) => {
                    assert!(p.len <= 128);
                    let key: u128 = addr.into();
                    let mask = if p.len == 128 {
                        u128::MAX
                    } else {
                        ((1u128 << p.len) - 1) << (128 - p.len)
                    };
                    let selector_v6 = selector.to_u128().unwrap();
                    selector_v6 & mask == key & mask
                }
                IpAddr::V4(addr) => {
                    assert!(p.len <= 32);
                    let key: u32 = addr.into();
                    let mask = if p.len == 32 {
                        u32::MAX
                    } else {
                        ((1u32 << p.len) - 1) << (32 - p.len)
                    };
                    let selector_v4: u32 = selector.to_u32().unwrap();
                    selector_v4 & mask == key & mask
                }
            }
        }
    }
}

pub fn keyset_matches<const D: usize>(selector: &[BigUint; D], key: &[Key; D]) -> bool {
    for i in 0..D {
        if !key_matches(&selector[i], &key[i]) {
            return false
        }
    }
    true
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TableEntry<const D: usize> {
    pub key: [Key; D],
    //pub action: fn(),
    pub priority: u32,
    pub name: String,
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::net::Ipv6Addr;

    fn contains_entry<const D: usize>(entries: &Vec<TableEntry<D>>, name: &str) -> bool {
        for e in entries {
            if e.name.as_str() == name {
                return true;
            }
        }
        false
    }

    #[test]
    /// A few tests on the following table.
    ///
    /// +--------+-------------+--------------+---------+
    /// | Action | switch addr | ingress port | is icmp |
    /// +--------+-------------+--------------+---------+
    /// | a0     | true        | _            | true    |
    /// | a1     | true        | _            | false   |
    /// | a2     | _           | 2            | _       |
    /// | a3     | _           | 4            | _       |
    /// | a4     | _           | 7            | _       |
    /// | a5     | _           | 19           | _       |
    /// | a6     | _           | 33           | _       |
    /// | a7     | _           | 47           | _       |
    /// +--------+-------------+--------------+---------+
    fn match_ternary_1() {

        let mut table = Table::<3>::new();
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::Value(BigUint::from(1u8))),
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(1u8))),
            ],
            priority: 10,
            name: "a0".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::Value(BigUint::from(1u8))),
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(0u8))),
            ],
            priority: 1,
            name: "a1".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(2u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a2".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(4u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a3".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(7u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a4".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(19u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a5".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(33u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a6".into(),
        });
        table.entries.insert(TableEntry::<3>{
            key: [
                Key::Ternary(Ternary::DontCare),
                Key::Ternary(Ternary::Value(BigUint::from(47u8))),
                Key::Ternary(Ternary::DontCare),
            ],
            priority: 1,
            name: "a7".into(),
        });

        //println!("M1 ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
        let selector = [BigUint::from(1u8), BigUint::from(99u16), BigUint::from(1u8)];
        let matches = table.match_selector(&selector);
        //println!("{:#?}", matches);
        assert_eq!(matches.len(), 1);
        assert!(contains_entry(&matches, "a0"));

        //println!("M2 ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
        let selector = [BigUint::from(1u8), BigUint::from(47u16), BigUint::from(1u8)];
        let matches = table.match_selector(&selector);
        //println!("{:#?}", matches);
        assert_eq!(matches.len(), 2);
        assert!(contains_entry(&matches, "a0"));
        assert!(contains_entry(&matches, "a7"));
        // check priority
        assert_eq!(matches[0].name.as_str(), "a0");

    }

    fn lpm(name: &str, addr: &str, len: u8) -> TableEntry::<1> {
        TableEntry::<1>{
            key: [ Key::Lpm(Prefix{ addr: addr.parse().unwrap(), len: len }) ],
            priority: 1,
            name: name.into(),
        }
    }

    #[test]
    /// A few tests on the following table.
    ///
    /// +--------+--------------------------+
    /// | Action | Prefix                   |
    /// +--------+--------------------------+
    /// | a0     | fd00:4700::/24           |
    /// | a1     | fd00:4701::/32           |
    /// | a2     | fd00:4702::/32           |
    /// | a3     | fd00:4701:0001::/48      |
    /// | a4     | fd00:4701:0002::/48      |
    /// | a5     | fd00:4702:0001::/48      |
    /// | a6     | fd00:4702:0002::/48      |
    /// | a7     | fd00:4701:0001:0001::/64 |
    /// | a8     | fd00:4701:0001:0002::/64 |
    /// | a9     | fd00:4702:0001:0001::/64 |
    /// | a10    | fd00:4702:0001:0002::/64 |
    /// | a11    | fd00:4701:0002:0001::/64 |
    /// | a12    | fd00:4701:0002:0002::/64 |
    /// | a13    | fd00:4702:0002:0001::/64 |
    /// | a14    | fd00:4702:0002:0002::/64 |
    /// | a15    | fd00:1701::/32           |
    /// +--------+----------------+---------+
    fn match_lpm_1() {

        let mut table = Table::<1>::new();
        table.entries.insert(lpm("a0", "fd00:4700::", 24));
        table.entries.insert(lpm("a1", "fd00:4701::", 32));
        table.entries.insert(lpm("a2", "fd00:4702::", 32));
        table.entries.insert(lpm("a3", "fd00:4701:0001::", 48));
        table.entries.insert(lpm("a4", "fd00:4701:0002::", 48));
        table.entries.insert(lpm("a5", "fd00:4702:0001::", 48));
        table.entries.insert(lpm("a6", "fd00:4702:0002::", 48));
        table.entries.insert(lpm("a7", "fd00:4701:0001:0001::", 64));
        table.entries.insert(lpm("a8", "fd00:4701:0001:0002::", 64));
        table.entries.insert(lpm("a9", "fd00:4702:0001:0001::", 64));
        table.entries.insert(lpm("a10", "fd00:4702:0001:0002::", 64));
        table.entries.insert(lpm("a11", "fd00:4701:0002:0001::", 64));
        table.entries.insert(lpm("a12", "fd00:4701:0002:0002::", 64));
        table.entries.insert(lpm("a13", "fd00:4702:0002:0001::", 64));
        table.entries.insert(lpm("a14", "fd00:4702:0002:0002::", 64));
        table.entries.insert(lpm("a15", "fd00:1701::", 32));

        let addr: Ipv6Addr = "fd00:4700::1".parse().unwrap();
        let selector = [BigUint::from(u128::from_be_bytes(addr.octets()))];
        let matches = table.match_selector(&selector);
        //println!("{:#?}", matches);
        assert_eq!(matches.len(), 1);
        assert!(contains_entry(&matches, "a0"));

        let addr: Ipv6Addr = "fd00:4800::1".parse().unwrap();
        let selector = [BigUint::from(u128::from_be_bytes(addr.octets()))];
        let matches = table.match_selector(&selector);
        assert_eq!(matches.len(), 0);
        //println!("{:#?}", matches);

        let addr: Ipv6Addr = "fd00:4702:0002:0002::1".parse().unwrap();
        let selector = [BigUint::from(u128::from_be_bytes(addr.octets()))];
        let matches = table.match_selector(&selector);
        //println!("{:#?}", matches);
        assert_eq!(matches.len(), 1); // only one longest prefix match
        assert!(contains_entry(&matches, "a14"));
        // longest prefix first
        assert_eq!(matches[0].name.as_str(), "a14");

    }

    fn tlpm(
        name: &str,
        addr: &str,
        len: u8,
        zone: Ternary,
        priority: u32,
    ) -> TableEntry::<2> {
        TableEntry::<2>{
            key: [
                Key::Lpm(Prefix{ addr: addr.parse().unwrap(), len: len }),
                Key::Ternary(zone),
            ],
            priority,
            name: name.into(),
        }
    }

    #[test]
    fn match_lpm_ternary_1() {

        let table = Table::<2>{
            entries: HashSet::from([
                 tlpm("a0", "fd00:1::", 64, Ternary::DontCare, 1),
                 tlpm("a1", "fd00:1::", 64, Ternary::Value(1u16.into()), 10),
                 tlpm("a2", "fd00:1::", 64, Ternary::Value(2u16.into()), 10),
                 tlpm("a3", "fd00:1::", 64, Ternary::Value(3u16.into()), 10),
            ])
        };

        let dst: Ipv6Addr = "fd00:1::1".parse().unwrap();
        let selector = [
            BigUint::from(u128::from_be_bytes(dst.octets())),
            BigUint::from(0u16),
        ];
        let matches = table.match_selector(&selector);
        println!("zone-0: {:#?}", matches);
        let selector = [
            BigUint::from(u128::from_be_bytes(dst.octets())),
            BigUint::from(2u16),
        ];
        let matches = table.match_selector(&selector);
        println!("zone-2: {:#?}", matches);


    }

}
