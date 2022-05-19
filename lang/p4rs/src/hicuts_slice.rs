use crate::bits::{bit, bytes, Field};

/// A range-based classifier for a K-bit key.
#[derive(Debug, Clone)]
pub struct Interval<const K: usize> 
    where [u8; bytes!(K)]: Sized,
{
    pub begin: bit<K>, // low values of the classifier
    pub end: bit<K>,   // high values of the classifier
}


#[derive(Debug, Clone)]
pub struct Rule<const K: usize> 
    where [u8; bytes!(K)]: Sized,
{
    /// Name of the rule.
    pub name: String,
    pub interval: Interval<K>
}

#[derive(Debug, Clone)]
pub struct Partition<const K: usize> 
    where [u8; bytes!(K)]: Sized,
{
    pub interval: Interval<K>,
    pub rules: Vec<Rule<K>>,
}

#[derive(Debug)]
pub enum Node<const K: usize> 
    where [u8; bytes!(K)]: Sized,
{
    Internal(Internal<K>),
    Leaf(Vec<Rule<K>>),
}

#[derive(Debug)]
pub struct Internal<const K: usize> 
    where [u8; bytes!(K)]: Sized,
{
    pub interval: Interval<K>,
    pub d: usize,
    pub children: Vec<Node<K>>,
}

#[derive(Debug)]
pub struct DecisionTree<const K: usize, const D: usize> 
    where [u8; bytes!(K)]: Sized,
{
    pub binth: usize,
    pub spfac: f32,
    pub root: Internal<K>,
    pub domain_layout: [usize; D]
}

impl<const K: usize, const D: usize> DecisionTree<K, D> 
    where [u8; bytes!(K)]: Sized,
{

    pub fn new(
        binth: usize,
        spfac: f32,
        rules: Vec<Rule<K>>,
        domain_layout: [usize; D],
    ) -> Self {
        Self {
            binth,
            spfac,
            domain_layout,
            root: Self::cut(
                binth,
                spfac,
                Interval::<K>{
                    begin: bit::<K>::min(), 
                    end: bit::<K>::max()
                },
                rules,
                &domain_layout,
            )
        }
    }

    pub fn cut(
        binth: usize,
        spfac: f32,
        domain: Interval::<K>,
        rules: Vec<Rule<K>>,
        domain_layout: &[usize; D],
    ) -> Internal<K> {
        let (d, partitions) = Self::cut_dimension(
            &rules,
            spfac,
            &domain,
            domain_layout,
        );

        println!("domain={}", d);
        println!("{:#?}", partitions);

        let mut node = Internal::<K>{
            interval: domain,
            d: d,
            children: Vec::new(),
        };

        for p in partitions {
            if p.rules.len() <= binth {
                node.children.push(Node::<K>::Leaf(p.rules));
            } else {
                node.children.push(Node::<K>::Internal(Self::cut(
                    binth,
                    spfac,
                    p.interval,
                    p.rules,
                    domain_layout,
                )));
            }
        }

        node
    }

    pub fn cut_dimension(
        rules: &Vec<Rule<K>>,
        spfac: f32,
        domain: &Interval<K>,
        domain_layout: &[usize; D],
    ) -> (usize, Vec::<Partition<K>>) {

        let mut candidates = Vec::new();

        let mut offset = 0;
        for (d, domain_size) in domain_layout.iter().enumerate() {
            let partitions = Self::partitions(
                d,
                offset,
                *domain_size,
                rules,
                spfac,
                domain,
            );
            offset += domain_size;

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

    pub fn partitions(
        d: usize,
        d_offset: usize,
        d_size: usize,
        rules: &Vec<Rule<K>>,
        spfac: f32,
        interval: &Interval<K>,
    ) -> Vec<Partition<K>> {

        // the maximum number of partitions is the number of points in the
        // interval along the specified dimension minus 1
        let _lower = interval.begin.field(d_offset, d_size);
        let _upper = interval.end.field(d_offset, d_size);

        let goal = (spfac * rules.len() as f32) as usize;

        /*
        let max_partitions = (1<<(d_size+1)) - 1;
        */
        // #well fuck
        //let max_partitions = (upper - lower) - 1;
        let max_partitions = 1;
        let min_partitions = 0;

        // starting point
        let mut x = (max_partitions - min_partitions)/2;

        // binary search delta
        let mut dx = x/2;

        let mut rule_count = 0;
        let mut partitions = Vec::new();

        loop {
            if dx == 0 {
                break;
            }

            partitions = Self::partition(
                rules,
                d,
                interval.begin.field(d_offset, d_size),
                x,
            );
            rule_count = partitions.iter().map(|x| x.rules.len()).sum();

            println!(
                "opc: check x={} dx={} goal={} rules={} parts={}",
                x,
                dx,
                goal,
                rule_count,
                partitions.len()
            );

            if rule_count == goal {
                break;
            }
            if rule_count > goal {
                x -= dx;
                dx /= 2;
                continue;
            }
            if rule_count < goal {
                x += dx;
                dx /= 2;
                continue;
            }
        }

        if rule_count > goal {
            x -= 1;
            partitions = Self::partition(
                rules,
                d,
                interval.begin.field(d_offset, d_size),
                x,
            );
        }

        partitions

    }

    fn partition(
        _rules: &Vec<Rule<K>>,
        _d: usize,
        _begin: Field, // field value to start partitioning at
        count: usize, // # of partitions
    ) -> Vec<Partition<K>> {

        let result = Vec::new();

        if count == 0 {
            return result;
        }

        result

    }

}
