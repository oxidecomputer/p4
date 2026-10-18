// Copyright 2026 Oxide Computer Company

use std::collections::HashMap;

use crate::ast::{
    Call, Control, ControlParameter, DeclarationInfo, Direction, Expression,
    ExpressionKind, Header, Lvalue, NameInfo, Parser, State, Statement,
    StatementBlock, Struct, Table, Transition, Type, VisitorMut, AST,
};
use crate::hlir::{implicitly_converts, Hlir, HlirGenerator};
use crate::lexer::Token;
use colored::Colorize;

// TODO Check List
// This is a running list of things to check
//
// - Table keys should be constrained to bit, varbit, int

#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Level of this diagnostic.
    pub level: Level,

    /// Message associated with this diagnostic.
    pub message: String,

    /// The first token from the lexical element where the semantic error was
    /// detected.
    pub token: Token,
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub enum Level {
    Info,
    Deprecation,
    Warning,
    Error,
}

#[derive(Debug, Default)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl Diagnostics {
    pub fn new() -> Self {
        Diagnostics(Vec::new())
    }
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.0.iter().filter(|x| x.level == Level::Error).collect()
    }
    pub fn extend(&mut self, diags: &Diagnostics) {
        self.0.extend(diags.0.clone())
    }
    pub fn push(&mut self, d: Diagnostic) {
        self.0.push(d);
    }
}

pub fn all(ast: &AST) -> (Hlir, Diagnostics) {
    let mut diags = Diagnostics::new();
    let mut hg = HlirGenerator::new(ast);
    hg.run();
    diags.extend(&hg.diags);

    if !diags.errors().is_empty() {
        return (hg.hlir, diags);
    }

    for p in &ast.parsers {
        diags.extend(&ParserChecker::check(p, ast));
    }
    for c in &ast.controls {
        diags.extend(&ControlChecker::check(c, ast, &hg.hlir));
    }
    for s in &ast.structs {
        diags.extend(&StructChecker::check(s, ast));
    }
    for h in &ast.headers {
        diags.extend(&HeaderChecker::check(h, ast));
    }

    check_port_bitmap(ast, &mut diags);

    (hg.hlir, diags)
}

/// The egress port bitmap, e.g. for multicast replication.
///
/// Bit N (the bit set by `1 << N`) selects port N. The pipeline reads this
/// field after ingress.
///
/// A non-zero value takes precedence over `broadcast` and `port`. If filtering
/// excludes every selected port, the packet is dropped.
pub const PORT_BITMAP_FIELD: &str = "port_bitmap";

/// If declared, require `PORT_BITMAP_FIELD` in SoftNPU egress metadata to be a
/// `bit<N>`.
fn check_port_bitmap(ast: &AST, diags: &mut Diagnostics) {
    let Some(egress_struct) = ast
        .package_instance
        .as_ref()
        // SoftNPU(parser, ingress, egress) -> the egress control.
        .and_then(|inst| inst.parameters.get(2))
        .and_then(|name| ast.get_control(name))
        // Control parameters are (headers, ingress_meta, egress_meta).
        .and_then(|egress| egress.parameters.get(2))
        .and_then(|meta| match &meta.ty {
            Type::UserDefined(name) => ast.get_struct(name),
            _ => None,
        })
    else {
        return;
    };

    diags.0.extend(
        egress_struct
            .members
            .iter()
            .filter(|m| {
                m.name == PORT_BITMAP_FIELD && !matches!(m.ty, Type::Bit(_))
            })
            .map(|m| Diagnostic {
                level: Level::Error,
                message: format!(
                    "{PORT_BITMAP_FIELD} in egress metadata {} must be a \
                     bit<N>, found {}",
                    egress_struct.name, m.ty,
                ),
                token: m.token.clone(),
            }),
    );
}

pub struct ControlChecker {}

impl ControlChecker {
    pub fn check(c: &Control, ast: &AST, hlir: &Hlir) -> Diagnostics {
        let mut diags = Diagnostics::new();
        let names = c.names();
        Self::check_params(c, ast, &mut diags);
        Self::check_tables(c, &names, ast, &mut diags);
        Self::check_variables(c, ast, &mut diags);
        Self::check_actions(c, ast, hlir, &mut diags);
        Self::check_apply(c, ast, hlir, &mut diags);
        diags
    }

