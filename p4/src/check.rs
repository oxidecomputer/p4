use std::collections::HashMap;

use crate::lexer::Token;
use crate::ast::{
    AST, Expression, Lvalue, Parser, Statement, StatementBlock, Type
};

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

#[derive(Debug, PartialEq, Clone)]
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

pub fn all(ast: &AST) -> Diagnostics {
    let mut diags = Vec::new();
    for parser in &ast.parsers {
        diags.extend(ParserChecker::check(parser, ast).0);
    }
    Diagnostics(diags)
}

pub struct ParserChecker {}

impl ParserChecker {
    pub fn check(p: &Parser, ast: &AST) -> Diagnostics {
        let mut diags = Diagnostics::new();

        if !p.decl_only {
            Self::start_state(p, &mut diags);
            Self::lvalues(p, ast, &mut diags);
        }

        diags
    }

    /// Provide a mapping of names within the context of this parser to their
    /// types.
    pub fn name_context(p: &Parser) -> HashMap::<String, Type> {
        let mut ctx = HashMap::new();
        for x in &p.parameters {
            ctx.insert(x.name.clone(), x.ty.clone());
        }
        ctx
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

    /// Check lvalue references
    pub fn lvalues(parser: &Parser, ast: &AST, diags: &mut Diagnostics) {
        let names = Self::name_context(parser);
        for state in &parser.states {
            let mut state_names = names.clone();
            for v in &state.variables {
                state_names.insert(v.name.clone(), v.ty.clone());
            }
            for stmt in &state.statements {
                diags.extend(&check_statement_lvalues(stmt, ast, &names));
            }
        }
    }
}

fn check_name(
    name: &str,
    names: &HashMap::<String, Type>,
    token: &Token,
    parent: Option<&str>,
) -> (Diagnostics, Option<Type>) {

    let ty = names.get(name);
    match ty {
        Some(ty) => (Diagnostics::new(), Some(ty.clone())),
        None => (
            Diagnostics(vec![Diagnostic {
                level: Level::Error,
                message: match parent {
                    Some(p) => format!("{} does not have member {}", p, name),
                    None => format!("'{}' is undefined", name),
                },
                token: token.clone(),
            }]),
            None
        )
    }

}

fn check_statement_lvalues(
    stmt: &Statement,
    ast: &AST,
    names: &HashMap::<String, Type>
) -> Diagnostics {
    let mut diags = Diagnostics::new();
    match stmt {
        Statement::Empty => {}, 
        Statement::Assignment(lval, expr) => {
            diags.extend(&check_lvalue(lval, ast, &names, None));
            diags.extend(
                &check_expression_lvalues(expr, ast, &names)
            );
        }
        Statement::Call(call) => {
            diags.extend(&check_lvalue(&call.lval, ast, &names, None));
            for arg in &call.args {
                diags.extend(
                    &check_expression_lvalues(
                        arg.as_ref(),
                        ast,
                        &names,
                    )
                );
            }
        }
        Statement::If(if_block) => {
            diags.extend(&check_expression_lvalues(
                if_block.predicate.as_ref(),
                ast,
                &names,
            ));
            diags.extend(&check_statement_block_lvalues(
                &if_block.block,
                ast,
                &names,
            ));
            for elif in &if_block.else_ifs {
                diags.extend(&check_expression_lvalues(
                    elif.predicate.as_ref(),
                    ast,
                    &names,
                ));
                diags.extend(&check_statement_block_lvalues(
                    &elif.block,
                    ast,
                    &names,
                ));
            }
            if let Some(ref else_block) = if_block.else_block {
                diags.extend(&check_statement_block_lvalues(
                    &else_block,
                    ast,
                    &names,
                ));
            }
        }
    }
    diags
}

fn check_statement_block_lvalues(
    block: &StatementBlock,
    ast: &AST,
    names: &HashMap::<String, Type>
) -> Diagnostics {
    let mut diags = Diagnostics::new();
    let mut block_names = names.clone();
    for var in &block.variables {
        block_names.insert(var.name.clone(), var.ty.clone());
    }
    for stmt in &block.statements {
        diags.extend(&check_statement_lvalues(
            stmt,
            ast,
            &block_names
        ));
    }
    diags
}

fn check_expression_lvalues(
    xpr: &Expression,
    ast: &AST,
    names: &HashMap::<String, Type>,
) -> Diagnostics {

    match xpr {
        Expression::Lvalue(lval) => check_lvalue(lval, ast, names, None),
        Expression::Binary(lhs, _, rhs) => {
            let mut diags = Diagnostics::new();
            diags.extend(&check_expression_lvalues(lhs.as_ref(), ast, names));
            diags.extend(&check_expression_lvalues(rhs.as_ref(), ast, names));
            diags
        }
        Expression::Index(lval, xpr) => {
            let mut diags = Diagnostics::new();
            diags.extend(&check_lvalue(lval, ast, names, None));
            diags.extend(&check_expression_lvalues(xpr.as_ref(), ast, names));
            diags
        }
        Expression::Slice(lhs, rhs) => {
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
    names: &HashMap::<String, Type>,
    parent: Option<&str>,
) -> Diagnostics {

    let parts = lval.parts();

    let ty = match check_name(parts[0], names, &lval.token, parent) {
        (_, Some(ty)) => ty,
        (diags, None) => {
            return diags
        }
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
                    message: format!(
                        "extern functions do not have members",
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
                    let sub_lval = Lvalue{
                        name: parts[1..].join("."),
                        token: token,
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
                    let sub_lval = Lvalue{
                        name: parts[1..].join("."),
                        token: token,
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
                    let sub_lval = Lvalue{
                        name: parts[1..].join("."),
                        token: token,
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
