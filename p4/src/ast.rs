use std::cmp::{Eq, PartialEq};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::lexer::Token;

#[derive(Debug, Default)]
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

pub enum UserDefinedType<'a> {
    Struct(&'a Struct),
    Header(&'a Header),
    Extern(&'a Extern),
}

impl AST {
    pub fn get_struct(&self, name: &str) -> Option<&Struct> {
        self.structs.iter().find(|&s| s.name == name)
    }

    pub fn get_header(&self, name: &str) -> Option<&Header> {
        self.headers.iter().find(|&h| h.name == name)
    }

    pub fn get_extern(&self, name: &str) -> Option<&Extern> {
        self.externs.iter().find(|&e| e.name == name)
    }

    pub fn get_control(&self, name: &str) -> Option<&Control> {
        self.controls.iter().find(|&c| c.name == name)
    }

    pub fn get_parser(&self, name: &str) -> Option<&Parser> {
        self.parsers.iter().find(|&p| p.name == name)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bool,
    Error,
    Bit(usize),
    Varbit(usize),
    Int(usize),
    String,
    UserDefined(String),
    ExternFunction, //TODO actual signature
    Table,
    Void,
    List(Vec<Box<Type>>),
    State,
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
            Type::ExternFunction => write!(f, "extern function"),
            Type::Table => write!(f, "table"),
            Type::Void => write!(f, "void"),
            Type::State => write!(f, "state"),
            Type::List(elems) => {
                write!(f, "list<")?;
                for e in elems {
                    write!(f, "{},", e)?;
                }
                write!(f, ">")
            }
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
    pub initializer: Option<Box<Expression>>,
    pub parameters: Vec<ControlParameter>,
    pub token: Token,
}

#[derive(Debug, Clone)]
pub struct Expression {
    pub token: Token,
    pub kind: ExpressionKind,
}

impl Expression {
    pub fn new(token: Token, kind: ExpressionKind) -> Box<Self> {
        Box::new(Self { token, kind })
    }
}

impl Hash for Expression {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token.hash(state);
    }
}

impl PartialEq for Expression {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

impl Eq for Expression {}

#[derive(Debug, Clone)]
pub enum ExpressionKind {
    BoolLit(bool),
    IntegerLit(i128),
    BitLit(u16, u128),
    SignedLit(u16, i128),
    Lvalue(Lvalue),
    Binary(Box<Expression>, BinOp, Box<Expression>),
    Index(Lvalue, Box<Expression>),
    Slice(Box<Expression>, Box<Expression>),
    Call(Call),
    List(Vec<Box<Expression>>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Subtract,
    Geq,
    Eq,
    Mask,
    NotEq,
}

impl BinOp {
    pub fn english_verb(&self) -> &str {
        match self {
            BinOp::Add => "add",
            BinOp::Subtract => "subtract",
            BinOp::Geq | BinOp::Eq => "compare",
            BinOp::Mask => "mask",
            BinOp::NotEq => "not equal",
        }
    }
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
    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.members {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: p.ty.clone(),
                    decl: DeclarationInfo::HeaderMember,
                },
            );
        }
        names
    }
}

#[derive(Debug, Clone)]
pub struct HeaderMember {
    pub ty: Type,
    pub name: String,
    pub token: Token,
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

    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.members {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: p.ty.clone(),
                    decl: DeclarationInfo::StructMember,
                },
            );
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
    pub variables: Vec<Variable>,
    pub constants: Vec<Constant>,
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
            variables: Vec::new(),
            constants: Vec::new(),
            type_parameters: Vec::new(),
            parameters: Vec::new(),
            actions: Vec::new(),
            tables: Vec::new(),
            apply: StatementBlock::default(),
        }
    }

    pub fn get_parameter(&self, name: &str) -> Option<&ControlParameter> {
        self.parameters.iter().find(|&p| p.name == name)
    }

    pub fn get_action(&self, name: &str) -> Option<&Action> {
        self.actions.iter().find(|&a| a.name == name)
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|&t| t.name == name)
    }

    /// Return all the tables in this control block, recursively expanding local
    /// control block variables and including their tables. In the returned
    /// vector, the table in the second element of the tuple belongs to the
    /// control in the first element.
    pub fn tables<'a>(&'a self, ast: &'a AST) -> Vec<(Vec<&Control>, &Table)> {
        self.tables_rec(ast, Vec::new())
    }

    fn tables_rec<'a>(
        &'a self,
        ast: &'a AST,
        mut chain: Vec<&'a Control>,
    ) -> Vec<(Vec<&Control>, &Table)> {
        let mut result = Vec::new();
        chain.push(self);
        for table in &self.tables {
            result.push((chain.clone(), table));
        }
        for v in &self.variables {
            if let Type::UserDefined(name) = &v.ty {
                if let Some(control_inst) = ast.get_control(name) {
                    result.extend_from_slice(
                        &control_inst.tables_rec(ast, chain.clone()),
                    );
                }
            }
        }
        result
    }

    pub fn is_type_parameter(&self, name: &str) -> bool {
        for t in &self.type_parameters {
            if t == name {
                return true;
            }
        }
        false
    }

    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.parameters {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: p.ty.clone(),
                    decl: DeclarationInfo::Parameter(p.direction),
                },
            );
        }
        for t in &self.tables {
            names.insert(
                t.name.clone(),
                NameInfo {
                    ty: Type::Table,
                    decl: DeclarationInfo::ControlTable,
                },
            );
        }
        for v in &self.variables {
            names.insert(
                v.name.clone(),
                NameInfo {
                    ty: v.ty.clone(),
                    decl: DeclarationInfo::ControlMember,
                },
            );
        }
        for c in &self.constants {
            names.insert(
                c.name.clone(),
                NameInfo {
                    ty: c.ty.clone(),
                    decl: DeclarationInfo::ControlMember,
                },
            );
        }
        names
    }
}