    pub fn check_params(c: &Control, ast: &AST, diags: &mut Diagnostics) {
        for p in &c.parameters {
            check_type_width(&p.ty, &p.ty_token, diags);
            check_parameter_type(p, diags);
            if let Type::UserDefined(typename) = &p.ty {
                if ast.get_user_defined_type(typename).is_none() {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Typename {} not found", typename),
                        token: p.ty_token.clone(),
                    })
                }
            }
        }
    }

    pub fn check_tables(
        c: &Control,
        names: &HashMap<String, NameInfo>,
        ast: &AST,
        diags: &mut Diagnostics,
    ) {
        for t in &c.tables {
            Self::check_table(c, t, names, ast, diags);
        }
    }

    pub fn check_table(
        c: &Control,
        t: &Table,
        names: &HashMap<String, NameInfo>,
        ast: &AST,
        diags: &mut Diagnostics,
    ) {
        for (lval, _match_kind) in &t.key {
            diags.extend(&check_lvalue(lval, ast, names, Some(&c.name)))
        }
        if t.default_action.is_empty() {
            diags.push(Diagnostic {
                level: Level::Error,
                message: "Table must have a default action".into(),
                token: t.token.clone(),
            });
        } else if !t
            .actions
            .iter()
            .any(|action| action.name == t.default_action)
        {
            // P4-16 14.2.1.3: the default action must be one of the actions
            // that appear in the actions list.
            diags.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "default action {} is not in the actions of table {}",
                    t.default_action, t.name,
                ),
                token: t.token.clone(),
            });
        }
    }

    pub fn check_variables(c: &Control, ast: &AST, diags: &mut Diagnostics) {
        for v in &c.variables {
            match &v.ty {
                Type::Bit(w) | Type::Varbit(w) | Type::Int(w) if *w > 128 => {
                    check_type_width(&v.ty, &v.token, diags);
                }
                Type::UserDefined(typename)
                    if ast.get_extern(typename).is_some()
                        || ast.get_control(typename).is_some() => {}
                Type::UserDefined(typename)
                    if ast.get_user_defined_type(typename).is_none() =>
                {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Typename {typename} not found"),
                        token: v.token.clone(),
                    });
                }
                _ => diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "A variable of type {} cannot be declared at control \
                         scope.",
                        v.ty,
                    ),
                    token: v.token.clone(),
                }),
            }
        }
    }

    pub fn check_actions(
        c: &Control,
        ast: &AST,
        hlir: &Hlir,
        diags: &mut Diagnostics,
    ) {
        for t in &c.tables {
            Self::check_table_action_reference(c, t, ast, diags);
        }
        for a in &c.actions {
            for p in &a.parameters {
                check_type_width(&p.ty, &p.ty_token, diags);
                check_runtime_type(&p.ty, &p.ty_token, diags);
            }
            check_statement_block(&a.statement_block, hlir, diags, ast, true);
        }
    }

    pub fn check_table_action_reference(
        c: &Control,
        t: &Table,
        _ast: &AST,
        diags: &mut Diagnostics,
    ) {
        for a in &t.actions {
            if a.name != "NoAction" && c.get_action(&a.name).is_none() {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "Table {} does not have action {}",
                        t.name, &a.name,
                    ),
                    token: a.token.clone(), //TODO plumb token for lvalue
                });
            }
        }
    }

    pub fn check_apply(
        c: &Control,
        ast: &AST,
        hlir: &Hlir,
        diags: &mut Diagnostics,
    ) {
        diags.extend(&check_statement_block_lvalues(&c.apply, ast, &c.names()));

        let mut apc = ApplyCallChecker {
            c,
            ast,
            hlir,
            diags,
        };
        c.accept_mut(&mut apc);
    }
}

