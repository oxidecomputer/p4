#[derive(Debug)]
pub struct DecisionTree<const K: usize> {
    // Tuning parameters
    pub binth: usize,
    pub spfac: f32,

    pub domain: [usize; K],
    pub root: Internal<K>,
}

impl<const K: usize> DecisionTree<K> {
    /// Create a new decision tree that has at most `binth` rules in each leaf
    /// node. The overall memory requirement for the tree is proportional to the
    /// `spfac` parameter. The depth of the tree is inversely proportional to
    /// `binth` and `spfac`. General trends indicate the shorter the tree the
    /// better the query performance.
    ///
    /// The `domain` parameter indicates the size in bits of each domain. For
    /// example to classify IPv6/UDP packets over the destination address and
    /// destination port you would use [128, 16].
    pub fn new(
        binth: usize,
        spfac: f32,
        domain: [usize; K],
        rules: Vec<Rule<K>>,
    ) -> Self {
        Self {
            binth,
            spfac,
            domain,
            root: Self::cut(
                binth,
                spfac,
                domain.map(|x| Interval::new(0, 1 << x)),
                rules,
            ),
        }
    }

    fn cut(
        binth: usize,
        spfac: f32,
        domain: [Interval; K],
        rules: Vec<Rule<K>>,
    ) -> Internal<K> {
        let (d, partitions) = Self::cut_dimension(&rules, spfac, domain);

        println!("DOMAIN={}", d);
        println!("{:#?}", partitions);

        let mut node = Internal::new(d, domain);

        for p in partitions {
            if p.rules.len() <= binth {
                node.children.push(Node::<K>::Leaf(p.rules));
            } else {
                let mut child_domain = domain;
                child_domain[d] = p.interval;
                node.children.push(Node::<K>::Internal(Self::cut(
                    binth,
                    spfac,
                    child_domain,
                    p.rules,
                )));
            }
        }

        node
    }

    pub fn insert(&mut self, rule: Rule<K>) {
        self.root.insert(rule);
    }

    /// Create the partitions for a given cut. This function will optimize the
    /// number of partitions to be the maximum possible within the spfac
    /// heuristic.
    pub fn partitions(
        d: usize,
        spfac: f32,
        rules: &Vec<Rule<K>>,
        min: usize,
        max: usize,
    ) -> (usize, Vec<Partition<K>>) {
        // A space measure for a cut at node v (c[v]) is the sum of the number
        // of rules in each child that would be created by the cut plus the
        // number of times c partitions v. A cut always partitions v into equal
        // intervals.
        //
        //   sm(c[v]) = sum(num_rules(child[i]) + np(c[v])
        //
        // If a partition is created that divides a rule, that means the rule
        // will be present on both sides of the partition, thus replicating the
        // rule, making a bigger tree and consuming more memory.
        //
        // Chosing a cut that maximizes the inequality
        //
        //   sm(c[v]) <= spmf(num_rules(v))
        //
        // constrained by
        //
        //   1 <= sm(c[v]) <= len(v[d]) - 1
        //
        // where v[d] is the length of the interval for dimension d of node v.
        //
        // gives us the maximum number of cuts within the tuning parameter
        // spfac.  Which is defined as
        //
        //   spmf(N) = spfac * N

        //
        // run a binary search on values of np(c[v])
        //

        let lower = min;
        let upper = max;
        let mut x = upper / 2;
        let mut bound = x / 2;
        let goal = (spfac * rules.len() as f32) as usize;
        let mut rule_count = 0;
        let mut partitions = Vec::new();
        loop {
            if bound == 0 {
                break;
            }

            partitions = Self::partition(&rules, d, min, x, upper - lower);

            rule_count = partitions.iter().map(|x| x.rules.len()).sum();

            println!(
                "opc: check x={} bound={} goal={} rules={} parts={}",
                x,
                bound,
                goal,
                rule_count,
                partitions.len()
            );

            if rule_count == goal {
                break;
            }
            if rule_count > goal {
                x -= bound;
                bound /= 2;
                continue;
            }
            if rule_count < goal {
                x += bound;
                bound /= 2;
                continue;
            }
        }

        if rule_count > goal {
            x -= 1;
            partitions = Self::partition(&rules, d, min, x, upper - lower);
        }

        (x, partitions)
    }

    /// Given a set of `rules`, a desired partition `count`, and a space to
    /// partition `over`, partition the rules in dimension `d`.
    fn partition(
        rules: &Vec<Rule<K>>,
        d: usize,
        begin: usize,
        count: usize,
        over: usize,
    ) -> Vec<Partition<K>> {
        let mut result = Vec::new();

        if count == 0 {
            return result;
        }

        let psize = over / count;

        for partition in 0..count {
            //beginning and end of the partition
            let p_begin = begin + psize * partition;
            let p_end = p_begin + psize;

            let mut p =
                Partition::<K>::new(Interval::new(p_begin, p_end), Vec::new());

            for r in rules {
                // beginning and end of the rule in the given dimension
                let r_begin = r.intervals[d].begin;
                let r_end = r.intervals[d].end;

                // There are 3 overlap conditions to check
                //
                // 1. the beginning of this rule is within the partition
                // 2. the end of this rule is within the partition
                // 3. this rule contains the partition
                let begin = r_begin >= p_begin && r_begin < p_end;
                let end = r_end >= p_begin && r_end < p_end;
                let contain = r_begin <= p_begin && r_end >= p_end;

                // add the rule to the partition if there is any overlap
                if begin | end | contain {
                    p.rules.push(r.clone());
                }
            }

            result.push(p);
        }

        result
    }

