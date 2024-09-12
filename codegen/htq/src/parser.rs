// Copyright 2024 Oxide Computer Company

use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::emit_statement,
};
use p4::hlir::Hlir;

pub(crate) fn emit_parser_functions(
    ast: &p4::ast::AST,
    hlir: &Hlir,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::new();

    for parser in &ast.parsers {
        let pf = emit_parser(ast, hlir, parser)?;
        result.extend(pf.into_iter());
    }

    Ok(result)
}

pub(crate) fn emit_parser(
    _ast: &p4::ast::AST,
    _hlir: &Hlir,
    parser: &p4::ast::Parser,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::new();
    let mut parameters = Vec::new();

    for x in &parser.parameters {
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            pointer: true,
            typ: p4_type_to_htq_type(&x.ty)?,
        };
        parameters.push(p);
    }

    for state in &parser.states {
        let mut statements = Vec::new();
        for s in &state.statements.statements {
            statements.extend(emit_statement(s)?.into_iter());
        }
        let f = htq::ast::Function {
            name: format!("{}_{}", parser.name, state.name),
            parameters: parameters.clone(),
            statements,
        };
        result.push(f);
    }

    Ok(result)
}