fn check_statement_block(
    block: &StatementBlock,
    hlir: &Hlir,
    diags: &mut Diagnostics,
    ast: &AST,
    in_action: bool,
) {
    for s in &block.statements {
        match s {
            Statement::Empty => {}
            Statement::Call(c) if in_action => {
                let Some(name_info) =
                    hlir.lvalue_decls.get(&c.lval.pop_right())
                else {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Could not resolve lvalue {}",
                            &c.lval.name,
                        ),
                        token: c.lval.token.clone(),
                    });
                    continue;
                };
                match &name_info.ty {
                    Type::Table => {
                        diags.push(Diagnostic {
                            level: Level::Error,
                            message: String::from(
                                "Cannot apply table within action",
                            ),
                            token: c.lval.token.clone(),
                        });
                    }
                    Type::UserDefined(name) => {
                        if ast.get_control(name).is_some() {
                            diags.push(Diagnostic {
                                level: Level::Error,
                                message: String::from(
                                    "Cannot apply control within action",
                                ),
                                token: c.lval.token.clone(),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Statement::Variable(v) => {
                check_type_width(&v.ty, &v.token, diags);
                check_runtime_type(&v.ty, &v.token, diags);
            }
            Statement::If(ifb) => {
                check_statement_block(&ifb.block, hlir, diags, ast, in_action);
                for ei in &ifb.else_ifs {
                    check_statement_block(
                        &ei.block, hlir, diags, ast, in_action,
                    );
                }
                if let Some(eb) = &ifb.else_block {
                    check_statement_block(eb, hlir, diags, ast, in_action);
                }
            }
            _ => {
                // TODO: constants skip the width and int checks applied to
                // variables.
            }
        }
    }
}

pub struct ApplyCallChecker<'a> {
    c: &'a Control,
    ast: &'a AST,
    hlir: &'a Hlir,
    diags: &'a mut Diagnostics,
}

impl VisitorMut for ApplyCallChecker<'_> {
    fn call(&mut self, call: &Call) {
        let name = call.lval.root();
        let names = self.c.names();
        let Some(name_info) = names.get(name) else {
            self.diags.push(Diagnostic {
                level: Level::Error,
                message: format!("{name} is undefined"),
                token: call.lval.token.clone(),
            });
            return;
        };

        match &name_info.ty {
            Type::UserDefined(name) => {
                if let Some(ctl) = self.ast.get_control(name) {
                    self.check_apply_ctl_apply(call, ctl)
                }
            }
            Type::Table => {
                if let Some(tbl) = self.c.get_table(name) {
                    self.check_apply_table_apply(call, tbl)
                }
            }
            _ => {
                //TODO
            }
        }
    }
}

impl ApplyCallChecker<'_> {
    pub fn check_apply_table_apply(&mut self, _call: &Call, _tbl: &Table) {
        //TODO
    }

    pub fn check_apply_ctl_apply(&mut self, call: &Call, ctl: &Control) {
        if call.args.len() != ctl.parameters.len() {
            let signature: Vec<String> = ctl
                .parameters
                .iter()
                .map(|x| x.ty.to_string().bright_blue().to_string())
                .collect();

            let signature = format!("{}({})", ctl.name, signature.join(", "));

            self.diags.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "{} arguments provided to control {}, {} required\n    \
                    expected signature: {}",
                    call.args.len().to_string().yellow(),
                    ctl.name.blue(),
                    ctl.parameters.len().to_string().yellow(),
                    signature,
                ),
                token: call.lval.token.clone(),
            });
            return;
        }

        for (i, arg) in call.args.iter().enumerate() {
            let arg_t = match self.hlir.expression_types.get(arg) {
                Some(typ) => typ,
                None => panic!("bug: no type for expression {:?}", arg),
            };
            let param = &ctl.parameters[i];
            let compatible = match param.direction {
                Direction::Out | Direction::InOut => arg_t == &param.ty,
                _ => implicitly_converts(arg, arg_t, &param.ty),
            };
            if !compatible {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "wrong argument type for {} parameter {}\n    \
                         argument provided:  {}\n    \
                         parameter requires: {}",
                        ctl.name.bright_blue(),
                        param.name.bright_blue(),
                        format!("{}", arg_t).bright_blue(),
                        format!("{}", param.ty).bright_blue(),
                    ),
                    token: arg.token.clone(),
                });
            }
        }
    }
}

