/// A keyset is a sequence of fields
#[derive(Debug, Clone)]
pub struct Keyset<const K: usize>([u8; K]);

impl<const K: usize> Keyset<K> {
    fn set<const D: usize>(
        &mut self,
        d: usize,
        layout: &[usize; D],
        f: &Field
    ) {

        let mut offset = 0;
        for width in &layout[..d] {
            offset += width;
        }
        let end = offset + layout[d];
        self.0[offset..end].copy_from_slice(&f.0.as_slice()[..layout[d]]);
    }
}

impl<const K: usize> Keyset<K> {
    fn dump(&self) -> String {
        format!("{:?}", self.0)
    }
}

impl<const K: usize> Keyset<K> {
    pub const MIN: Self = Self([u8::MIN; K]);
    pub const MAX: Self = Self([u8::MAX; K]);
}

#[derive(Debug, Clone)]
pub struct Rule<const K: usize> {
    pub name: String,
    pub range: KeysetRange<K>
}

impl<const K: usize> Rule<K> {
    fn dump(&self) -> String {
        format!("{}: {}", self.name, self.range.dump())
    }
}

#[derive(Debug, Clone)]
pub struct KeysetRange<const K: usize> {
    pub begin: Keyset<K>,
    pub end: Keyset<K>,
}

impl<const K: usize> KeysetRange<K> {
    fn dump(&self) -> String {
        format!("begin={} end={}", self.begin.dump(), self.end.dump())
    }
}

#[derive(Debug, Clone)]
pub struct Partition<const K: usize> {
    pub range: KeysetRange<K>,
    pub rules: Vec<Rule<K>>,
}

#[derive(Debug)]
pub enum Node<const K: usize> {
    Internal(Internal<K>),
    Leaf(Leaf<K>),
}

impl<const K: usize> Node<K> {
    fn dump(&self, level: usize) -> String {
        let indent = "  ".repeat(level);
        match self {
            Self::Internal(i) => {
                format!("{}",i.dump(level))
            }
            Self::Leaf(l)=> {
                format!("{}",l.dump(level))
                /*
                let mut s = format!("{}Leaf\n", indent);
                for r in rules {
                    s += &format!("{}{}{}\n", indent, indent, r.dump());
                }
                s
                */
            }
        }
    }
}

#[derive(Debug)]
pub struct Internal<const K: usize> {
    pub range: KeysetRange<K>,
    pub d: usize,
    pub children: Vec<Node<K>>,
}

impl<const K: usize> Internal<K> {
    fn dump(&self, level: usize) -> String {
        let indent = "  ".repeat(level);
        let mut s =
            format!("{}Internal(d={} range=({}))\n",
                indent, self.d, self.range.dump());

        if !self.children.is_empty() {
            for c in &self.children {
                s += &format!("{}{}", indent, c.dump(level+1));
            }
        }

        s
    }
}

#[derive(Debug)]
pub struct Leaf<const K: usize> {
    pub range: KeysetRange<K>,
    pub rules: Vec<Rule<K>>,
}

impl<const K: usize> Leaf<K> {
    fn dump(&self, level: usize) -> String {
        let indent = "  ".repeat(level);
        let mut s = format!("{}Leaf(range={})\n", indent, self.range.dump());
        for r in &self.rules {
            s += &format!("{}{}{}\n", indent, indent, r.dump());
        }
        s
    }
}

#[derive(Debug)]
pub struct DecisionTree<const K: usize, const D: usize> {
    pub binth: usize,
    pub spfac: f32,
    pub layout: [usize; D],
    pub root: Internal<K>,
}

impl<const K: usize, const D: usize> DecisionTree<K, D> {

    pub fn new(
        binth: usize,
        spfac: f32,
        layout: [usize; D],
        rules: Vec<Rule<K>>,
    ) -> Self {
        Self {
            binth,
            spfac,
            layout,
            root: Self::cut(
                binth,
                spfac,
                KeysetRange::<K>{
                    begin: Keyset::<K>::MIN,
                    end:   Keyset::<K>::MAX,
                },
                &layout,
                rules,
            )
        }
    }

