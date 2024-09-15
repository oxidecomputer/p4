// Copyright 2024 Oxide Computer Company

use htq::ast::Parameter;
use p4::hlir::Hlir;

use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::emit_statement,
    RegisterAllocator,
};

pub(crate) fn emit_control_functions(
    ast: &p4::ast::AST,
    hlir: &Hlir,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::default();

    for control in &ast.controls {
        let cf = emit_control(ast, hlir, control)?;
        result.extend(cf.into_iter());
    }

    Ok(result)
}

fn emit_control(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::default();

    let mut parameters = Vec::new();
    for x in &control.parameters {
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            pointer: true,
            typ: p4_type_to_htq_type(&x.ty)?,
        };
        parameters.push(p);
    }

    result.push(emit_control_apply(ast, hlir, control, &parameters)?);

    for action in &control.actions {
        result.push(emit_control_action(
            ast,
            hlir,
            control,
            action,
            &parameters,
        )?);
    }
    Ok(result)
}

fn emit_control_apply(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
    parameters: &[Parameter],
) -> Result<htq::ast::Function, CodegenError> {
    let mut ra = RegisterAllocator::default();
    let mut names = control.names();
    let mut statements = Vec::default();

    for p in parameters {
        ra.alloc(&p.reg.0);
    }

    for s in &control.apply.statements {
        statements.extend(
            emit_statement(s, ast, hlir, &mut names, &mut ra)?.into_iter(),
        )
    }
    let f = htq::ast::Function {
        name: format!("{}_apply", control.name),
        parameters: parameters.to_owned(),
        statements,
    };
    Ok(f)
}

fn emit_control_action(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
    action: &p4::ast::Action,
    parameters: &[Parameter],
) -> Result<htq::ast::Function, CodegenError> {
    let mut ra = RegisterAllocator::default();
    let mut names = control.names();
    let mut statements = Vec::default();
    let mut parameters = parameters.to_owned();
    for x in &action.parameters {
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            pointer: true,
            typ: p4_type_to_htq_type(&x.ty)?,
        };
        parameters.push(p);
    }
    for p in &parameters {
        ra.alloc(&p.reg.0);
    }
    for s in &action.statement_block.statements {
        statements.extend(
            emit_statement(s, ast, hlir, &mut names, &mut ra)?.into_iter(),
        )
    }
    let f = htq::ast::Function {
        name: format!("{}_{}", control.name, action.name),
        parameters,
        statements,
    };
    Ok(f)
}