pub struct ParserChecker {}

impl ParserChecker {
    pub fn check(p: &Parser, ast: &AST) -> Diagnostics {
        let mut diags = Diagnostics::new();

        for param in &p.parameters {
            check_parameter_type(param, &mut diags);
        }

        if !p.decl_only {
            Self::start_state(p, &mut diags);
            for s in &p.states {
                Self::ensure_transition(s, &mut diags);
            }
            Self::lvalues(p, ast, &mut diags);
        }

        diags
    }

    /// Ensure the parser has a start state
    pub fn start_state(parser: &Parser, diags: &mut Diagnostics) {
        for s in &parser.states {
            if s.name == "start" {
                return;
            }
        }

        diags.push(Diagnostic {
            level: Level::Error,
            message: format!(
                "start state not found for parser {}",
                parser.name.bright_blue(),
            ),
            token: parser.token.clone(),
        });
    }

    pub fn ensure_transition(state: &State, diags: &mut Diagnostics) {
        let stmts = &state.statements.statements;

        if stmts.is_empty() {
            diags.push(Diagnostic {
                level: Level::Error,
                message: "state must include transition".into(),
                token: state.token.clone(),
            });
        }

        //TODO the right thing to do here is to ensure all code paths end in a
        //     transition, for now just check that the last statement is a
        //     transition.
        let last = stmts.last();
        if !matches!(last, Some(Statement::Transition(_))) {
            diags.push(Diagnostic {
                level: Level::Error,
                message: "final parser state statement must be a transition"
                    .into(),
                token: state.token.clone(),
            });
        }
    }

    /// Check lvalue references
    pub fn lvalues(parser: &Parser, ast: &AST, diags: &mut Diagnostics) {
        for state in &parser.states {
            // create a name context for each parser state to pick up any
            // variables that may get created within parser states to reference
            // locally.
            let names = parser.names();
            diags.extend(&check_statement_block_lvalues(
                &state.statements,
                ast,
                &names,
            ));
        }
    }
}

pub struct StructChecker {}

impl StructChecker {
    pub fn check(s: &Struct, ast: &AST) -> Diagnostics {
        let mut diags = Diagnostics::new();
        for m in &s.members {
            check_type_width(&m.ty, &m.token, &mut diags);
            check_runtime_type(&m.ty, &m.token, &mut diags);
            if let Type::UserDefined(typename) = &m.ty {
                if ast.get_user_defined_type(typename).is_none() {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Typename {} not found",
                            typename.bright_blue()
                        ),
                        token: m.token.clone(),
                    })
                }
            }
        }
        diags
    }
}

pub struct HeaderChecker {}

impl HeaderChecker {
    pub fn check(h: &Header, ast: &AST) -> Diagnostics {
        let mut diags = Diagnostics::new();
        for m in &h.members {
            check_type_width(&m.ty, &m.token, &mut diags);
            check_runtime_type(&m.ty, &m.token, &mut diags);
            if let Type::UserDefined(typename) = &m.ty {
                if ast.get_user_defined_type(typename).is_none() {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Typename {} not found",
                            typename.bright_blue()
                        ),
                        token: m.token.clone(),
                    })
                }
            }
        }
        diags
    }
}

/// Rust represents bit values as u128 for literals, shifts, and arithmetic
/// slice operations.
///
/// Control and action parameters, locals, and struct and header members wider
/// than 128 bits are rejected.
///
/// TODO: parser parameters and constants are not yet checked.
fn check_type_width(ty: &Type, token: &Token, diags: &mut Diagnostics) {
    if let Type::Bit(w) | Type::Varbit(w) | Type::Int(w) = ty {
        if *w > 128 {
            diags.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "Width {w} exceeds the 128-bit compiler limit",
                ),
                token: token.clone(),
            });
        }
    }
}

