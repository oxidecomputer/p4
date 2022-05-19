use crate::bits::{bit, bytes};

#[derive(Debug)]
pub struct Interval<const A: usize> 
where
    [u8; bytes!(A)]: Sized,
{
    pub begin: bit<A>,
    pub end: bit<A>,
}

#[derive(Debug)]
pub struct Rule<const A: usize, const B: usize>
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    pub name: String,
    pub intervals: (Interval<A>, Interval<B>),
}

#[derive(Debug)]
pub struct Partition<const A: usize, const B: usize>
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    pub intervals: (Interval<A>, Interval<B>),
    pub rules: Vec<Rule<A, B>>,
}

#[derive(Debug)]
pub enum Node<const A: usize, const B: usize>
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    Internal(Internal<A, B>),
    Leaf(Vec<Rule<A, B>>),
}

#[derive(Debug)]
pub struct Internal<const A: usize, const B: usize>
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    pub intervals: (Interval<A>, Interval<B>),
    pub d: usize,
    pub children: Vec<Node<A, B>>,
}

#[derive(Debug)]
pub struct DecisionTree<const A: usize, const B: usize>
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    pub binth: usize,
    pub spfac: f32,
    pub root: Internal<A, B>,
}

impl<const A: usize, const B: usize> DecisionTree<A, B> 
where
    [u8; bytes!(A)]: Sized,
    [u8; bytes!(B)]: Sized,
{
    pub fn new(
        binth: usize,
        spfac: f32,
        domain: (Interval<A>, Interval<B>),
        rules: Vec<Rule<A, B>>,
    ) -> Self {
        Self {
            binth,
            spfac,
            root: Self::cut(
                binth,
                spfac,
                domain,
                rules,
            ),
        }
    }

    pub fn cut(
        binth: usize,
        spfac: f32,
        domain: (Interval<A>, Interval<B>),
        rules: Vec<Rule<A, B>>,
    ) -> Internal<A, B> {

        let (d, partitions) = Self::cut_dimension(&rules, spfac, &domain);

        println!("DOMAIN={}", d);
        println!("{:#?}", partitions);

        let mut node = Internal{d, intervals: domain, children: Vec::new()};

        for p in partitions {
            if p.rules.len() <= binth {
                node.children.push(Node::<A,B>::Leaf(p.rules));
            } else {
                node.children.push(Node::<A,B>::Internal(Self::cut(
                    binth,
                    spfac,
                    p.intervals,
                    p.rules,
                )));
            }
        }

        node
    }

    pub fn cut_dimension(
        rules: &Vec<Rule<A, B>>,
        spfac: f32,
        domain: &(Interval<A>, Interval<B>),
    ) -> (usize, Vec<Partition<A, B>>) {

        let partitions_a = Self::partitions(
            0,
            spfac,
            rules,
            domain,
            domain.0.begin,
            domain.0.end,
        );

        let largest_a = 
            partitions_a.iter().map(|x| x.rules.len()).max().unwrap_or(0);

        let partitions_b = Self::partitions(
            1,
            spfac,
            rules,
            domain,
            domain.1.begin,
            domain.1.end,
        );

        let largest_b = 
            partitions_b.iter().map(|x| x.rules.len()).max().unwrap_or(0);

        if largest_a > largest_b {
            (0, partitions_a)
        } else {
            (1, partitions_b)
        }

    }

    pub fn partitions<const K: usize>(
        d: usize,
        spfac: f32,
        rules: &Vec<Rule<A, B>>,
        _interval: &(Interval<A>, Interval<B>),
        min: bit<K>,
        max: bit<K>,
    ) -> Vec<Partition<A, B>> 
        where [u8; bytes!(K)]: Sized,
    {
        let lower = min;
        let upper = max;
        let mut x = upper >> 1;
        let mut bound = x >> 1;
        let goal = (spfac * rules.len() as f32) as usize;
        let mut rule_count = 0;
        let mut partitions = Vec::new();

        loop {
            if bound == bit::<K>::ZERO {
                break;
            }

            partitions = Self::partition(&rules, d, min, x, upper - lower);

            rule_count = partitions.iter().map(|x| x.rules.len()).sum();

            println!(
                "opc: check x={:?} bound={:?} goal={:?} rules={:?} parts={:?}",
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
                x = x - bound;
                bound = bound >> 1;
                continue;
            }
            if rule_count < goal {
                x = x + bound;
                bound = bound >> 1;
                continue;
            }

        }

        if rule_count > goal {
            x = x - 1;
            partitions = Self::partition(&rules, d, min, x, upper - lower);
        }

        partitions
    }

    fn partition<const K: usize>(
        _rules: &Vec<Rule<A, B>>,
        _d: usize,
        begin: bit<K>,
        count: bit<K>,
        over: bit<K>,
    ) -> Vec<Partition<A, B>> 
        where [u8; bytes!(K)]: Sized,
    {
        let result = Vec::new();

        if count == bit::<K>::ZERO {
            return result
        }

        let psize = over / count;

        let partition = bit::<K>::ZERO;
        loop {

            let p_begin = begin + psize * partition;
            let _p_end = p_begin + psize;

            /*
            let mut p = Partition::<A, B>{
                intervals: Interval{
                    begin: p_begin, 
                    end: p_end
                }, 
                rules: Vec::new(),
            };
            */
            //partition = partition + 1;
            break;
        }

        todo!();
    }

}
