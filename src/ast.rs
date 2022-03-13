use std::collections::BTreeMap;

#[derive(Debug)]
pub struct AST {
    pub constants: Vec<Constant>,
    pub headers: Vec<Header>,
    pub structs: Vec<Struct>,
    pub typedefs: Vec<Typedef>,
    pub controls: Vec<Control>,
    pub parsers: Vec<Parser>,
}

impl Default for AST {
    fn default() -> Self {
        Self{
            constants: Vec::new(),
            headers: Vec::new(),
            structs: Vec::new(),
            typedefs: Vec::new(),
            controls: Vec::new(),
            parsers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Type {
    Bool,
    Error,
    Bit(usize),
    Varbit(usize),
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
pub struct Variable {
    pub ty: Type,
    pub name: String,
    //TODO initializer: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    IntegerLit(i128),
    BitLit(u16, u128),
    SignedLit(u16, i128),
    Lvalue(Lvalue),
    Binary(Box::<Expression>, Box::<Expression>),
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
pub struct Struct {
    pub name: String,
    pub members: Vec::<StructMember>,
}

impl Struct {
    pub fn new(name: String) -> Self {
        Struct{name,  members: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct StructMember {
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
pub struct Parser {
    pub name: String,
    pub parameters: Vec::<ControlParameter>,
    pub states: Vec::<State>,
}

impl Parser {
    pub fn new(name: String) -> Self {
        Self{
            name,
            parameters: Vec::new(),
            states: Vec::new(),
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
    Unspecified,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub parameters: Vec::<ActionParameter>,
    pub variables: Vec::<Variable>,
    pub constants: Vec::<Constant>,
    pub statements: Vec::<Statement>,
}

impl Action {
    pub fn new(name: String) -> Self {
        Self{
            name,
            parameters: Vec::new(),
            variables: Vec::new(),
            constants: Vec::new(),
            statements: Vec::new(),
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
    pub key: BTreeMap<Lvalue, MatchKind>,
    pub const_entries: Vec::<ConstTableEntry>,
}

impl Table {
    pub fn new(name: String) -> Self {
        Self{
            name,
            actions: Vec::new(),
            default_action: String::new(),
            key: BTreeMap::new(),
            const_entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstTableEntry {
    pub keyset: Vec::<KeySetElement>,
    pub action: ActionRef,
}

#[derive(Debug, Clone)]
pub enum KeySetElement {
    Expression(Expression),
    Default,
    DontCare,
    Masked(Expression, Expression),
    Ranged(Expression, Expression),
}


#[derive(Debug, Clone)]
pub struct ActionRef {
    pub name: String,
    pub parameters: Vec::<Expression>,
}

impl ActionRef {
    pub fn new(name: String) -> Self {
        Self { name, parameters: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub enum MatchKind {
    Exact,
    Ternary,
    LongestPrefixMatch,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Empty,
    Assignment(Lvalue, Expression),
    Call(Call),
    // TODO ...
}

/// A function or method call
#[derive(Debug, Clone)]
pub struct Call {
    pub lval: Lvalue,
    pub args: Vec::<Expression>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lvalue {
    pub name: String,
}


#[derive(Debug, Clone)]
pub struct State {
    pub name: String,
    pub variables: Vec::<Variable>,
    pub constants: Vec::<Constant>,
    pub statements: Vec::<Statement>,
    pub transition: Option<Transition>,
}

impl State {
    pub fn new(name: String) -> Self {
        Self{
            name,
            variables: Vec::new(),
            constants: Vec::new(),
            statements: Vec::new(),
            transition: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Transition {
    Reference(String),
    Select(Select),
}

#[derive(Debug, Clone, Default)]
pub struct Select {
    pub parameters: Vec::<Expression>,
    pub elements: Vec::<SelectElement>,
}

#[derive(Debug, Clone)]
pub struct SelectElement {
    pub keyset: Vec::<KeySetElement>,
    pub name: String,
}