/// Reject directional `int` parameters (P4-16 7.1.6.5) and directionless ones,
/// which SoftNPU cannot bind.
fn check_parameter_type(p: &ControlParameter, diags: &mut Diagnostics) {
    match (p.direction, &p.ty) {
        (Direction::Unspecified, Type::ArbitraryInt) => {
            diags.push(Diagnostic {
                level: Level::Error,
                message: "directionless int parameters are not supported by \
                          SoftNPU"
                    .into(),
                token: p.ty_token.clone(),
            })
        }
        (_, Type::ArbitraryInt) => diags.push(Diagnostic {
            level: Level::Error,
            message: "int parameters must be directionless".into(),
            token: p.ty_token.clone(),
        }),
        _ => {}
    }
}

/// Reject `int` in positions that hold runtime values (P4-16 7.1.6.5).
fn check_runtime_type(ty: &Type, token: &Token, diags: &mut Diagnostics) {
    if let Type::ArbitraryInt = ty {
        diags.push(Diagnostic {
            level: Level::Error,
            message: "int is only valid for compile-time known values".into(),
            token: token.clone(),
        });
    }
}

fn check_name(
    name: &str,
    names: &HashMap<String, NameInfo>,
    token: &Token,
    parent: Option<&str>,
) -> (Diagnostics, Option<Type>) {
    match names.get(name) {
        Some(name_info) => (Diagnostics::new(), Some(name_info.ty.clone())),
        None => (
            Diagnostics(vec![Diagnostic {
                level: Level::Error,
                message: match parent {
                    Some(p) => format!(
                        "{} does not have member {}",
                        p.bright_blue(),
                        name.bright_blue(),
                    ),
                    None => format!("'{}' is undefined", name),
                },
                token: token.clone(),
            }]),
            None,
        ),
    }
}

fn check_statement_lvalues(
    stmt: &Statement,
    ast: &AST,
    names: &mut HashMap<String, NameInfo>,
) -> Diagnostics {
    let mut diags = Diagnostics::new();
    match stmt {
        Statement::Empty => {}
        Statement::Variable(v) => {
            check_type_width(&v.ty, &v.token, &mut diags);
            check_runtime_type(&v.ty, &v.token, &mut diags);
            if let Some(expr) = &v.initializer {
                diags.extend(&check_expression_lvalues(
                    expr.as_ref(),
                    ast,
                    names,
                ));
            }
            names.insert(
                v.name.clone(),
                NameInfo {
                    ty: v.ty.clone(),
                    decl: DeclarationInfo::Local,
                },
            );
        }
        Statement::Constant(c) => {
            diags.extend(&check_expression_lvalues(
                c.initializer.as_ref(),
                ast,
                names,
            ));
        }
        Statement::Assignment(lval, expr) => {
            diags.extend(&check_lvalue(lval, ast, names, None));
            diags.extend(&check_expression_lvalues(expr, ast, names));
        }
        Statement::SliceAssignment(lval, _hi, _lo, expr) => {
            diags.extend(&check_lvalue(lval, ast, names, None));
            diags.extend(&check_expression_lvalues(expr, ast, names));
        }
        Statement::Call(call) => {
            diags.extend(&check_lvalue(&call.lval, ast, names, None));
            for arg in &call.args {
                diags.extend(&check_expression_lvalues(
                    arg.as_ref(),
                    ast,
                    names,
                ));
            }
        }
        Statement::If(if_block) => {
            diags.extend(&check_expression_lvalues(
                if_block.predicate.as_ref(),
                ast,
                names,
            ));
            diags.extend(&check_statement_block_lvalues(
                &if_block.block,
                ast,
                names,
            ));
            for elif in &if_block.else_ifs {
                diags.extend(&check_expression_lvalues(
                    elif.predicate.as_ref(),
                    ast,
                    names,
                ));
                diags.extend(&check_statement_block_lvalues(
                    &elif.block,
                    ast,
                    names,
                ));
            }
            if let Some(ref else_block) = if_block.else_block {
                diags.extend(&check_statement_block_lvalues(
                    else_block, ast, names,
                ));
            }
        }
        Statement::Transition(transition) => {
            match transition {
                Transition::Reference(lval) => {
                    if lval.name != "accept" && lval.name != "reject" {
                        diags.extend(&check_lvalue(lval, ast, names, None));
                    }
                }
                Transition::Select(_sel) => {
                    //TODO
                }
            }
        }
        Statement::Return(xpr) => {
            if let Some(xpr) = xpr {
                diags.extend(&check_expression_lvalues(
                    xpr.as_ref(),
                    ast,
                    names,
                ));
            }
        }
    }
    diags
}