impl PartialEq for Control {
    fn eq(&self, other: &Control) -> bool {
        self.name == other.name
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
                return true;
            }
        }
        false
    }

    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.parameters {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: p.ty.clone(),
                    decl: DeclarationInfo::Parameter(p.direction),
                },
            );
        }
        for s in &self.states {
            names.insert(
                s.name.clone(),
                NameInfo {
                    ty: Type::State,
                    decl: DeclarationInfo::State,
                },
            );
        }
        names
    }

    pub fn get_start_state(&self) -> Option<&State> {
        self.states.iter().find(|&s| s.name == "start")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
    InOut,
    Unspecified,
}

#[derive(Debug, Clone, Default)]
pub struct StatementBlock {
    pub statements: Vec<Statement>,
}

impl StatementBlock {
    fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }
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

    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.parameters {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: p.ty.clone(),
                    decl: DeclarationInfo::Parameter(p.direction),
                },
            );
        }
        names
    }
}

#[derive(Debug, Clone)]
pub struct ActionParameter {
    pub direction: Direction,
    pub ty: Type,
    pub name: String,

    pub ty_token: Token,
    pub name_token: Token,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub actions: Vec<Lvalue>,
    pub default_action: String,
    pub key: Vec<(Lvalue, MatchKind)>,
    pub const_entries: Vec<ConstTableEntry>,
    pub size: usize,
    pub token: Token,
}

impl Table {
    pub fn new(name: String, token: Token) -> Self {
        Self {
            name,
            actions: Vec::new(),
            default_action: String::new(),
            key: Vec::new(),
            const_entries: Vec::new(),
            size: 0,
            token,
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
    Range,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Empty,
    Assignment(Lvalue, Box<Expression>),
    //TODO get rid of this in favor of ExpressionKind::Call ???
    Call(Call),
    If(IfBlock),
    Variable(Variable),
    Constant(Constant),
    Transition(Transition),
    Return(Option<Box<Expression>>),
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
        self.name.split('.').collect()
    }
    pub fn root(&self) -> &str {
        self.parts()[0]
    }
    pub fn leaf(&self) -> &str {
        let parts = self.parts();
        parts[parts.len() - 1]
    }
    pub fn degree(&self) -> usize {
        self.parts().len()
    }
    pub fn pop_left(&self) -> Self {
        let parts = self.parts();
        Lvalue {
            name: parts[1..].join("."),
            token: Token {
                kind: self.token.kind.clone(),
                line: self.token.line,
                col: self.token.col + parts[0].len() + 1,
                file: self.token.file.clone(),
            },
        }
    }
    pub fn pop_right(&self) -> Self {
        let parts = self.parts();
        Lvalue {
            name: parts[..parts.len() - 1].join("."),
            token: Token {
                kind: self.token.kind.clone(),
                line: self.token.line,
                col: self.token.col,
                file: self.token.file.clone(),
            },
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
    pub statements: StatementBlock,
    pub token: Token,
}

impl State {
    pub fn new(name: String, token: Token) -> Self {
        Self {
            name,
            statements: StatementBlock::new(),
            token,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Transition {
    Reference(Lvalue),
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
    pub fn names(&self) -> HashMap<String, NameInfo> {
        let mut names = HashMap::new();
        for p in &self.methods {
            names.insert(
                p.name.clone(),
                NameInfo {
                    ty: Type::ExternFunction,
                    decl: DeclarationInfo::Method,
                },
            );
        }
        names
    }

    pub fn get_method(&self, name: &str) -> Option<&ExternMethod> {
        self.methods.iter().find(|&m| m.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct ExternMethod {
    pub return_type: Type,
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<ControlParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationInfo {
    Parameter(Direction),
    Method,
    StructMember,
    HeaderMember,
    Local,
    ControlTable,
    ControlMember,
    State,
}

#[derive(Debug, Clone)]
pub struct NameInfo {
    pub ty: Type,
    pub decl: DeclarationInfo,
}