    /// Decide the dimension to cut along. The heuristic employed here is to
    /// select the dimension whose largest child is the smallest e.g. min-max on
    /// child cardinality.
    pub fn cut_dimension(
        rules: &Vec<Rule<K>>,
        spfac: f32,
        domain: [Interval; K],
    ) -> (usize, Vec<Partition<K>>) {
        let mut candidates = Vec::new();

        for d in 0..K {
            let (_, partitions) = Self::partitions(
                d,
                spfac,
                rules,
                domain[d].begin,
                domain[d].end,
            );

            let largest_child =
                partitions.iter().map(|x| x.rules.len()).max().unwrap_or(0);

            println!("d={} lc={} {:#?}", d, largest_child, partitions);
            candidates.push((largest_child, partitions));
        }

        let index = candidates
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.0.cmp(&b.0))
            .map(|(i, _)| i)
            .unwrap_or(0);

        (index, candidates[index].1.clone())
    }

    /// Maximize resuse of child nodes.
    pub fn max_reuse() {
        todo!();
    }

    /// Eliminate redundancy
    pub fn eliminate_redundancy() {
        todo!();
    }
}

#[derive(Debug)]
pub enum Node<const K: usize> {
    Internal(Internal<K>),
    Leaf(Vec<Rule<K>>),
}

#[derive(Debug)]
pub struct Internal<const K: usize> {
    /// The intervals this internal node spans.
    pub intervals: [Interval; K],

    /// Cut dimension index
    pub d: usize,

    /// Child nodes of this node.
    pub children: Vec<Node<K>>,
}

impl<const K: usize> Internal<K> {
    pub fn new(d: usize, intervals: [Interval; K]) -> Self {
        Self {
            intervals,
            d,
            children: Vec::new(),
        }
    }

    pub fn insert(&mut self, _rule: Rule<K>) {
        todo!();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub begin: usize,
    pub end: usize,
}

impl Interval {
    pub fn new(begin: usize, end: usize) -> Self {
        Self { begin, end }
    }
}

#[derive(Clone, Debug)]
pub struct Partition<const K: usize> {
    pub interval: Interval,
    pub rules: Vec<Rule<K>>,
}

impl<const K: usize> Partition<K> {
    pub fn new(interval: Interval, rules: Vec<Rule<K>>) -> Self {
        Self { interval, rules }
    }
}

impl Default for Interval {
    fn default() -> Self {
        Self { begin: 0, end: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct Rule<const K: usize> {
    pub name: String,
    pub intervals: [Interval; K],
}

impl<const K: usize> Rule<K> {
    pub fn new(name: &str, intervals: [Interval; K]) -> Self {
        Self {
            name: name.into(),
            intervals,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_from_paper() -> Vec<Rule<2>> {
        vec![
            Rule::new("r1", [Interval::new(0, 31), Interval::new(0, 255)]),
            Rule::new("r2", [Interval::new(0, 255), Interval::new(128, 131)]),
            Rule::new("r3", [Interval::new(64, 71), Interval::new(128, 255)]),
            Rule::new("r4", [Interval::new(67, 67), Interval::new(0, 127)]),
            Rule::new("r5", [Interval::new(64, 71), Interval::new(0, 15)]),
            Rule::new("r6", [Interval::new(128, 191), Interval::new(4, 131)]),
            Rule::new("r7", [Interval::new(192, 192), Interval::new(0, 255)]),
        ]
    }

    #[test]
    fn example_from_paper() {
        let rules = rules_from_paper();

        let d = DecisionTree::<2>::new(2, 1.5, [8, 8], rules);
        println!("{:#?}", d);

        /*
        for r in rules_from_paper() {
            d.insert(r);
        }
        */
    }

    #[test]
    fn partition_rule_count() {
        let rules = rules_from_paper();

        let partitions = DecisionTree::<2>::partition(&rules, 0, 0, 4, 256);

        println!("{:#?}", partitions);

        let partitioned_rule_count: usize =
            partitions.iter().map(|x| x.rules.len()).sum();

        assert_eq!(partitioned_rule_count, 10);
    }

    #[test]
    fn optimize_partitions() {
        let rules = rules_from_paper();

        let (optimal_partitions, partitions) =
            DecisionTree::<2>::partitions(0, 1.5, &rules, 0, 256);

        println!("{:#?}", partitions);

        assert_eq!(optimal_partitions, 4);
    }

    #[test]
    fn cut_dimension() {
        let rules = rules_from_paper();

        let (d, partitions) = DecisionTree::<2>::cut_dimension(
            &rules,
            1.5,
            [Interval::new(0, 256); 2],
        );

        println!("{:#?}", partitions);

        assert_eq!(d, 0);
    }
}