fn check_statement_block_lvalues(
    block: &StatementBlock,
    ast: &AST,
    names: &HashMap<String, NameInfo>,
) -> Diagnostics {
    let mut diags = Diagnostics::new();
    let mut block_names = names.clone();
    for stmt in &block.statements {
        diags.extend(&check_statement_lvalues(stmt, ast, &mut block_names));
    }
    diags
}

fn check_expression_lvalues(
    xpr: &Expression,
    ast: &AST,
    names: &HashMap<String, NameInfo>,
) -> Diagnostics {
    match &xpr.kind {
        ExpressionKind::Lvalue(lval) => check_lvalue(lval, ast, names, None),
        ExpressionKind::Binary(lhs, _, rhs) => {
            let mut diags = Diagnostics::new();
            diags.extend(&check_expression_lvalues(lhs.as_ref(), ast, names));
            diags.extend(&check_expression_lvalues(rhs.as_ref(), ast, names));
            diags
        }
        ExpressionKind::Index(lval, xpr) => {
            let mut diags = Diagnostics::new();
            diags.extend(&check_lvalue(lval, ast, names, None));
            diags.extend(&check_expression_lvalues(xpr.as_ref(), ast, names));
            diags
        }
        ExpressionKind::Slice(lhs, rhs) => {
            let mut diags = Diagnostics::new();
            diags.extend(&check_expression_lvalues(lhs.as_ref(), ast, names));
            diags.extend(&check_expression_lvalues(rhs.as_ref(), ast, names));
            diags
        }
        _ => Diagnostics::new(),
    }
}

fn check_lvalue(
    lval: &Lvalue,
    ast: &AST,
    names: &HashMap<String, NameInfo>,
    parent: Option<&str>,
) -> Diagnostics {
    let parts = lval.parts();

    let ty = match check_name(parts[0], names, &lval.token, parent) {
        (_, Some(ty)) => ty,
        (diags, None) => return diags,
    };

    let mut diags = Diagnostics::new();

    match ty {
        Type::Bool => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "bool".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::State => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "state".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Action => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "action".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Error => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "error".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Bit(size) => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        format!("bit<{}>", size).bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Varbit(size) => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        format!("varbit<{}>", size).bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Int(size) => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type int<{}> does not have a member {}",
                        format!("int<{}>", size).bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::ArbitraryInt => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "int".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::String => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "string".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::ExternFunction => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: "extern functions do not have members".into(),
                    token: lval.token.clone(),
                });
            }
        }
        Type::HeaderMethod => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: "header methods do not have members".into(),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Table => {
            if parts.len() > 1 && parts.last() != Some(&"apply") {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "table".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::Void => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "void".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::List(_) => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} does not have a member {}",
                        "list".bright_blue(),
                        parts[1].bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
        Type::UserDefined(name) => {
            // get the parent type definition from the AST and check for the
            // referenced member
            if let Some(parent) = ast.get_struct(&name) {
                if parts.len() > 1 {
                    let mut struct_names = names.clone();
                    struct_names.extend(parent.names());
                    let sub_lval = lval.pop_left();
                    let sub_diags = check_lvalue(
                        &sub_lval,
                        ast,
                        &struct_names,
                        Some(&parent.name),
                    );
                    diags.extend(&sub_diags);
                }
            } else if let Some(parent) = ast.get_header(&name) {
                if parts.len() > 1 {
                    let mut header_names = names.clone();
                    header_names.extend(parent.names());
                    let sub_lval = lval.pop_left();
                    let sub_diags = check_lvalue(
                        &sub_lval,
                        ast,
                        &header_names,
                        Some(&parent.name),
                    );
                    diags.extend(&sub_diags);
                }
            } else if let Some(parent) = ast.get_extern(&name) {
                if parts.len() > 1 {
                    let mut extern_names = names.clone();
                    extern_names.extend(parent.names());
                    let sub_lval = lval.pop_left();
                    let sub_diags = check_lvalue(
                        &sub_lval,
                        ast,
                        &extern_names,
                        Some(&parent.name),
                    );
                    diags.extend(&sub_diags);
                }
            } else if let Some(_control) = ast.get_control(&name) {
                if parts.len() > 1 && parts.last() != Some(&"apply") {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Control {} has no member {}",
                            name.bright_blue(),
                            parts.last().unwrap().bright_blue(),
                        ),
                        token: lval.token.clone(),
                    });
                }
            } else {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type {} is not defined",
                        name.bright_blue(),
                    ),
                    token: lval.token.clone(),
                });
            }
        }
    };
    diags
}

