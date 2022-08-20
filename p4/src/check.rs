use std::collections::HashMap;

use crate::ast::{
    Control, DeclarationInfo, Expression, ExpressionKind, Header, Lvalue,
    NameInfo, Parser, State, Statement, StatementBlock, Struct, Table, Type,
    AST,
};
use crate::hlir::{Hlir, HlirGenerator};
use crate::lexer::Token;

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
    for p in &ast.parsers {
        diags.extend(&ParserChecker::check(p, ast));
    }
    for c in &ast.controls {
        diags.extend(&ControlChecker::check(c, ast));
    }
    for s in &ast.structs {
        diags.extend(&StructChecker::check(s, ast));
    }
    for h in &ast.headers {
        diags.extend(&HeaderChecker::check(h, ast));
    }
    let mut hg = HlirGenerator::new(ast);
    hg.run();
    diags.extend(&hg.diags);
    (hg.hlir, diags)
}

pub struct ControlChecker {}

impl ControlChecker {
    pub fn check(c: &Control, ast: &AST) -> Diagnostics {
        let mut diags = Diagnostics::new();
        let names = c.names();
        Self::check_params(c, ast, &mut diags);
        Self::check_tables(c, &names, ast, &mut diags);
        Self::check_variables(c, ast, &mut diags);
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
                parser.name
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
        let last = &stmts[stmts.len() - 1];
        if !matches!(last, Statement::Transition(_)) {
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
                        message: format!("Typename {} not found", typename),
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
                        message: format!("Typename {} not found", typename),
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
                    Some(p) => format!("{} does not have member {}", p, name),
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
            match &v.initializer {
                Some(expr) => {
                    diags.extend(&check_expression_lvalues(
                        expr.as_ref(),
                        ast,
                        names,
                    ));
                }
                None => {}
            };
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
        Statement::Transition(_transition) => {
            //TODO
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
                        "type bool does not have a member {}",
                        parts[1]
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
                        "type error does not have a member {}",
                        parts[1]
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
                        "type bit<{}> does not have a member {}",
                        size, parts[1]
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
                        "type varbit<{}> does not have a member {}",
                        size, parts[1]
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
                        size, parts[1]
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
                        "type string does not have a member {}",
                        parts[1]
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
        Type::Table => {
            if parts.len() > 1 {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type table does not have a member {}",
                        parts[1]
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
                        "type void does not have a member {}",
                        parts[1]
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
                        "type list does not have a member {}",
                        parts[1]
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
            } else {
                diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!("type {} is not defined", name),
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
