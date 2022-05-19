//
// Interval
//

//TODO enforce that T is a tuple of objects conforming to a trait?
#[derive(Debug)]
pub struct Interval<T> {
    pub begin: T,
    pub end: T,
}

impl<T> Interval<T> {
    pub fn new(begin: T, end: T) -> Self {
        Self { begin, end }
    }
}

//
// Rule
//

#[derive(Debug)]
pub struct Rule<T> {
    pub name: String,
    pub interval: Interval<T>,
}

impl<T> Rule<T> {
    pub fn new(name: &str, interval: Interval<T>) -> Self {
        Self { name: name.into(), interval }
    }
}

//
// Partition
//

#[derive(Debug)]
pub struct Partition<T> {
    pub interval: Interval<T>,
    pub rules: Vec<Rule<T>>,
}

impl<T> Partition<T> {
    pub fn new(
        interval: Interval<T>,
        rules: Vec<Rule<T>>,
    ) -> Self {
        Self { interval, rules }
    }
}

//
// Node
//

#[derive(Debug)]
pub enum Node<T> {
    Internal(Internal<T>),
    Leaf(Vec<Rule<T>>),
}

//
// Internal
//

#[derive(Debug)]
pub struct Internal<T> {
    pub interval: Interval<T>,
    pub d: usize,
    pub children: Vec<Node<T>>,
}

impl<T> Internal<T> {
    pub fn new(
        d: usize,
        interval: Interval<T>,
    ) -> Self {
        Self {
            interval,
            d,
            children: Vec::new(),
        }
    }
}

//
// DecisionTree
//

#[derive(Debug)]
pub struct DecisionTree<T> {
    pub binth: usize,
    pub spfac: f32,
    pub root: Internal<T>,
}

impl<T> DecisionTree<T> 
    where T: Domain + std::fmt::Debug,
{

    pub fn new(
        binth: usize,
        spfac: f32,
        rules: Vec<Rule<T>>,
    ) -> Self {
        Self {
            binth,
            spfac,
            root: Self::cut(
                binth,
                spfac,
                Interval::new(T::min(), T::max()),
                rules,
            )
        }
    }

    pub fn cut(
        binth: usize,
        spfac: f32,
        domain: Interval::<T>,
        rules: Vec<Rule<T>>,
    ) -> Internal<T> {
        let (d, partitions) = Self::cut_dimension(&rules, spfac, &domain);

        println!("DOMAIN={}", d);
        println!("{:#?}", partitions);

        let mut node = Internal::new(d, domain);

        for p in partitions {
            if p.rules.len() <= binth {
                node.children.push(Node::<T>::Leaf(p.rules));
            } else {
                node.children.push(Node::<T>::Internal(Self::cut(
                    binth,
                    spfac,
                    p.interval,
                    p.rules,
                )));
            }
        }

        node
    }

    pub fn cut_dimension(
        _rules: &Vec<Rule<T>>,
        _spfac: f32,
        _domain: &Interval<T>,
    ) -> (usize, Vec::<Partition<T>>) {
        //let mut candidates = Vec::new();
        /*
        Self::partitions(
            rules,
            spfac,
            domain.begin.0,
            domain.end.0,
        );
        */

        todo!();
    }

    pub fn partitions<P>(
        _rules: &Vec<Rule<T>>,
        _spfac: f32,
        _min: P,
        _max: P,
    ) -> Vec<Partition<T>> {
        todo!();
    }

}


pub trait Domain {
    fn min() -> Self;
    fn max() -> Self;
}

trait Project<const K: usize> {
}
