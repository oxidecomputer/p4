// Copyright 2022 Oxide Computer Company

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

    pub fn accept<V: Visitor>(&self, v: &V) {
        for c in &self.constants {
            c.accept(v)
        }
        for h in &self.headers {
            h.accept(v);
        }
        for s in &self.structs {
            s.accept(v);
        }
        for t in &self.typedefs {
            t.accept(v);
        }
        for c in &self.controls {
            c.accept(v);
        }
        for p in &self.parsers {
            p.accept(v);
        }
        for p in &self.packages {
            p.accept(v);
        }
        for e in &self.externs {
            e.accept(v);
        }
        if let Some(p) = &self.package_instance {
            p.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        for c in &mut self.constants {
            c.accept_mut(v)
        }
        for h in &mut self.headers {
            h.accept_mut(v);
        }
        for s in &mut self.structs {
            s.accept_mut(v);
        }
        for t in &mut self.typedefs {
            t.accept_mut(v);
        }
        for c in &mut self.controls {
            c.accept_mut(v);
        }
        for p in &mut self.parsers {
            p.accept_mut(v);
        }
        for p in &mut self.packages {
            p.accept_mut(v);
        }
        for e in &mut self.externs {
            e.accept_mut(v);
        }
        if let Some(p) = &mut self.package_instance {
            p.accept_mut(v);
        }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.package_instance(self);
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.package_instance(self);
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.package(self);
        for p in &self.parameters {
            p.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.package(self);
        for p in &mut self.parameters {
            p.accept_mut(v);
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.package_parameter(self);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.package_parameter(self);
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
    Action,
    HeaderMethod,
}

impl Type {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.typ(self);
        if let Type::List(types) = self {
            for t in types {
                t.accept(v);
            }
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.typ(self);
        if let Type::List(types) = self {
            for t in types {
                t.accept_mut(v);
            }
        }
    }
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
            Type::Action => write!(f, "action"),
            Type::HeaderMethod => write!(f, "header method"),
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

impl Typedef {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.typedef(self);
        self.ty.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.typedef(self);
        self.ty.accept_mut(v);
    }
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub ty: Type,
    pub name: String,
    pub initializer: Box<Expression>,
}

impl Constant {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.constant(self);
        self.ty.accept(v);
        self.initializer.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.constant(self);
        self.ty.accept_mut(v);
        self.initializer.accept_mut(v);
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub ty: Type,
    pub name: String,
    pub initializer: Option<Box<Expression>>,
    pub parameters: Vec<ControlParameter>,
    pub token: Token,
}

impl Variable {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.variable(self);
        self.ty.accept(v);
        if let Some(init) = &self.initializer {
            init.accept(v);
        }
        for p in &self.parameters {
            p.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.variable(self);
        self.ty.accept_mut(v);
        if let Some(init) = &mut self.initializer {
            init.accept_mut(v);
        }
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
    }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.expression(self);
        match &self.kind {
            ExpressionKind::Lvalue(lv) => lv.accept(v),
            ExpressionKind::Binary(lhs, op, rhs) => {
                lhs.accept(v);
                op.accept(v);
                rhs.accept(v);
            }
            ExpressionKind::Index(lval, xpr) => {
                lval.accept(v);
                xpr.accept(v);
            }
            ExpressionKind::Slice(begin, end) => {
                begin.accept(v);
                end.accept(v);
            }
            ExpressionKind::Call(call) => call.accept(v),
            ExpressionKind::List(xprs) => {
                for xp in xprs {
                    xp.accept(v);
                }
            }
            _ => {} // covered by top level visit
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.expression(self);
        match &mut self.kind {
            ExpressionKind::Lvalue(lv) => lv.accept_mut(v),
            ExpressionKind::Binary(lhs, op, rhs) => {
                lhs.accept_mut(v);
                op.accept_mut(v);
                rhs.accept_mut(v);
            }
            ExpressionKind::Index(lval, xpr) => {
                lval.accept_mut(v);
                xpr.accept_mut(v);
            }
            ExpressionKind::Slice(begin, end) => {
                begin.accept_mut(v);
                end.accept_mut(v);
            }
            ExpressionKind::Call(call) => call.accept_mut(v),
            ExpressionKind::List(xprs) => {
                for xp in xprs {
                    xp.accept_mut(v);
                }
            }
            _ => {} // covered by top level visit
        }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.binop(self);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.binop(self);
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
        names.insert(
            "setValid".into(),
            NameInfo {
                ty: Type::HeaderMethod,
                decl: DeclarationInfo::Method,
            },
        );
        names.insert(
            "setInvalid".into(),
            NameInfo {
                ty: Type::HeaderMethod,
                decl: DeclarationInfo::Method,
            },
        );
        names.insert(
            "isValid".into(),
            NameInfo {
                ty: Type::HeaderMethod,
                decl: DeclarationInfo::Method,
            },
        );
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.header(self);
        for m in &self.members {
            m.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.header(self);
        for m in &mut self.members {
            m.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeaderMember {
    pub ty: Type,
    pub name: String,
    pub token: Token,
}

impl HeaderMember {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.header_member(self);
        self.ty.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.header_member(self);
        self.ty.accept_mut(v);
    }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.p4struct(self);
        for m in &self.members {
            m.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.p4struct(self);
        for m in &mut self.members {
            m.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructMember {
    pub ty: Type,
    pub name: String,
    pub token: Token,
}

impl StructMember {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.struct_member(self);
        self.ty.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.struct_member(self);
        self.ty.accept_mut(v);
    }
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
    pub fn tables<'a>(
        &'a self,
        ast: &'a AST,
    ) -> Vec<(Vec<(String, &Control)>, &Table)> {
        self.tables_rec(ast, String::new(), Vec::new())
    }

    fn tables_rec<'a>(
        &'a self,
        ast: &'a AST,
        name: String,
        mut chain: Vec<(String, &'a Control)>,
    ) -> Vec<(Vec<(String, &Control)>, &Table)> {
        let mut result = Vec::new();
        chain.push((name, self));
        for table in &self.tables {
            result.push((chain.clone(), table));
        }
        for v in &self.variables {
            if let Type::UserDefined(typename) = &v.ty {
                if let Some(control_inst) = ast.get_control(typename) {
                    result.extend_from_slice(&control_inst.tables_rec(
                        ast,
                        v.name.clone(),
                        chain.clone(),
                    ));
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
        for a in &self.actions {
            names.insert(
                a.name.clone(),
                NameInfo {
                    ty: Type::Action,
                    decl: DeclarationInfo::Action,
                },
            );
        }
        names
    }
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.control(self);
        for var in &self.variables {
            var.accept(v);
        }
        for c in &self.constants {
            c.accept(v);
        }
        for p in &self.parameters {
            p.accept(v);
        }
        for a in &self.actions {
            a.accept(v);
        }
        for t in &self.tables {
            t.accept(v);
        }
        for s in &self.apply.statements {
            s.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.control(self);
        for var in &mut self.variables {
            var.accept_mut(v);
        }
        for c in &mut self.constants {
            c.accept_mut(v);
        }
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
        for a in &mut self.actions {
            a.accept_mut(v);
        }
        for t in &mut self.tables {
            t.accept_mut(v);
        }
        for s in &mut self.apply.statements {
            s.accept_mut(v);
        }
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

    pub fn accept<V: Visitor>(&self, v: &V) {
        v.parser(self);
        for p in &self.parameters {
            p.accept(v);
        }
        for s in &self.states {
            s.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.parser(self);
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
        for s in &mut self.states {
            s.accept_mut(v);
        }
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

impl ControlParameter {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.control_parameter(self);
        self.ty.accept(v);
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.control_parameter(self);
        self.ty.accept_mut(v);
    }
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
                    decl: DeclarationInfo::ActionParameter(p.direction),
                },
            );
        }
        names
    }
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.action(self);
        for p in &self.parameters {
            p.accept(v);
        }
        for s in &self.statement_block.statements {
            s.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.action(self);
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
        for s in &mut self.statement_block.statements {
            s.accept_mut(v);
        }
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

impl ActionParameter {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.action_parameter(self);
        self.ty.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.action_parameter(self);
        self.ty.accept_mut(v);
    }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.table(self);
        for a in &self.actions {
            a.accept(v);
        }
        for (lval, mk) in &self.key {
            lval.accept(v);
            mk.accept(v);
        }
        for e in &self.const_entries {
            e.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.table(self);
        for a in &mut self.actions {
            a.accept_mut(v);
        }
        for (lval, mk) in &mut self.key {
            lval.accept_mut(v);
            mk.accept_mut(v);
        }
        for e in &mut self.const_entries {
            e.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConstTableEntry {
    pub keyset: Vec<KeySetElement>,
    pub action: ActionRef,
}

impl ConstTableEntry {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.const_table_entry(self);
        for k in &self.keyset {
            k.accept(v);
        }
        self.action.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.const_table_entry(self);
        for k in &mut self.keyset {
            k.accept_mut(v);
        }
        self.action.accept_mut(v);
    }
}

#[derive(Debug, Clone)]
pub struct KeySetElement {
    pub value: KeySetElementValue,
    pub token: Token,
}

impl KeySetElement {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.key_set_element(self);
        self.value.accept(v);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.key_set_element(self);
        self.value.accept_mut(v);
    }
}

#[derive(Debug, Clone)]
pub enum KeySetElementValue {
    Expression(Box<Expression>),
    Default,
    DontCare,
    Masked(Box<Expression>, Box<Expression>),
    Ranged(Box<Expression>, Box<Expression>),
}

impl KeySetElementValue {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.key_set_element_value(self);
        match self {
            KeySetElementValue::Expression(xpr) => xpr.accept(v),
            KeySetElementValue::Default => {}
            KeySetElementValue::DontCare => {}
            KeySetElementValue::Masked(val, mask) => {
                val.accept(v);
                mask.accept(v);
            }
            KeySetElementValue::Ranged(begin, end) => {
                begin.accept(v);
                end.accept(v);
            }
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.key_set_element_value(self);
        match self {
            KeySetElementValue::Expression(xpr) => xpr.accept_mut(v),
            KeySetElementValue::Default => {}
            KeySetElementValue::DontCare => {}
            KeySetElementValue::Masked(val, mask) => {
                val.accept_mut(v);
                mask.accept_mut(v);
            }
            KeySetElementValue::Ranged(begin, end) => {
                begin.accept_mut(v);
                end.accept_mut(v);
            }
        }
    }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.action_ref(self);
        for p in &self.parameters {
            p.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.action_ref(self);
        for p in &mut self.parameters {
            p.accept_mut(v);
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

impl MatchKind {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.match_kind(self);
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.match_kind(self);
    }
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

impl Statement {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.statement(self);
        match self {
            Statement::Empty => {}
            Statement::Assignment(lval, xpr) => {
                lval.accept(v);
                xpr.accept(v);
            }
            Statement::Call(call) => call.accept(v),
            Statement::If(if_block) => if_block.accept(v),
            Statement::Variable(var) => var.accept(v),
            Statement::Constant(constant) => constant.accept(v),
            Statement::Transition(transition) => transition.accept(v),
            Statement::Return(xpr) => {
                if let Some(rx) = xpr {
                    rx.accept(v);
                }
            }
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.statement(self);
        match self {
            Statement::Empty => {}
            Statement::Assignment(lval, xpr) => {
                lval.accept_mut(v);
                xpr.accept_mut(v);
            }
            Statement::Call(call) => call.accept_mut(v),
            Statement::If(if_block) => if_block.accept_mut(v),
            Statement::Variable(var) => var.accept_mut(v),
            Statement::Constant(constant) => constant.accept_mut(v),
            Statement::Transition(transition) => transition.accept_mut(v),
            Statement::Return(xpr) => {
                if let Some(rx) = xpr {
                    rx.accept_mut(v);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct IfBlock {
    pub predicate: Box<Expression>,
    pub block: StatementBlock,
    pub else_ifs: Vec<ElseIfBlock>,
    pub else_block: Option<StatementBlock>,
}

impl IfBlock {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.if_block(self);
        self.predicate.accept(v);
        for s in &self.block.statements {
            s.accept(v);
        }
        for ei in &self.else_ifs {
            ei.accept(v);
        }
        if let Some(eb) = &self.else_block {
            for s in &eb.statements {
                s.accept(v);
            }
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.if_block(self);
        self.predicate.accept_mut(v);
        for s in &mut self.block.statements {
            s.accept_mut(v);
        }
        for ei in &mut self.else_ifs {
            ei.accept_mut(v);
        }
        if let Some(eb) = &mut self.else_block {
            for s in &mut eb.statements {
                s.accept_mut(v);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ElseIfBlock {
    pub predicate: Box<Expression>,
    pub block: StatementBlock,
}

impl ElseIfBlock {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.else_if_block(self);
        self.predicate.accept(v);
        for s in &self.block.statements {
            s.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.else_if_block(self);
        self.predicate.accept_mut(v);
        for s in &mut self.block.statements {
            s.accept_mut(v);
        }
    }
}

/// A function or method call
#[derive(Debug, Clone)]
pub struct Call {
    pub lval: Lvalue,
    pub args: Vec<Box<Expression>>,
}

impl Call {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.call(self);
        self.lval.accept(v);
        for a in &self.args {
            a.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.call(self);
        self.lval.accept_mut(v);
        for a in &mut self.args {
            a.accept_mut(v);
        }
    }
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
    fn accept<V: Visitor>(&self, v: &V) {
        v.lvalue(self);
    }
    fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.lvalue(self);
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.state(self);
        for s in &self.statements.statements {
            s.accept(v);
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.state(self);
        for s in &mut self.statements.statements {
            s.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub enum Transition {
    Reference(Lvalue),
    Select(Select),
}

impl Transition {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.transition(self);
        match self {
            Transition::Reference(lval) => lval.accept(v),
            Transition::Select(sel) => sel.accept(v),
        }
    }
    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.transition(self);
        match self {
            Transition::Reference(lval) => lval.accept_mut(v),
            Transition::Select(sel) => sel.accept_mut(v),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Select {
    pub parameters: Vec<Box<Expression>>,
    pub elements: Vec<SelectElement>,
}

impl Select {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.select(self);
        for p in &self.parameters {
            p.accept(v);
        }
        for e in &self.elements {
            e.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.select(self);
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
        for e in &mut self.elements {
            e.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectElement {
    pub keyset: Vec<KeySetElement>,
    pub name: String,
}

impl SelectElement {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.select_element(self);
        for k in &self.keyset {
            k.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.select_element(self);
        for k in &mut self.keyset {
            k.accept_mut(v);
        }
    }
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
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.p4extern(self);
        for m in &self.methods {
            m.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.p4extern(self);
        for m in &mut self.methods {
            m.accept_mut(v);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExternMethod {
    pub return_type: Type,
    pub name: String,
    pub type_parameters: Vec<String>,
    pub parameters: Vec<ControlParameter>,
}

impl ExternMethod {
    pub fn accept<V: Visitor>(&self, v: &V) {
        v.extern_method(self);
        self.return_type.accept(v);
        for p in &self.parameters {
            p.accept(v);
        }
    }

    pub fn accept_mut<V: MutVisitor>(&mut self, v: &V) {
        v.extern_method(self);
        self.return_type.accept_mut(v);
        for p in &mut self.parameters {
            p.accept_mut(v);
        }
    }
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
    Action,
    ActionParameter(Direction),
}

#[derive(Debug, Clone)]
pub struct NameInfo {
    pub ty: Type,
    pub decl: DeclarationInfo,
}

pub trait Visitor {
    fn constant(&self, _: &Constant) {}
    fn header(&self, _: &Header) {}
    fn p4struct(&self, _: &Struct) {}
    fn typedef(&self, _: &Typedef) {}
    fn control(&self, _: &Control) {}
    fn parser(&self, _: &Parser) {}
    fn package(&self, _: &Package) {}
    fn package_instance(&self, _: &PackageInstance) {}
    fn p4extern(&self, _: &Extern) {}

    fn statement(&self, _: &Statement) {}
    fn action(&self, _: &Action) {}
    fn control_parameter(&self, _: &ControlParameter) {}
    fn action_parameter(&self, _: &ActionParameter) {}
    fn expression(&self, _: &Expression) {}
    fn header_member(&self, _: &HeaderMember) {}
    fn struct_member(&self, _: &StructMember) {}
    fn call(&self, _: &Call) {}
    fn typ(&self, _: &Type) {}
    fn binop(&self, _: &BinOp) {}
    fn lvalue(&self, _: &Lvalue) {}
    fn variable(&self, _: &Variable) {}
    fn if_block(&self, _: &IfBlock) {}
    fn else_if_block(&self, _: &ElseIfBlock) {}
    fn transition(&self, _: &Transition) {}
    fn select(&self, _: &Select) {}
    fn select_element(&self, _: &SelectElement) {}
    fn key_set_element(&self, _: &KeySetElement) {}
    fn key_set_element_value(&self, _: &KeySetElementValue) {}
    fn table(&self, _: &Table) {}
    fn match_kind(&self, _: &MatchKind) {}
    fn const_table_entry(&self, _: &ConstTableEntry) {}
    fn action_ref(&self, _: &ActionRef) {}
    fn state(&self, _: &State) {}
    fn package_parameter(&self, _: &PackageParameter) {}
    fn extern_method(&self, _: &ExternMethod) {}
}

pub trait MutVisitor {
    fn constant(&self, _: &mut Constant) {}
    fn header(&self, _: &mut Header) {}
    fn p4struct(&self, _: &mut Struct) {}
    fn typedef(&self, _: &mut Typedef) {}
    fn control(&self, _: &mut Control) {}
    fn parser(&self, _: &mut Parser) {}
    fn package(&self, _: &mut Package) {}
    fn package_instance(&self, _: &mut PackageInstance) {}
    fn p4extern(&self, _: &mut Extern) {}

    fn statement(&self, _: &mut Statement) {}
    fn action(&self, _: &mut Action) {}
    fn control_parameter(&self, _: &mut ControlParameter) {}
    fn action_parameter(&self, _: &mut ActionParameter) {}
    fn expression(&self, _: &mut Expression) {}
    fn header_member(&self, _: &mut HeaderMember) {}
    fn struct_member(&self, _: &mut StructMember) {}
    fn call(&self, _: &mut Call) {}
    fn typ(&self, _: &mut Type) {}
    fn binop(&self, _: &mut BinOp) {}
    fn lvalue(&self, _: &mut Lvalue) {}
    fn variable(&self, _: &mut Variable) {}
    fn if_block(&self, _: &mut IfBlock) {}
    fn else_if_block(&self, _: &mut ElseIfBlock) {}
    fn transition(&self, _: &mut Transition) {}
    fn select(&self, _: &mut Select) {}
    fn select_element(&self, _: &mut SelectElement) {}
    fn key_set_element(&self, _: &mut KeySetElement) {}
    fn key_set_element_value(&self, _: &mut KeySetElementValue) {}
    fn table(&self, _: &mut Table) {}
    fn match_kind(&self, _: &mut MatchKind) {}
    fn const_table_entry(&self, _: &mut ConstTableEntry) {}
    fn action_ref(&self, _: &mut ActionRef) {}
    fn state(&self, _: &mut State) {}
    fn package_parameter(&self, _: &mut PackageParameter) {}
    fn extern_method(&self, _: &mut ExternMethod) {}
}
