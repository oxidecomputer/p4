use std::collections::BTreeMap;

#[derive(Debug)]
pub struct AST {
    pub constants: Vec<Constant>,
    pub headers: Vec<Header>,
    pub typedefs: Vec<Typedef>,
    pub controls: Vec<Control>,
}

impl Default for AST {
    fn default() -> Self {
        Self{
            constants: Vec::new(),
            headers: Vec::new(),
            typedefs: Vec::new(),
            controls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    Bool,
    Error,
    Bit(usize),
    Int(usize),
    String,
    UserDefined(String),
}

#[derive(Debug, Clone)]
pub struct Typedef {
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub ty: Type,
    pub name: String,
    //TODO initializer: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    IntegerLit(i128),
    BitLit(i128),
}

#[derive(Debug, Clone)]
pub struct Header {
    pub name: String,
    pub members: Vec::<HeaderMember>,
}

impl Header {
    pub fn new(name: String) -> Self {
        Header{name,  members: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct HeaderMember {
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Control {
    pub name: String,
    pub parameters: Vec::<ControlParameter>,
    pub actions: Vec::<Action>,
    pub tables: Vec::<Table>,
}

impl Control {
    pub fn new(name: String) -> Self {
        Self{
            name,
            parameters: Vec::new(),
            actions: Vec::new(),
            tables: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ControlParameter {
    pub direction: Direction,
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub enum Direction {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub parameters: Vec::<ActionParameter>,
}

impl Action {
    pub fn new(name: String) -> Self {
        Self{
            name,
            parameters: Vec::new(),
        }
    }
}


#[derive(Debug, Clone)]
pub struct ActionParameter {
    pub ty: Type,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub actions: Vec::<String>,
    pub default_action: String,
    pub key: BTreeMap<String, MatchKind>,
}

impl Table {
    pub fn new(name: String) -> Self {
        Self{
            name,
            actions: Vec::new(),
            default_action: String::new(),
            key: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum MatchKind {
    Exact,
    Ternary,
    LongestPrefixMatch,
}
