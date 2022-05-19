//
// Interval
//

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

macro_rules! rule_type {
    ($name: ident, $($t: ident,)*) => {

        #[derive(Debug)]
        /// An n-dimensional rule.
        pub struct $name<$($t),*> {
            pub name: String,
            pub intervals: ($(Interval::<$t>,)*),
        }

        impl<$($t),*> $name<$($t),*> {
            pub fn new(name: &str, intervals: ($(Interval::<$t>,)*)) -> Self {
                Self {
                    name: name.into(),
                    intervals,
                }
            }
        }
    }
}

rule_type!(Rule1, A,);
rule_type!(Rule2, A, B,);

//
// Partition
//

macro_rules! partition_type {
    ($name: ident, $rt: ident, $($t: ident,)*) => {

        #[derive(Debug)]
        /// An n-dimensional partition.
        pub struct $name<$($t),*> {
            pub intervals: ($(Interval::<$t>,)*),
            pub rules: Vec::<$rt::<$($t,)*>>,
        }

        impl<$($t),*> $name<$($t),*> {
            pub fn new(
                intervals: ($(Interval::<$t>,)*),
                rules: Vec::<$rt::<$($t,)*>>,
            ) -> Self {
                Self { intervals, rules }
            }
        }
    }
}

partition_type!(Partition1, Rule1, A,);
partition_type!(Partition2, Rule2, A, B,);

//
// Node
//

macro_rules! node_type {
    ($name: ident, $rt: ident, $($t: ident,)*) => {
        pub enum $name<$($t),*> {
            Leaf(Vec<$rt::<$($t,)*>>),
        }
    }
}

node_type!(Node1, Rule1, A,);
node_type!(Node2, Rule2, A, B,);

//
// Internal
//

macro_rules! internal_type {
    ($name: ident, $nt: ident, $($t: ident,)*) => {
        pub struct $name<$($t),*> {
            pub intervals: ($(Interval::<$t>,)*),
            pub d: usize,
            pub children: Vec<$nt::<$($t,)*>>,
        }

        impl<$($t),*> $name<$($t),*> {
            pub fn new(
                d: usize,
                intervals: ($(Interval::<$t>,)*),
            ) -> Self {
                Self {
                    intervals,
                    d,
                    children: Vec::new(),
                }
            }
        }
    }
}

internal_type!(Internal1, Node1, A,);
internal_type!(Internal2, Node2, A, B,);

//
// DecisionTree
//

macro_rules! decision_tree {
    (
        $name: ident,
        $int: ident,
        $rt: ident,
        $pt: ident,
        $($t: ident,)*
    ) => {

        pub struct $name<$($t),*> {
            pub binth: usize,
            pub spfac: f32,

            pub root: $int<$($t),*>,
        }

        impl<$($t),*> $name<$($t),*> 
            where $($t: MinMax + std::fmt::Debug),*
        {

            pub fn new(
                binth: usize,
                spfac: f32,
                rules: Vec<$rt<$($t),*>>,
            ) -> Self {
                Self {
                    binth,
                    spfac,
                    root: Self::cut(
                        binth,
                        spfac,
                        ($(Interval::<$t>::new($t::min(), $t::max()),)*),
                        rules,
                    )
                }
            }

            pub fn cut(
                _binth: usize,
                spfac: f32,
                domain: ($(Interval::<$t>,)*),
                rules: Vec<$rt<$($t),*>>,
            ) -> $int<$($t),*> {

                let (d, partitions) = Self::cut_dimension(&rules, spfac, &domain);

                println!("DOMAIN={}", d);
                println!("{:#?}", partitions);

                let mut _node = $int::new(d, domain);

                todo!();

            }

            pub fn cut_dimension(
                _rules: &Vec<$rt<$($t),*>>,
                _spfac: f32,
                _domain: &($(Interval::<$t>,)*),
            ) -> (usize, Vec::<$pt<$($t),*>>) {
                todo!();
            }
        }
    }
}

decision_tree!(DecisionTree1, Internal1, Rule1, Partition1, A,);
decision_tree!(DecisionTree2, Internal2, Rule2, Partition2, A, B,);

pub trait MinMax {
    fn min() -> Self;
    fn max() -> Self;
}
