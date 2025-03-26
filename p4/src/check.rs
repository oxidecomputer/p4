// Copyright 2022 Oxide Computer Company

use std::collections::HashMap;

use crate::ast::{
    Call, Control, DeclarationInfo, Expression, ExpressionKind, Header, Lvalue,
    NameInfo, Parser, State, Statement, StatementBlock, Struct, Table,
    Transition, Type, VisitorMut, AST,
};
use crate::hlir::{Hlir, HlirGenerator};
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
    (hg.hlir, diags)
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
        }
    }

    pub fn check_variables(c: &Control, ast: &AST, diags: &mut Diagnostics) {
        for v in &c.variables {
            if let Type::UserDefined(typename) = &v.ty {
                if ast.get_user_defined_type(typename).is_some() {
                    continue;
                }
                if ast.get_control(typename).is_some() {
                    continue;
                }
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!("Typename {} not found", typename),
                    token: v.token.clone(),
                })
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
            if c.get_action(&a.name).is_none() {
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
            Statement::Assignment(lval, xpr) => {
                let name_info = match hlir.lvalue_decls.get(lval) {
                    Some(info) => info,
                    None => {
                        diags.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Could not resolve lvalue {}",
                                &lval.name,
                            ),
                            token: lval.token.clone(),
                        });
                        return;
                    }
                };

                let expression_type =
                    match hlir.expression_types.get(xpr.as_ref()) {
                        Some(ty) => ty,
                        None => {
                            diags.push(Diagnostic {
                                level: Level::Error,
                                message: "Could not determine expression type"
                                    .to_owned(),
                                token: xpr.token.clone(),
                            });
                            return;
                        }
                    };

                if &name_info.ty != expression_type {
                    diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Cannot assign {} to {}",
                            expression_type, &name_info.ty,
                        ),
                        token: xpr.token.clone(),
                    });
                }
            }
            Statement::Empty => {}
            Statement::Call(c) if in_action => {
                let lval = c.lval.pop_right();
                let name_info = match hlir.lvalue_decls.get(&lval) {
                    Some(info) => info,
                    None => {
                        diags.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Could not resolve lvalue {}",
                                &c.lval.name,
                            ),
                            token: c.lval.token.clone(),
                        });
                        return;
                    }
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
            _ => {
                // TODO
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
        let name_info = match names.get(name) {
            Some(info) => info,
            None => {
                self.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!("{} is undefined", name),
                    token: call.lval.token.clone(),
                });
                return;
            }
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
            if arg_t != &param.ty {
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
                    let mut token = lval.token.clone();
                    token.col += parts[0].len() + 1;
                    let sub_lval = Lvalue {
                        name: parts[1..].join("."),
                        token,
                    };
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
                    let mut token = lval.token.clone();
                    token.col += parts[0].len() + 1;
                    let sub_lval = Lvalue {
                        name: parts[1..].join("."),
                        token,
                    };
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
                    let mut token = lval.token.clone();
                    token.col += parts[0].len() + 1;
                    let sub_lval = Lvalue {
                        name: parts[1..].join("."),
                        token,
                    };
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

pub struct ExpressionTypeChecker {
    //ast: &'a mut AST,
    //ast: RefCell::<AST>,
}

impl ExpressionTypeChecker {
    pub fn run(&self) -> (HashMap<Expression, Type>, Diagnostics) {
        // These iterations may seem a bit odd. The reason I'm using numeric
        // indices here is that I cannot borrow a mutable node of the AST and
        // the AST itself at the same time, and then pass them separately into a
        // handler function. So instead I just pass along the mutable AST
        // together with the index of the thing I'm going to mutate within the
        // AST. Then in the handler function, a mutable reference to the node of
        // interest is acquired based on the index.
        /*
        let mut diags = Diagnostics::new();
        for i in 0..self.ast.constants.len() {
            diags.extend(&self.check_constant(i));
        }
        for i in 0..self.ast.controls.len() {
            diags.extend(&self.check_control(i));
        }
        for i in 0..self.ast.parsers.len() {
            diags.extend(&self.check_parser(i));
        }
        diags
        */

        todo!();
    }

    pub fn check_constant(&self, _index: usize) -> Diagnostics {
        todo!("global constant expression type check");
    }

    pub fn check_control(&self, _index: usize) -> Diagnostics {
        /*
        let c = &mut self.ast.controls[index];
        let mut diags = Diagnostics::new();
        let names = c.names();
        for a in &mut c.actions {
            diags.extend(
                &self.check_statement_block(&mut a.statement_block, &names)
            )
        }
        diags
        */
        todo!();
    }

    pub fn check_statement_block(
        &self,
        _sb: &mut StatementBlock,
        _names: &HashMap<String, NameInfo>,
    ) -> Diagnostics {
        todo!();

        /*
        let mut diags = Diagnostics::new();
        for stmt in &mut sb.statements {
            match stmt {
                Statement::Empty => {}
                Statement::Assignment(_, xpr) => {
                    diags.extend(&self.check_expression(xpr, names));
                    todo!()
                }
                Statement::Call(c) => { todo!() }
                Statement::If(ifb) => { todo!() }
                Statement::Variable(v) => { todo!() }
                Statement::Constant(c) => { todo!() }
            }
        }
        diags
        */
    }

    pub fn check_expression(
        &self,
        _xpr: &mut Expression,
        _names: &HashMap<String, NameInfo>,
    ) -> Diagnostics {
        /*
        let mut diags = Diagnostics::new();
        match &mut xpr.kind {
            ExpressionKind::BoolLit(_) => {
                xpr.ty = Some(Type::Bool)
            }
            //TODO P4 spec section 8.9.1/8.9.2
            ExpressionKind::IntegerLit(_) => {
                xpr.ty = Some(Type::Int(128))
            }
            ExpressionKind::BitLit(width, _) => {
                xpr.ty = Some(Type::Bit(*width as usize))
            }
            ExpressionKind::SignedLit(width, _) => {
                xpr.ty = Some(Type::Int(*width as usize))
            }
            ExpressionKind::Lvalue(lval) => {
                //let ty = lvalue_type(lval, ast, names)
                todo!();
            }
            ExpressionKind::Binary(lhs, _, rhs) => {
                todo!();
            }
            ExpressionKind::Index(lval, xpr) => {
                todo!();
            }
            ExpressionKind::Slice(begin, end) => {
                todo!();
            }

        }
        diags
        */
        todo!();
    }

    pub fn check_parser(&self, _index: usize) -> Diagnostics {
        todo!("parser expression type check");
    }
}
