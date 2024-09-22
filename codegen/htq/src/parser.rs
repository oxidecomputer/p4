// Copyright 2024 Oxide Computer Company

use std::collections::HashMap;

use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::emit_statement,
    AsyncFlagAllocator, P4Context, RegisterAllocator,
};
use htq::ast::Register;
use p4::{ast::ControlParameter, hlir::Hlir};

pub(crate) fn emit_parser_functions(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    afa: &mut AsyncFlagAllocator,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::new();

    for parser in &ast.parsers {
        let pf = emit_parser(ast, hlir, parser, afa)?;
        result.extend(pf.into_iter());
    }

    Ok(result)
}

fn emit_parser(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    parser: &p4::ast::Parser,
    afa: &mut AsyncFlagAllocator,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::new();
    let mut parameters = Vec::new();
    let mut psub = HashMap::<ControlParameter, Vec<Register>>::default();
    let mut ra = RegisterAllocator::default();

    let mut return_signature = Vec::new();

    for x in &parser.parameters {
        let typ = p4_type_to_htq_type(&x.ty)?;
        if x.direction.is_out() {
            return_signature.push(typ.clone());
        }
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            typ,
        };
        ra.alloc(&p.reg.0);
        parameters.push(p);
    }

    let mut names = parser.names();

    for state in &parser.states {
        // keeps track of register revisions for locals
        let mut statements = Vec::default();
        let mut blocks = Vec::default();
        for s in &state.statements.statements {
            let (stmts, blks) = emit_statement(
                s,
                ast,
                &P4Context::Parser(parser),
                hlir,
                &mut names,
                &mut ra,
                afa,
                &mut psub,
            )?;
            statements.extend(stmts);
            blocks.extend(blks);
        }
        let f = htq::ast::Function {
            name: format!("{}_{}", parser.name, state.name),
            parameters: parameters.clone(),
            statements,
            blocks,
            return_signature: return_signature.clone(),
        };
        result.push(f);
    }

    Ok(result)
}
