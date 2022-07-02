use std::fmt;
use std::collections::HashMap;

use crate::lexer::Token;

#[derive(Debug)]
pub struct AST {
    pub constants: Vec<Constant>,
    pub headers: Vec<Header>,
    pub structs: Vec<Struct>,
    pub typedefs: Vec<Typedef>,
    pub controls: Vec<Control>,
    pub parsers: Vec<Parser>,
    pub packages: Vec<Package>,
    pub package_instance: Option<PackageInstance>,
    pub externs: Vec<Extern>,
}

impl Default for AST {
    fn default() -> Self {
        Self {
            constants: Vec::new(),
            headers: Vec::new(),
            structs: Vec::new(),
            typedefs: Vec::new(),
            controls: Vec::new(),
            parsers: Vec::new(),
            packages: Vec::new(),
            package_instance: None,
            externs: Vec::new(),
        }
    }
}

pub enum UserDefinedType<'a> {
    Struct(&'a Struct),
    Header(&'a Header),
    Extern(&'a Extern),
}

impl AST {
    pub fn get_struct(&self, name: &str) -> Option<&Struct> {
        for s in &self.structs {
            if s.name == name {
                return Some(s);
            }
        }
        None
    }

    pub fn get_header(&self, name: &str) -> Option<&Header> {
        for h in &self.headers {
            if h.name == name {
                return Some(h);
            }
        }
        None
    }

    pub fn get_extern(&self, name: &str) -> Option<&Extern> {
        for e in &self.externs {
            if e.name == name {
                return Some(e);
            }
        }
        None
    }

    pub fn get_user_defined_type(&self, name: &str) -> Option<UserDefinedType> {
        if let Some(user_struct) = self.get_struct(name) {
            return Some(UserDefinedType::Struct(user_struct));
        }
        if let Some(user_header) = self.get_header(name) {
            return Some(UserDefinedType::Header(user_header));
        }
        if let Some(platform_extern) = self.get_extern(name) {
            return Some(UserDefinedType::Extern(platform_extern));
        }
        None
    }
    
    pub fn type_map(&self) -> HashMap<String, Type> {
        let mut tm = HashMap::new();
        for x in &self.structs {
            tm.insert(x.name.clone(), Type::UserDefined(x.name.clone()));
        }
        tm
    }
}

#[derive(Debug)]
pub struct PackageInstance {
    pub instance_type: String,
    pub name: String,
    pub parameters: Vec<String>,
}

impl PackageInstance {
    pub fn new(instance_type: String) -> Self {
        Self {
            instance_type,
            name: "".into(),
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<PackageParameter>,
}

impl Package {
    pub fn new(name: String) -> Self {
        Self {
            name,
            type_parameters: Vec::new(),
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct PackageParameter {
    pub type_name: String,
    pub type_parameters: Vec<String>,
    pub name: String,
}

impl PackageParameter {
    pub fn new(type_name: String) -> Self {
        Self {
            type_name,
            type_parameters: Vec::new(),
            name: String::new(),
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
    ExternFunction, //TODO actual signature
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Type::Bool => write!(f, "bool"),
            Type::Error => write!(f, "error"),
            Type::Bit(size) => write!(f, "bit<{}>", size),
            Type::Varbit(size) => write!(f, "varbit<{}>", size),
            Type::Int(size) => write!(f, "int<{}>", size),
            Type::String => write!(f, "string"),
            Type::UserDefined(name) => write!(f, "{}", name),
            Type::ExternFunction => write!(f, "Extern Function"),
        }
    }
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
    pub initializer: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub ty: Type,
    pub name: String,
    //TODO initializer: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    BoolLit(bool),
    IntegerLit(i128),
    BitLit(u16, u128),
    SignedLit(u16, i128),
    Lvalue(Lvalue),
    Binary(Box<Expression>, BinOp, Box<Expression>),
    Index(Lvalue, Box<Expression>),
    Slice(Box<Expression>, Box<Expression>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Subtract,
    Geq,
    Eq,
    Mask,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub name: String,
    pub members: Vec<HeaderMember>,
}

impl Header {
    pub fn new(name: String) -> Self {
        Header {
            name,
            members: Vec::new(),
        }
    }
    pub fn names(&self) -> HashMap::<String, Type> {
        let mut names = HashMap::new();
        for p in &self.members {
            names.insert(p.name.clone(), p.ty.clone());
        }
        names
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
    pub members: Vec<StructMember>,
}

impl Struct {
    pub fn new(name: String) -> Self {
        Struct {
            name,
            members: Vec::new(),
        }
    }

    pub fn names(&self) -> HashMap::<String, Type> {
        let mut names = HashMap::new();
        for p in &self.members {
            names.insert(p.name.clone(), p.ty.clone());
        }
        names
    }
}

#[derive(Debug, Clone)]
pub struct StructMember {
    pub ty: Type,
    pub name: String,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct Control {
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<ControlParameter>,
    pub actions: Vec<Action>,
    pub tables: Vec<Table>,
    pub apply: StatementBlock,
}

impl Control {
    pub fn new(name: String) -> Self {
        Self {
            name,
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            actions: Vec::new(),
            tables: Vec::new(),
            apply: StatementBlock::default(),
        }
    }

    pub fn get_parameter(&self, name: &str) -> Option<&ControlParameter> {
        for p in &self.parameters {
            if p.name == name {
                return Some(p);
            }
        }
        None
    }

    pub fn get_action(&self, name: &str) -> Option<&Action> {
        for a in &self.actions {
            if a.name == name {
                return Some(a);
            }
        }
        None
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        for t in &self.tables {
            if t.name == name {
                return Some(t);
            }
        }
        None
    }

    pub fn is_type_parameter(&self, name: &str) -> bool {
        for t in &self.type_parameters {
            if t == name {
                return true
            }
        }
        return false;
    }
    
    pub fn names(&self) -> HashMap::<String, Type> {
        let mut names = HashMap::new();
        for p in &self.parameters {
            names.insert(p.name.clone(), p.ty.clone());
        }
        names
    }
}

#[derive(Debug, Clone)]
pub struct Parser {
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<ControlParameter>,
    pub states: Vec<State>,
    pub decl_only: bool,

    /// The first token of this parser, used for error reporting.
    pub token: Token,
}

impl Parser {
    pub fn new(name: String, token: Token) -> Self {
        Self {
            name,
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            states: Vec::new(),
            decl_only: false,
            token,
        }
    }

    pub fn is_type_parameter(&self, name: &str) -> bool {
        for t in &self.type_parameters {
            if t == name {
                return true
            }
        }
        return false;
    }
}

#[derive(Debug, Clone)]
pub struct ControlParameter {
    pub direction: Direction,
    pub ty: Type,
    pub name: String,

    pub dir_token: Token,
    pub ty_token: Token,
    pub name_token: Token,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    In,
    Out,
    InOut,
    Unspecified,
}

#[derive(Debug, Clone, Default)]
pub struct StatementBlock {
    pub variables: Vec<Variable>,
    pub constants: Vec<Constant>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub name: String,
    pub parameters: Vec<ActionParameter>,
    pub statement_block: StatementBlock,
}

impl Action {
    pub fn new(name: String) -> Self {
        Self {
            name,
            parameters: Vec::new(),
            statement_block: StatementBlock::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionParameter {
    pub ty: Type,
    pub name: String,

    pub ty_token: Token,
    pub name_token: Token,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub actions: Vec<String>,
    pub default_action: String,
    pub key: Vec<(Lvalue, MatchKind)>,
    pub const_entries: Vec<ConstTableEntry>,
    pub size: usize,
}

impl Table {
    pub fn new(name: String) -> Self {
        Self {
            name,
            actions: Vec::new(),
            default_action: String::new(),
            key: Vec::new(),
            const_entries: Vec::new(),
            size: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstTableEntry {
    pub keyset: Vec<KeySetElement>,
    pub action: ActionRef,
}

#[derive(Debug, Clone)]
pub struct KeySetElement {
    pub value: KeySetElementValue,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub enum KeySetElementValue {
    Expression(Box<Expression>),
    Default,
    DontCare,
    Masked(Box<Expression>, Box<Expression>),
    Ranged(Box<Expression>, Box<Expression>),
}

#[derive(Debug, Clone)]
pub struct ActionRef {
    pub name: String,
    pub parameters: Vec<Box<Expression>>,

    pub token: Token,
}

impl ActionRef {
    pub fn new(name: String, token: Token) -> Self {
        Self {
            name,
            token,
            parameters: Vec::new(),
        }
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
    Assignment(Lvalue, Box<Expression>),
    Call(Call),
    If(IfBlock),

    // TODO ...
}

#[derive(Debug, Clone)]
pub struct IfBlock {
    pub predicate: Box<Expression>,
    pub block: StatementBlock,
    pub else_ifs: Vec<ElseIfBlock>,
    pub else_block: Option<StatementBlock>,
}

#[derive(Debug, Clone)]
pub struct ElseIfBlock {
    pub predicate: Box<Expression>,
    pub block: StatementBlock,
}

/// A function or method call
#[derive(Debug, Clone)]
pub struct Call {
    pub lval: Lvalue,
    pub args: Vec<Box<Expression>>,
}

#[derive(Debug, Clone)]
pub struct Lvalue {
    pub name: String,
    pub token: Token,
}

impl Lvalue {
    pub fn parts(&self) -> Vec<&str> {
        self.name.split(".").collect()
    }
    pub fn root(&self) -> &str {
        self.parts()[0]
    }
    pub fn pop_left(&self) -> Self {
        let parts = self.parts();
        Lvalue{
            name: parts[1..].join("."),
            token: Token{
                kind: self.token.kind.clone(),
                line: self.token.line,
                col: self.token.col + parts[0].len() + 1,
            }
        }
    }
}

impl std::hash::Hash for Lvalue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Lvalue {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Lvalue {}

impl PartialOrd for Lvalue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Lvalue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

#[derive(Debug, Clone)]
pub struct State {
    pub name: String,
    pub variables: Vec<Variable>,
    pub constants: Vec<Constant>,
    pub statements: Vec<Statement>,
    pub transition: Option<Transition>,
}

impl State {
    pub fn new(name: String) -> Self {
        Self {
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
    pub parameters: Vec<Box<Expression>>,
    pub elements: Vec<SelectElement>,
}

#[derive(Debug, Clone)]
pub struct SelectElement {
    pub keyset: Vec<KeySetElement>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Extern {
    pub name: String,
    pub methods: Vec<ExternMethod>,
    pub token: Token,
}

impl Extern {
    pub fn names(&self) -> HashMap::<String, Type> {
        let mut names = HashMap::new();
        for p in &self.methods{
            names.insert(p.name.clone(), Type::ExternFunction);
        }
        names
    }
}

#[derive(Debug, Clone)]
pub struct ExternMethod {
    pub return_type: Type,
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<ControlParameter>,
}
