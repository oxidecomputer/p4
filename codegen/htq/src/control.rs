// Copyright 2024 Oxide Computer Company

use std::collections::HashMap;

use htq::ast::{Parameter, Register};
use p4::{ast::ControlParameter, hlir::Hlir};

use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::emit_statement,
    AsyncFlagAllocator, P4Context, RegisterAllocator,
};

pub(crate) fn emit_control_functions(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    afa: &mut AsyncFlagAllocator,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::default();

    for control in &ast.controls {
        let cf = emit_control(ast, hlir, control, afa)?;
        result.extend(cf.into_iter());
    }

    Ok(result)
}

fn emit_control(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
    afa: &mut AsyncFlagAllocator,
) -> Result<Vec<htq::ast::Function>, CodegenError> {
    let mut result = Vec::default();
    let mut psub = HashMap::<ControlParameter, Vec<Register>>::default();

    let mut parameters = Vec::new();
    let mut apply_return_signature = Vec::new();
    let mut action_return_signature = Vec::new();
    for x in &control.parameters {
        let typ = p4_type_to_htq_type(&x.ty)?;
        if x.direction.is_out() {
            // TODO the special case nature of lookup results is quite
            // unfortunate
            if x.ty.is_lookup_result() {
                apply_return_signature.push(htq::ast::Type::Bool); //hit
                apply_return_signature.push(htq::ast::Type::Unsigned(16)); //variant

                let args_size = control
                    .resolve_lookup_result_args_size(&x.name, ast)
                    .ok_or(CodegenError::LookupResultArgSize(x.clone()))?;

                apply_return_signature
                    .push(htq::ast::Type::Unsigned(args_size));

                if x.ty.is_sync() {
                    apply_return_signature.push(htq::ast::Type::Unsigned(128)); //async flag
                }
            } else {
                apply_return_signature.push(typ.clone());
                action_return_signature.push(typ.clone());
            }
        }
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            typ,
        };
        parameters.push((p, x.clone()));
    }

    result.push(emit_control_apply(
        ast,
        hlir,
        control,
        &parameters,
        &apply_return_signature,
        afa,
        &mut psub,
    )?);

    for action in &control.actions {
        result.push(emit_control_action(
            ast,
            hlir,
            control,
            action,
            &parameters,
            &action_return_signature,
            afa,
            &mut psub,
        )?);
    }
    Ok(result)
}

fn emit_control_apply(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
    parameters: &[(Parameter, p4::ast::ControlParameter)],
    return_signature: &[htq::ast::Type],
    afa: &mut AsyncFlagAllocator,
    psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<htq::ast::Function, CodegenError> {
    let mut ra = RegisterAllocator::default();
    let mut names = control.names();
    let mut statements = Vec::default();

    for (p, p4p) in parameters {
        if psub.get(p4p).is_some() {
            continue;
        }
        ra.alloc(&p.reg.0);
    }

    for s in &control.apply.statements {
        statements.extend(
            emit_statement(
                s,
                ast,
                P4Context::Control(control),
                hlir,
                &mut names,
                &mut ra,
                afa,
                psub,
            )?
            .into_iter(),
        )
    }

    let mut signature = Vec::new();
    let mut return_registers = Vec::new();

    for (p, p4p) in parameters {
        if p4p.direction.is_out() {
            if let Some(substituted) = psub.get(p4p) {
                return_registers.extend(
                    substituted
                        .clone()
                        .into_iter()
                        .map(|x| ra.get_reg(&x).unwrap()),
                );
            } else {
                signature.push(p.clone());
                return_registers.push(ra.get_reg(&p.reg).unwrap());
            }
        }
    }

    statements.push(htq::ast::Statement::Return(htq::ast::Return {
        registers: return_registers,
    }));

    let f = htq::ast::Function {
        name: format!("{}_apply", control.name),
        parameters: signature,
        statements,
        return_signature: return_signature.to_vec(),
    };
    Ok(f)
}

fn emit_control_action(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    control: &p4::ast::Control,
    action: &p4::ast::Action,
    parameters: &[(Parameter, p4::ast::ControlParameter)],
    return_signature: &[htq::ast::Type],
    afa: &mut AsyncFlagAllocator,
    psub: &mut HashMap<ControlParameter, Vec<Register>>,
) -> Result<htq::ast::Function, CodegenError> {
    let mut ra = RegisterAllocator::default();
    let mut names = control.names();
    let mut statements = Vec::default();
    let parameters: Vec<(Parameter, p4::ast::ControlParameter)> = parameters
        .iter()
        .filter(|(_, p4p)| !p4p.ty.is_lookup_result())
        .cloned()
        .collect();
    let mut action_parameters: Vec<Parameter> =
        parameters.iter().cloned().map(|x| x.0).collect();

    for x in &action.parameters {
        let p = htq::ast::Parameter {
            reg: htq::ast::Register::new(x.name.as_str()),
            typ: p4_type_to_htq_type(&x.ty)?,
        };
        action_parameters.push(p);
    }

    for (p, p4p) in &parameters {
        if psub.get(p4p).is_some() {
            continue;
        }
        ra.alloc(&p.reg.0);
    }

    for s in &action.statement_block.statements {
        statements.extend(
            emit_statement(
                s,
                ast,
                P4Context::Control(control),
                hlir,
                &mut names,
                &mut ra,
                afa,
                psub,
            )?
            .into_iter(),
        )
    }

    let mut signature = Vec::new();
    let mut return_registers = Vec::new();

    for (p, p4p) in &parameters {
        if p4p.direction.is_out() {
            if let Some(substituted) = psub.get(p4p) {
                for x in substituted {
                    if let Some(r) = ra.get_reg(x) {
                        return_registers.push(r.clone())
                    }
                }
                /*
                return_registers.extend(
                    substituted
                        .clone()
                        .into_iter()
                        .map(|x| ra.get_reg(&x).unwrap()),
                );
                */
            } else {
                signature.push(p.clone());
                return_registers.push(ra.get_reg(&p.reg).unwrap());
            }
        }
    }
    statements.push(htq::ast::Statement::Return(htq::ast::Return {
        registers: return_registers,
    }));

    let f = htq::ast::Function {
        name: format!("{}_{}", control.name, action.name),
        parameters: action_parameters,
        statements,
        return_signature: return_signature.to_vec(),
    };
    Ok(f)
}
