use crate::ast::{AST, Parser};

#[derive(Debug)]
pub struct Diagnostic {
    /// Level of this diagnostic.
    pub level: Level,

    /// Message associated with this diagnostic.
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub enum Level {
    Info,
    Deprecation,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.0.iter().filter(|x| x.level == Level::Error).collect()
    }
}

pub fn all(ast: &AST) -> Diagnostics {
    let mut diags = Vec::new();
    for parser in &ast.parsers {
        diags.extend(ParserChecker::check(parser).0);
    }
    Diagnostics(diags)
}

pub struct ParserChecker {}

impl ParserChecker {
    pub fn check(p: &Parser) -> Diagnostics {
        let mut diags = Vec::new();

        Self::start_state(p, &mut diags);

        Diagnostics(diags)
    }

    pub fn start_state(parser: &Parser, diags: &mut Vec<Diagnostic>) {

        for s in &parser.states {
            if s.name == "start" {
                return;
            }
        }

        diags.push(Diagnostic{
            level: Level::Error,
            message: format!(
                "start state not found for parser {}", parser.name),
        });

    }
}