#[cfg(test)]
mod tests {
    use crate::ast::AST;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use std::sync::Arc;

    fn check_p4(source: &str) -> super::Diagnostics {
        let lines: Vec<&str> = source.lines().collect();
        let filename = Arc::new("test.p4".to_string());
        let lexer = Lexer::new(lines, filename);
        let mut parser = Parser::new(lexer);
        let mut ast = AST::default();
        parser.run(&mut ast).expect("parse failed");
        let (_hlir, diags) = crate::check::all(&ast);
        diags
    }

    #[test]
    fn width_128_accepted() {
        let source = r#"
header h_t {
    bit<128> f;
}
struct headers_t {
    h_t h;
}
control ingress(inout headers_t hdr) {
    apply {
        bit<128> x = hdr.h.f;
    }
}
"#;
        let diags = check_p4(source);
        let errors = diags.errors();
        assert!(
            errors.is_empty(),
            "unexpected errors: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn control_data_variables_rejected() {
        let source =
            include_str!("../examples/bad/checker/control-data-variables.p4");

        let diags = check_p4(source);
        let errors = diags.errors();
        let expected = ["bit<16>", "bit<8>", "int<8>", "bool", "h_t", "data_t"];
        assert_eq!(
            errors.len(),
            expected.len(),
            "{:?}",
            errors.iter().map(|diag| &diag.message).collect::<Vec<_>>(),
        );

        for (error, ty) in errors.iter().zip(expected) {
            assert_eq!(
                error.message,
                format!(
                    "A variable of type {ty} cannot be declared at control \
                     scope."
                ),
            );
        }
    }

    #[test]
    fn control_instances_and_block_locals_accepted() {
        let source = include_str!("../examples/control-instances.p4");
        let diags = check_p4(source);
        let errors = diags.errors();
        assert!(
            errors.is_empty(),
            "{:?}",
            errors.iter().map(|diag| &diag.message).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn width_over_128_rejected() {
        let source = r#"
header h_t {
    bit<129> f;
}
struct metadata_t {
    bit<130> f;
}
struct headers_t {
    h_t h;
}
control ingress(inout headers_t hdr, in bit<131> parameter) {
    bit<132> control_variable;
    action a(bit<133> action_parameter) {
        bit<134> action_variable;
    }
    apply {
        bit<135> apply_variable;
    }
}
"#;
        let diags = check_p4(source);
        let errors = diags.errors();
        assert_eq!(
            errors.len(),
            7,
            "expected an error for each declaration site: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
        );
        let messages: Vec<_> =
            errors.iter().map(|error| &error.message).collect();
        for width in 129..=135 {
            assert!(
                messages.iter().any(|message| message
                    .contains(&format!("Width {width} exceeds"))),
                "missing diagnostic for width {width}: {messages:?}",
            );
        }
    }

    const PORT_BITMAP_PIPELINE: &str = r#"
struct headers_t {
    bit<8> f;
}
struct ingress_metadata_t {
    bit<16> port;
}
struct egress_metadata_t {
    PORT_BITMAP_TYPE port_bitmap;
}
parser parse(inout headers_t hdr) {
    state start {
        transition accept;
    }
}
control ingress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply { }
}
control egress(
    inout headers_t hdr,
    inout ingress_metadata_t ingress,
    inout egress_metadata_t egress,
) {
    apply { }
}
SoftNPU(parse(), ingress(), egress()) main;
"#;

    #[test]
    fn control_checks_follow_hlir() {
        let source = r#"
header h_t {
    bit<8> f;
}
struct headers_t {
    h_t h;
}
control child(in bool valid, in bit<8> value) {
    apply { }
}
control ingress(inout headers_t hdr) {
    child() c;
    table t {
        key = { hdr.h.f: exact; }
        actions = { NoAction; }
        default_action = NoAction;
    }
    action nested() {
        if (true) {
            t.apply();
        }
    }
    apply {
        c.apply(hdr.h.isValid(), 5);
    }
}
"#;
        let diags = check_p4(source);
        let errors: Vec<_> =
            diags.errors().iter().map(|d| d.message.clone()).collect();
        assert_eq!(errors, ["Cannot apply table within action"]);
    }

    #[test]
    fn control_out_parameter_rejects_literal() {
        let source = r#"
control child(out bit<8> value) {
    apply { }
}
control ingress() {
    child() c;
    apply {
        c.apply(5);
    }
}
"#;
        let diags = check_p4(source);
        let errors: Vec<_> =
            diags.errors().iter().map(|d| d.message.clone()).collect();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with("wrong argument type for"),
            "{errors:?}",
        );
    }

    #[test]
    fn default_action_must_be_listed() {
        let source = r#"
header h_t {
    bit<8> f;
}
struct headers_t {
    h_t h;
}
control ingress(inout headers_t hdr) {
    action hit() { }
    table arp {
        key = { hdr.h.f: exact; }
        actions = { hit; NoAction; }
        default_action = NoAction;
    }
    table routes {
        key = { hdr.h.f: exact; }
        actions = { hit; }
        default_action = NoAction;
    }
    apply {
        arp.apply();
        routes.apply();
    }
}
"#;
        let diags = check_p4(source);
        let errors: Vec<_> =
            diags.errors().iter().map(|d| d.message.clone()).collect();
        assert_eq!(
            errors,
            ["default action NoAction is not in the actions of table routes"],
        );
    }

    #[test]
    fn arbitrary_int_rejected_at_runtime() {
        let source = r#"
header h_t {
    int a;
    int<8> b;
}
struct s_t {
    int c;
}
parser parse(packet_in pkt, in int d, int j) {
    state start {
        transition accept;
    }
}
control ingress(in int e, int i) {
    action act(int f) {
        int g;
    }
    apply {
        int h;
    }
}
"#;
        let runtime = "int is only valid for compile-time known values";
        let directional = "int parameters must be directionless";
        let directionless =
            "directionless int parameters are not supported by SoftNPU";
        let diags = check_p4(source);
        let errors: Vec<_> =
            diags.errors().iter().map(|d| d.message.clone()).collect();
        assert_eq!(
            errors,
            [
                directional,
                directionless,
                directional,
                directionless,
                runtime,
                runtime,
                runtime,
                runtime,
                runtime,
            ],
        );
    }

    #[test]
    fn port_bitmap_accepts_bit_type() {
        let source =
            PORT_BITMAP_PIPELINE.replace("PORT_BITMAP_TYPE", "bit<128>");
        let diags = check_p4(&source);
        let errors = diags.errors();
        assert!(
            errors.is_empty(),
            "unexpected errors: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn port_bitmap_rejects_non_bit_type() {
        let source = PORT_BITMAP_PIPELINE.replace("PORT_BITMAP_TYPE", "bool");
        let diags = check_p4(&source);
        let errors = diags.errors();
        assert!(
            errors.iter().any(|d| d.message.contains(
                "port_bitmap in egress metadata egress_metadata_t must be a \
                 bit<N>"
            )),
            "missing port_bitmap type diagnostic: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>(),
        );
    }
}