    pub fn cut(
        binth: usize,
        spfac: f32,
        range: KeysetRange<K>,
        layout: &[usize; D],
        rules: Vec<Rule<K>>,
    ) -> Internal<K> {

        let (d, partitions) = Self::cut_dimension(&rules, spfac, &range, layout);

        println!("DOMAIN={}", d);
        println!("{:#?}", partitions);

        let mut node = Internal::<K>{
            range,
            d,
            children: Vec::new(),
        };

        for p in partitions {
            if p.rules.len() <= binth {
                node.children.push(Node::<K>::Leaf(Leaf::<K>{
                    range: p.range,
                    rules: p.rules,
                }));
            } else {
                node.children.push(Node::<K>::Internal(Self::cut(
                    binth,
                    spfac,
                    p.range,
                    layout,
                    p.rules,
                )));
            }
        }

        node

    }

    pub fn cut_dimension(
        rules: &Vec<Rule<K>>,
        spfac: f32,
        range: &KeysetRange<K>,
        layout: &[usize; D],
    ) -> (usize, Vec<Partition<K>>) {

        let mut candidates = Vec::new();

        for d in 0..K {

            let partitions = Self::partitions(
                d,
                spfac,
                rules,
                range,
                layout,
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

    pub fn partitions(
        d: usize,
        spfac: f32,
        rules: &Vec<Rule<K>>,
        range: &KeysetRange<K>,
        layout: &[usize; D],
    ) -> Vec<Partition<K>> {

        let lower = Self::extract_field(d, layout, &range.begin);
        let upper = Self::extract_field(d, layout, &range.end);
        let mut x = (&upper / 2) + 1;
        let mut bound = &x / 2;
        let goal = (spfac * rules.len() as f32) as usize;
        let mut rule_count = 0;
        let mut partitions = Vec::new();
        let over = (&upper - &lower) + 1;

        loop {

            println!("======================================================");
            println!("======================================================");
            println!("");
            println!("");
            println!("                      x={:?}", x);
            println!("");
            println!("");
            println!("======================================================");
            println!("======================================================");

            if bound.is_zero() {
                break;
            }

            partitions = Self::partition(
                &rules,
                d,
                lower.clone(),
                x.clone(),
                over.clone(),
                range,
                layout,
            );

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
                x = &x - &bound;
                bound = &bound / 2;
                continue;
            }
            if rule_count < goal {
                x = &x + &bound;
                bound = &bound / 2;
                continue;
            }
        }

        if rule_count > goal {
            x = &x - 1;
            partitions = Self::partition(
                &rules,
                d,
                lower.clone(),
                x.clone(),
                over.clone(),
                range,
                layout,
            );
        }

        partitions

    }

    pub fn partition(
        rules: &Vec<Rule<K>>,
        d: usize,
        begin: Field,
        count: Field,
        over: Field,
        range: &KeysetRange<K>,
        layout: &[usize; D],
    ) -> Vec<Partition<K>> {

        let mut result = Vec::new();

        if count.is_zero() {
            return result;
        }

        let psize = &over / &count;

        let mut partition = Field(vec![0]);
        loop {

            let p_begin = &begin + &psize * &partition;
            let p_end = &p_begin + &psize;

            println!("p_begin={:?}, p_end={:?}", p_begin, p_end);

            let mut p_range = range.clone();
            p_range.begin.set(d, layout, &p_begin);
            p_range.end.set(d, layout, &p_end);

            let mut p = Partition::<K>{
                range: p_range,
                rules: Vec::new(),
            };

            for r in rules {
                let r_begin = Self::extract_field(d, layout, &r.range.begin);
                let r_end = Self::extract_field(d, layout, &r.range.end);
                println!("  r_begin={:?}, r_end={:?}", r_begin, r_end);

                let begin = r_begin >= p_begin && r_begin < p_end;
                let end = r_end >= p_begin && r_end < p_end;
                let contain = r_begin <= p_begin && r_end >= p_end;

                if begin | end | contain {
                    println!("  -> {:?}", r);
                    p.rules.push(r.clone());
                }
            }

            partition = &partition + 1;
            result.push(p);

            if partition >= count {
                break;
            }

        }

        result
    }

    pub fn extract_field(
        d: usize,
        layout: &[usize; D],
        keyset: &Keyset<K>,
    ) -> Field {

        let mut offset = 0;
        for width in &layout[..d] {
            offset += width;
        }
        let end = offset + layout[d];
        Field(keyset.0[offset..end].to_owned())
    }

}

impl<const K: usize, const D: usize> DecisionTree<K, D> {
    fn dump(&self) -> String{
        let mut s = format!("DecisionTree(binth={}, spfac={} layout={:?})\n",
            self.binth, self.spfac, self.layout,
        );
        s += &format!("{}", self.root.dump(0));
        s
    }
}

#[derive(Clone, Debug)]
pub struct Field(Vec<u8>);

impl Field {
    pub fn is_zero(&self) -> bool {
        for x in &self.0 {
            if *x != 0u8 {
                return false;
            }
        }
        return true;
    }
}

impl std::ops::Div for &Field {
    type Output = Field;

    fn div(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a / b;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Mul for &Field {
    type Output = Field;

    fn mul(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a * b;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Div<usize> for &Field {
    type Output = Field;

    fn div(self, other: usize) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let c = a / other;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Sub<usize> for &Field {
    type Output = Field;

    fn sub(self, other: usize) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let c = a - other;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Sub for &Field {
    type Output = Field;

    fn sub(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a - b;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Add<usize> for &Field {
    type Output = Field;

    fn add(self, other: usize) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let c = a + other;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Add<usize> for Field {
    type Output = Field;

    fn add(self, other: usize) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let c = a + other;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Add for &Field {
    type Output = Field;

    fn add(self, other: Self) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a + b;
        Field(c.to_bytes_be())
    }
}

impl std::ops::Add<Field> for &Field {
    type Output = Field;

    fn add(self, other: Field) -> Self::Output {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        let c = a + b;
        Field(c.to_bytes_be())
    }
}

impl std::cmp::PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        a == b
    }
}

impl std::cmp::PartialOrd for Field {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let a = num::bigint::BigUint::from_bytes_be(&self.0);
        let b = num::bigint::BigUint::from_bytes_be(&other.0);
        Some(a.cmp(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_from_paper() -> Vec<Rule<2>> {
        vec![
            Rule::<2>{
                name: "r1".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([0, 0]),
                    end: Keyset::<2>([31, 255]),
                },
            },
            Rule::<2>{
                name: "r2".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([0, 128]),
                    end: Keyset::<2>([255, 131]),
                },
            },
            Rule::<2>{
                name: "r3".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([64, 128]),
                    end: Keyset::<2>([71, 255]),
                },
            },
            Rule::<2>{
                name: "r4".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([67, 0]),
                    end: Keyset::<2>([67, 127]),
                },
            },
            Rule::<2>{
                name: "r5".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([64, 0]),
                    end: Keyset::<2>([71, 15]),
                },
            },
            Rule::<2>{
                name: "r6".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([128, 4]),
                    end: Keyset::<2>([191, 131]),
                },
            },
            Rule::<2>{
                name: "r7".into(),
                range: KeysetRange::<2>{
                    begin: Keyset::<2>([192, 0]),
                    end: Keyset::<2>([192, 255]),
                },
            },
        ]
    }

    #[test]
    fn heap_example_from_paper() {
        let rules = rules_from_paper();

        //TODO layout in byes, should be in bits
        let d = DecisionTree::<2, 2>::new(2, 1.5, [1, 1], rules);
        println!("{:#?}", d);
        println!("{}", d.dump());
    }

}
