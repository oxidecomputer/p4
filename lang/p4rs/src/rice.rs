//! Rice: Ry's Incremental Cutting Engine
//!
//! Rice is based on experience implementing HiCuts. The key differentiators
//! with rice are
//!
//! - Designed to support incremental insertion and removal of rules.
//! - Cuts rules along a dimension based on rule boundaries instead of even
//!   space partitioning
//! - Takes advantage of P4 homogeneous keyset structure e.g. each key in a
//!   table has a statically known structure and match type that is the same
//!   across all keys in the table.
//!
//! ## Cutting Techniques
//!
//! The technique used to cut a keyspace along a given dimension depends on the
//! match type being used in that dimension. The match types rice supports are
//! the following
//!
//! - Exact: A packet field must match a rule value exactly.
//! - Range: A packet field must fall within a upper and lower bound.
//! - Lpm: A packet field matches the longest matching prefix in the table.
//! - Ternay: A packet field must match a rule value according to a mask.
//!
//! ### Longest Prefix Matching (lpm)
//!
//! Consider a table with the following lpm values
//!
//! ```text
//! - fd00:4700::/24
//! - fd00:4701::/32
//! - fd00:4702::/32
//! - fd00:4701:0001::/48
//! - fd00:4701:0002::/48
//! - fd00:4702:0001::/48
//! - fd00:4702:0002::/48
//! - fd00:4701:0001:0001::/64
//! - fd00:4701:0001:0002::/64
//! - fd00:4702:0001:0001::/64
//! - fd00:4702:0001:0002::/64
//! - fd00:4701:0002:0001::/64
//! - fd00:4701:0002:0002::/64
//! - fd00:4702:0002:0001::/64
//! - fd00:4702:0002:0002::/64
//! ```
//!
//! The overall space spanned by these addresses is
//!
//! ```text
//! fd00:4700:: - fd00:4700;;
//! ```
//!
//! Here the `;;` notation carries the same semantics as `::` except it implies
//! trailing ones instead of trailing zeros. Concretely
//!
//! ```text
//! fd00:4700;; = fd00:4700:ffff:ffff:ffff:ffff:ffff:ffff
//! ```
//!
//! The /24 prefix contains all of the other prefixes. Similarly, each /32
//! contains half the /48s and each /48 contains a quarter of the /64s.
//!
//! This leads to a natural prefix tree structure.
//!
//! ```text
//! - fd00:4700::/24
//! |
//! |---- fd00:4701::/32
//! |   |
//! |   |---- fd00:4701:0001::/48
//! |   |   |
//! |   |   |---- fd00:4701:0001:0001::/64
//! |   |   |---- fd00:4701:0001:0002::/64
//! |   |    
//! |   |---- fd00:4701:0002::/48
//! |   |   |
//! |   |   |---- fd00:4702:0001:0001::/64
//! |   |   |---- fd00:4702:0001:0002::/64
//! |   |    
//! |   - fd00:4702::/32
//! |   |
//! |   |---- fd00:4702:0001::/48
//! |   |   |
//! |   |   |---- fd00:4701:0002:0001::/64
//! |   |   |---- fd00:4701:0002:0002::/64
//! |   |    
//! |   |---- fd00:4702:0002::/48
//! |   |   |
//! |   |   |---- fd00:4702:0002:0001::/64
//! |   |   |---- fd00:4702:0002:0002::/64
//! ```
//!
//! Finding the longest prefix match within this tree ammounts to depth-first
//! search.
//!
//! ### Ternary Matching
//!
//! Consider a table with the following ternary values.
//!
//! +--------+----------------+--------------+---------+
//! | Action | switch address | ingress port | is icmp |
//! +--------+----------------+--------------+---------+
//! | a0     | true           | _            | true    |
//! | a1     | true           | _            | false   |
//! | a2     | _              | 2            | _       |
//! +--------+----------------+--------------+---------+
//!
//! The structure of the key in this table is (bit<1>, bit<16>, bit<1>). Where
//!
//! - True -> 1
//! - False -> 0
//! - _ -> "Don't Care"
//!
//! The don't care value is what makes these entries ternary matches instead of
//! exact matches. The don't care values are wildcards and match anything.
//!
//! - ([true, true], [0, 0xffff], [false, true]) @(d0)
//! |
//! | - ([true, tue], [0, 2], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [0, 2], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a2
//! | |  
//! | | - ([true, true], [0, 2], [true,true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a2
//! | |
//! | - ([true, true], [3, 0xffff], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [3, 0xffff], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | |  
//! | | - ([true, true], [3, 0xffff], [true, true]) @(d0, d1, d3)
//! | | |
//! | | | - a2
//! | |  
//! @ @ @
//! 0 1 2
//!
//! In general 1 decision is needed for each field, in the above case we only
//! have one bifurcation point for the ingress port because there is one entry.
//! However, the space for a 16-bit value is 65536 possible values so for more
//! complex/realistic tables more splitting will be required, consider the
//! following example that adds more match keys to this space.
//!
//! +--------+----------------+--------------+---------+
//! | Action | switch address | ingress port | is icmp |
//! +--------+----------------+--------------+---------+
//! | a0     | true           | _            | true    |
//! | a1     | true           | _            | false   |
//! | a2     | _              | 2            | _       |
//! | a3     | _              | 4            | _       |
//! | a4     | _              | 7            | _       |
//! | a5     | _              | 19           | _       |
//! | a6     | _              | 33           | _       |
//! | a7     | _              | 47           | _       |
//! +--------+----------------+--------------+---------+
//!
//! Without any additional splitting we have
//!
//! - ([true, true], [0, 0xffff], [false, true]) @(d0)
//! |
//! | - ([true, tue], [0, 2], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [0, 2], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a2
//! | |  
//! | | - ([true, true], [0, 2], [true,true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a2
//! | |
//! | - ([true, true], [3, 0xffff], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [3, 0xffff], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a2
//! | | | - a3
//! | | | - a4
//! | | | - a5
//! | | | - a6
//! | | | - a7
//! | |  
//! | | - ([true, true], [3, 0xffff], [true, true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a2
//! | | | - a3
//! | | | - a4
//! | | | - a5
//! | | | - a6
//! | | | - a7
//! | |  
//! @ @ @
//! 0 1 2
//!
//! This requires linear iteration of up to 7 entries. In reality this small
//! number is fine. But let's say our table size jumps up 2 orders of magnitued,
//! now we have linear iteration of 700 etries, which is not a good place to be
//! in the data path.
//!
//! Continuing with this small example, let's say that we want to limit our
//! linear iteration to 3 entries due to the ingress port constraint.
//! Concentrating just for the moment on the ingress port values 2, 4, 7, 19,
//! 33, and 47. We have 6 entries so the logical place to split is at 7. So if
//! we split at 7 instead of 2, and we have a look at our tree we see
//!
//! - ([true, true], [0, 0xffff], [false, true]) @(d0)
//! |
//! | - ([true, tue], [0, 7], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [0, 7], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a2
//! | | | - a3
//! | | | - a4
//! | |  
//! | | - ([true, true], [0, 7], [true,true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a2
//! | | | - a3
//! | | | - a4
//! | |
//! | - ([true, true], [8, 0xffff], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [8, 0xffff], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a5
//! | | | - a6
//! | | | - a7
//! | |  
//! | | - ([true, true], [8, 0xffff], [true, true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a5
//! | | | - a6
//! | | | - a7
//! | |  
//! @ @ @
//! 0 1 2
//!
//! If we want to have a smaller linear search, we need to partition the space
//! more than once. It's a trivial difference here, but for larger tables can
//! become critical. Consider splitting the space evenly twice, once at 5 and
//! once at 20 which gives us 3 even partitions of [2,4], [7, 19] and [33,47].
//!
//! - ([true, true], [0, 0xffff], [false, true]) @(d0)
//! |
//! | - ([true, tue], [0, 5], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [0, 5], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a2
//! | | | - a3
//! | |  
//! | | - ([true, true], [0, 5], [true,true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a2
//! | | | - a3
//! | |  
//! | - ([true, tue], [6, 20], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [6, 20], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a4
//! | | | - a5
//! | |  
//! | | - ([true, true], [6, 20], [true, true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a4
//! | | | - a5
//! |
//! | - ([true, true], [21, 0xffff], [false, true]) @(d0, d1)
//! | |
//! | | - ([true, true], [21, 0xffff], [false, false]) @(d0, d1, d3)
//! | | |
//! | | | - a1
//! | | | - a6
//! | | | - a7
//! | |  
//! | | - ([true, true], [21, 0xffff], [true, true]) @(d0, d1, d3)
//! | | |
//! | | | - a0
//! | | | - a6
//! | | | - a7
//! | |  
//! @ @ @
//! 0 1 2
//!
//! In the description above, we repartitioned a particular dimension of the
//! tree and reconstructed the whole thing for presentation purposes. Rice
//! however, is incremental in nature, it's in the name. The way we handle this
//! as rules are inserted by a tuning parameter mtl (max ternary leaf). This has a
//! similar semantic to the binth tuning parameter from hicuts, but here we use
//! it incrementally instead of by construction. When an action is inserted into
//! a ternary decision tree, if it would result in the size of the leaf going
//! beyond the mtl, then each ternary dimension is examined to see if it can be
//! further split. A test is performed on each splittable dimension to find the
//! most even partioning and the leaf node is thus partitioned into an internal
//! node with 2 new leaves.
//!
//! ### Exact Matching
//!
//! Exact matching is a special case of ternary matching, where wildcards are
//! not allowed.
//!
//! ### Range Matching
//!
//! TODO: We don't actually need range matching right now, so shelving this for
//! later.
//!
//! ### Combined Matching
//!
//! Now consider the combined case where we have both prefixes
//!
//! XXX This table is not possible - the prefix key cannot have don't care items
//! as it is not ternary
//!
//! +--------+--------------------------+-------------+--------------+---------+
//! | Action | Prefix                   | switch addr | ingress port | is icmp |
//! +--------+--------------------------+-------------+--------------+---------+
//! | a0     | _                        | true        | _            | true    |
//! | a1     | _                        | true        | _            | false   |
//! | a2     | _                        | _           | 2            | _       |
//! | a3     | _                        | _           | 4            | _       |
//! | a4     | _                        | _           | 7            | _       |
//! | a5     | _                        | _           | 19           | _       |
//! | a6     | _                        | _           | 33           | _       |
//! | a7     | _                        | _           | 47           | _       |
//! | a8     | fd00:4700::/24           | _           | _            | _       |
//! | a9     | fd00:4701::/32           | _           | _            | _       |
//! | a10    | fd00:4702::/32           | _           | _            | _       |
//! | a11    | fd00:4701:0001::/48      | _           | _            | _       |
//! | a12    | fd00:4701:0002::/48      | _           | _            | _       |
//! | a13    | fd00:4702:0001::/48      | _           | _            | _       |
//! | a14    | fd00:4702:0002::/48      | _           | _            | _       |
//! | a15    | fd00:4701:0001:0001::/64 | _           | _            | _       |
//! | a16    | fd00:4701:0001:0002::/64 | _           | _            | _       |
//! | a17    | fd00:4702:0001:0001::/64 | _           | _            | _       |
//! | a18    | fd00:4702:0001:0002::/64 | _           | _            | _       |
//! | a19    | fd00:4701:0002:0001::/64 | _           | _            | _       |
//! | a20    | fd00:4701:0002:0002::/64 | _           | _            | _       |
//! | a21    | fd00:4702:0002:0001::/64 | _           | _            | _       |
//! | a22    | fd00:4702:0002:0002::/64 | _           | _            | _       |
//! | a23    | fd00:1701::/32           | _           | _            | _       |
//! +--------+----------------+---------+-------------+--------------+---------+
//!
//! Let's build up this tree starting from the first entry, using the same
//! notation as the trees depicted above. We'll use the threshold of 3 to start
//! looking for splittings.
//!
//! A good question when looking at this table is: what happens when multiple
//! actions could match? For example a packet destined to fd00:4702:0002:0002::1
//! from ingress port 47. That would match actions a7 and a22. According to the
//! P4 Runtime Specification v 1.3.0 section 9.1 there is an implicit 32-bit
//! priority associated with each table entry that can break ties.
//!
//! #### Initial empty state
//!
//! - (_, _, _, _)
//!
//! #### Insert a0
//!
//! - (_, true, _, true)
//! |
//! | - a0
//!
//! #### Insert a1
//!
//! - (_, true, _, true)
//! |
//! | - a0
//! |
//! - (_, true, _, false)
//! |
//! | - a1
//!
//! #### Insert a2
//!
//! - (_, true, _, true)
//! |
//! | - a0
//! | - a2
//! |
//! - (_, true, _, false)
//! |
//! | - a1
//! | - a2
//!
//! #### Insert a3
//!
//! - (_, true, _, true)
//! |
//! | - a0
//! | - a2
//! | - a3
//! |
//! - (_, true, _, false)
//! |
//! | - a1
//! | - a2
//! | - a3
//!
//! #### Insert a4
//!
//! Here we start trying to break up the existing rules as both have 3 rules
//! that require linear search. All the prefix values are _ up to this point, so
//! cutting in the prefix dimension does nothing. The switch addr and icmp
//! dimensions are binary, so no cutting to be had there. We're left with
//! ingress port.
//!
//! The list of values we have so far are [2, 4], so let's split at 3.
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - a0
//! | | - a3
//! | | - a4
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - a1
//! | | - a3
//! | | - a4
//!
//! #### Insert a5
//!
//! Insertion lands in leaf nodes that both have 3 nodes, more splitting. Each
//! leaf node to split currently spans [4, 0xffff] and contains the values 4 and
//! 7 in the ingress port dimension. Let's split at 8
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - a0
//! |   | - a5
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - a1
//! |   | - a5
//!
//! #### Insert a6
//!
//! No splits needed, huzza!
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - a0
//! |   | - a5
//! |   | - a6
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - a1
//! |   | - a5
//! |   | - a6
//!
//! #### Insert a7
//! 
//! Need to split the range [9, 0xffff], we hve 19 and 33 as resident values,
//! let's split at 34.
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a0
//! |   | | - a5
//! |   | | - a6
//! |   |  
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a0
//! |     | - a7
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a1
//! |   | | - a5
//! |   | | - a6
//! |   |
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a1
//! |     | - a7
//!
//! Alrighty now it's time to insert some prefixes.
//!
//! #### Insert a22
//!
//! There are no ternary constraints on the prefixes so they literally replicate
//! everywhere into the tree and start their own subtrees. We start by inserting
//! the last prefix first to demonstrate what happens when a containing prefix
//! is inserted (in the next step). The a22 action is wrapped in angle brackets
//! below to indicate this is a lpm match kind.
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! | | - <a22>
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | | | - a2
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a0
//! |   | | - a5
//! |   | | - a6
//! |   | | - <a22>
//! |   |  
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a0
//! |     | - a7
//! |     | - <a22>
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! | | - <a22>
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | | | - <a22>
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a1
//! |   | | - a5
//! |   | | - a6
//! |   | | - <a22>
//! |   |
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a1
//! |     | - a7
//! |     | - <a22>
//!
//! #### Insert a8
//!
//! When we insert a8, the process is similar to a22. However, when we get to
//! the leaf node, we see that 1) there is already a prefix there and 2) that
//! the prefix we are inserting contains that prefix. So we add a22 as a child
//! to a8 and replace a22 in the leaf node with z8
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! | | - <a8>
//! |   | - <a22>
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | | | - a2
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a0
//! |   | | - a5
//! |   | | - a6
//! |   | | - <a8>
//! |   |   | - <a22>
//! |   |  
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a0
//! |     | - a7
//! |     | - <a8>
//! |       | - <a22>
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! | | - <a8>
//! |   | - <a22>
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | | | - <a8>
//! | |   | - <a22>
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a1
//! |   | | - a5
//! |   | | - a6
//! |   | | - <a8>
//! |   |   | - <a22>
//! |   |
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a1
//! |     | - a7            H1
//! |     | - <a8>
//! |       | - <a22>       H2
//!
//! Let's take a moment to evalutate the problematic packet that matches two
//! actions described above. This packet has a destination address of
//! fd00:4702:0002:0002::1 and an ingress port of 47, matching the rules a7 and
//! a22. The hits in the tree traversal are marked H1, H2. Note that there are
//! other occurences of a7 and a22 in the tree. However let's assume this is not
//! an icmp packet so the whole first half of the tree is out of play. In the
//! second half of the tree ingress port 47 takes us to the latter option of the
//! second choice, and finally port 47 also takes us to the latter option of the
//! third choice. We are now at a leaf node where both a7 and a22 are viable
//! options. If we take the additional step to sort actions in leaf nodes
//! according to priority, then we can just take the first match.
//!
//! This leads to another question. Our seach space benefited greatly from the
//! icmp constraint. But what if a0 had a _ for the icmp field? Is this trending
//! toward bad on the programmer for writing ambiguous rule sets, or at the very
//! least ones that lean really heavily on priorities to choose a winner (while
//! at the same time making our decision tree more like a decision list
//! pretending to be a tree)? I'm not going to explore this in this comment,
//! more explorations to come in code.
//!
//! At this point I'm going to stop typing out decision trees and start
//! computing them. We get the general idea at this point of how these things
//! are constructed. So let's start constructing them, writing good printers to
//! asses as we go which I'll reference later to explore this priority
//! arbitrated ambiguous rule thing and of course have a small mountain of tests
//! to catch bad behavior.
//!
//! Just kidding, one more case to write out before we continue. Let's add a23
//! which is disjoint from any of the existing prefixes.
//!
//! #### Insert a23
//!
//! Our first stop for inserting a32 is the first leaf. Here we have 3 rules and
//! a23 is disjoint from a8, so we ned to make a split. We could split further
//! on the ingress port. But let's go with the idea of only splitting on
//! dimensions for which the value being inserted does not have a don't care
//! value. This leaves us with splitting based on the prefix. Time to replace a8
//! with a pair of internal nodes.
//!
//! TODO this table is incomplete and wrong, generate it and then paste the
//! result here....
//!
//! - (_, true, _, true)
//! |
//! | - (_, [0, 3], _, true)
//! | | 
//! | | - a0
//! | | - a2
//! | | - (fd00:4700::/24, [0, 3], _, true)
//! | | |
//! | | | - <a8>
//! | |   | - <a22>
//! | |
//! | | - (fd00:1701::/32, [0, 3], _, true)
//! |   |
//! |   | - <a23>
//! |
//! | - (_, [4, 0xffff], _, true)
//! | | 
//! | | - (_, [4, 8], _, true)
//! | | |
//! | | | - a0
//! | | | - a3
//! | | | - a4
//! | | | - a2
//! | |   
//! | | - (_, [9, 0xffff], _, true)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a0
//! |   | | - a5
//! |   | | - a6
//! |   |
//! |   | - (fd00:4700::/24, [0, 0xffff], _, true)
//! |   | |
//! |   | | - <a8>
//! |   | | | - <a22>
//! |   |  
//! |   | - (fd00:1701::/32, [0, 0xffff], _, true)
//! |   | |
//! |   | | - <23>
//! |   |
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a0
//! |     | - a7
//! |     | - (fd00:4700::/24, [35, 0xffff], _ true)
//! |     | |
//! |     | | - <a8>
//! |     | | - <a22>
//! |     |
//! |     | - (fd00:4700::/24, [35, 0xffff], _ true)
//! |
//! - (_, true, _, false)
//! |
//! | - (_, [0, 3], _, false)
//! | |
//! | | - a1
//! | | - a2
//! | | - (fd00:4700::/24, [0, 3], _, false)
//! | | |
//! | | | - <a8>
//! | |   | - <a22>
//! | |
//! | | - (fd00:1701::/32, [0, 3], _, false)
//! | | |
//! | | | - <23>
//! |
//! | - (_, [4, 0xffff], _, false)
//! | |
//! | | - (_, [4, 8], _, false)
//! | | |
//! | | | - a1
//! | | | - a3
//! | | | - a4
//! | | | - <a8>
//! | |   | - <a22>
//! | |   
//! | | - (_, [9, 0xffff], _, false)
//! |   |
//! |   | - (_, [9, 34], _, true)
//! |   | |
//! |   | | - a1
//! |   | | - a5
//! |   | | - a6
//! |   | | - <a8>
//! |   |   | - <a22>
//! |   |
//! |   | - (_, [35, 0xffff], _, true)
//! |     |
//! |     | - a1
//! |     | - a7            H1
//! |     | - <a8>
//! |       | - <a22>       H2

use num::bigint::BigUint;

#[derive(Debug, Clone)]
pub struct Node<const D: usize> {
    pub keys: [Key; D],
    pub actions: Vec<Action<D>>,
    pub nodes: Vec<Box<Node<D>>>,
}

impl<const D: usize> Node<D> {
    pub fn insert(&mut self, action: Action<D>) {

        for (i, action_key) in action.keys.iter().enumerate() {

            if action_key == &Key::Ternary(Ternary::DontCare) {
                continue;
            }

            

        }
    }
}

impl<const D: usize> Default for Node<D> {
    fn default() -> Self {
        Self{
            keys: [(); D].map(|_| Key::default()),
            actions: Vec::new(),
            nodes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Action<const D: usize> {
    pub name: String,
    pub keys: [Key; D]
}

#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    Exact(BigUint),
    Range(BigUint, BigUint),
    Ternary(Ternary),
}

impl Default for Key {
    fn default() -> Self {
        Self::Ternary(Ternary::default())
    }
}

#[derive(Debug, Clone, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_comment_example() {

        let n = Node::<4>::default();
        println!("{:#?}", n);

    }

}
