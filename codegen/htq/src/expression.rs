use std::collections::HashMap;

use htq::ast::{
    Fget, Load, Lookup, Or, Register, Rset, Shl, Statement, Type, Value,
};
use p4::{
    ast::{
        type_size, BinOp, DeclarationInfo, Expression, ExpressionKind, Lvalue,
        NameInfo,
    },
    hlir::Hlir,
};

// Copyright 2024 Oxide Computer Company
use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::member_offsets,
    AsyncFlagAllocator, P4Context, RegisterAllocator, VersionedRegister,
};

pub(crate) struct ExpressionValue {
    // register the value of the expression is held in along with their types
    pub(crate) registers: Vec<(Register, Type)>,
    // sync flag associated with the expression
    #[allow(dead_code)]
    pub(crate) sync_flag: Option<u128>,
}

impl ExpressionValue {
    fn new(register: Register, typ: Type) -> Self {
        Self {
            registers: vec![(register, typ)],
            sync_flag: None,
        }
    }

    #[allow(dead_code)]
    fn new_async(register: Register, typ: Type, sync_flag: u128) -> Self {
        Self {
            registers: vec![(register, typ)],
            sync_flag: Some(sync_flag),
        }
    }
}

// Builds a vector of statements that implement the expression. Returns the
// statements and the resulting value of the expression, if any.
pub(crate) fn emit_expression(
    expr: &Expression,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let r = ra.alloc_tmp_for_token(&expr.token);
    match &expr.kind {
        ExpressionKind::BoolLit(value) => emit_bool_lit(*value, r),
        ExpressionKind::BitLit(width, value) => {
            emit_bit_lit(*width, *value, r, expr)
        }
        ExpressionKind::IntegerLit(value) => emit_int_lit(*value, r),
        ExpressionKind::SignedLit(width, value) => {
            emit_signed_lit(*width, *value, r)
        }
        ExpressionKind::Lvalue(lval) => emit_lval(lval, hlir, ast, ra, names),
        ExpressionKind::Call(call) => match context {
            P4Context::Control(c) => {
                emit_call(call, c, hlir, ast, ra, afa, names)
            }
            P4Context::Parser(_) => {
                Err(CodegenError::CallInParser(expr.clone()))
            }
        },
        ExpressionKind::Binary(lhs, op, rhs) => emit_binary_expr(
            lhs.as_ref(),
            op,
            rhs.as_ref(),
            hlir,
            ast,
            ra,
            afa,
            names,
        ),
        xpr => todo!("expression: {xpr:?}"),
    }
}

pub(crate) fn emit_single_valued_expression(
    expr: &Expression,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    context: &P4Context<'_>,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Register), CodegenError> {
    let (stmts, val) =
        emit_expression(expr, hlir, ast, context, ra, afa, names)?;

    let val = val.ok_or(CodegenError::ExpressionValueNeeded(expr.clone()))?;
    if val.registers.len() != 1 {
        return Err(CodegenError::SingularExpressionValueNeeded(expr.clone()));
    }

    Ok((stmts, val.registers[0].0.clone()))
}

pub(crate) fn emit_binary_expr(
    lhs: &Expression,
    op: &BinOp,
    rhs: &Expression,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    match op {
        BinOp::Add => todo!("bin op add"),
        BinOp::Subtract => todo!("bin op subtract"),
        BinOp::Mod => todo!("bin op mod"),
        BinOp::Geq => todo!("bin op geq"),
        BinOp::Gt => todo!("bin op gt"),
        BinOp::Leq => todo!("bin op leq"),
        BinOp::Lt => todo!("bin op lt"),
        BinOp::Eq => emit_binary_expr_eq(lhs, rhs, hlir, ast, ra, afa, names),
        BinOp::Mask => todo!("bin op mask"),
        BinOp::NotEq => todo!("bin op not eq"),
        BinOp::BitAnd => todo!("bin op bit and"),
        BinOp::BitOr => todo!("bin op bit or"),
        BinOp::Xor => todo!("bin op xor"),
    }
}

pub(crate) fn emit_binary_expr_eq(
    _lhs: &Expression,
    _rhs: &Expression,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _afa: &mut AsyncFlagAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!()
}

pub(crate) fn emit_call(
    call: &p4::ast::Call,
    control: &p4::ast::Control,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    match call.lval.leaf() {
        "apply" => emit_apply_call(call, control, hlir, ast, ra, afa, names),
        "setValid" => emit_set_valid_call(call, hlir, ast, ra, names, true),
        "setInvalid" => emit_set_valid_call(call, hlir, ast, ra, names, false),
        "isValid" => emit_is_valid_call(call, hlir, ast, ra, names),
        _ => emit_extern_call(call, hlir, ast, ra, names),
    }
}

fn emit_apply_call(
    call: &p4::ast::Call,
    control: &p4::ast::Control,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let parent = call.lval.pop_right();
    let parent = parent.leaf();
    let info = names
        .get(parent)
        .ok_or(CodegenError::CallParentNotFound(call.clone()))?;

    match &info.ty {
        p4::ast::Type::Table => {
            emit_table_apply_call(call, control, hlir, ast, ra, afa, names)
        }
        p4::ast::Type::UserDefined(name, _) => {
            // validate user defined type is a control
            ast.get_control(name)
                .ok_or(CodegenError::InvalidCallParent(
                    call.clone(),
                    info.ty.clone(),
                ))?;
            emit_control_apply_call(call, hlir, ast, ra, names)
        }
        p4::ast::Type::Action => emit_action_call(call, hlir, ast, ra, names),
        p4::ast::Type::ExternFunction => {
            emit_extern_call(call, hlir, ast, ra, names)
        }
        typ => Err(CodegenError::InvalidCallParent(call.clone(), typ.clone())),
    }
}

fn emit_table_apply_call(
    call: &p4::ast::Call,
    control: &p4::ast::Control,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    afa: &mut AsyncFlagAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    // %hit, %variant, %args = async 2 lookup proxy_arp %key;
    let table = call.lval.pop_right();
    let table = table.leaf();
    let table = control.get_table(table).ok_or(CodegenError::TableNotFound(
        table.to_owned(),
        control.clone(),
    ))?;

    let sync_flag = afa.allocate()?;

    let hit = VersionedRegister::for_token("hit", &call.lval.token);
    let variant = VersionedRegister::for_token("variant", &call.lval.token);
    let args = VersionedRegister::for_token("args", &call.lval.token);
    let sync_flag_reg =
        VersionedRegister::for_token("sync_flag", &call.lval.token);
    let mut key = VersionedRegister::for_token("key", &call.lval.token);
    let mut instrs = Vec::new();

    let mut total_key_size = 0;
    for (k, _) in &table.key {
        let info = hlir
            .lvalue_decls
            .get(k)
            .ok_or(CodegenError::UndefinedLvalue(k.clone()))?;
        total_key_size += type_size(&info.ty, ast);
    }

    let key_typ = Type::Bitfield(total_key_size);

    instrs.push(Statement::Rset(Rset {
        target: key.to_reg(),
        typ: key_typ.clone(),
        source: Value::number(0),
    }));

    instrs.push(Statement::Rset(Rset {
        target: sync_flag_reg.to_reg(),
        typ: Type::Bitfield(128),
        source: Value::number(sync_flag as i128),
    }));

    let mut offset = 0u128;
    for (k, _) in &table.key {
        let (key_extract_statements, extracted_value) =
            emit_lval(k, hlir, ast, ra, names)?;
        let extracted_value = extracted_value
            .ok_or(CodegenError::KeyExtractionProducedNoValue(k.clone()))?;
        instrs.extend(key_extract_statements.into_iter());

        let tmp = VersionedRegister::for_token("key", &k.token);
        instrs.push(Statement::Shl(Shl {
            target: tmp.to_reg(),
            typ: key_typ.clone(),
            source: Value::register(&extracted_value.registers[0].0 .0),
            amount: Value::number(offset as i128),
        }));

        let curr_key = key.clone();
        instrs.push(Statement::Or(Or {
            target: key.next().to_reg(),
            typ: key_typ.clone(),
            source_a: Value::reg(curr_key.to_reg()),
            source_b: Value::reg(tmp.to_reg()),
        }));
        offset += extracted_value.registers[0].1.bit_size().unwrap() as u128;
    }

    let lookup_instr = Statement::Lookup(Lookup {
        hit: hit.to_reg(),
        variant: variant.to_reg(),
        args: args.to_reg(),
        asynchronous: if table.is_async {
            Some(htq::ast::Async {
                identifier: Value::number(sync_flag as i128),
            })
        } else {
            None
        },
        table: table.name.clone(),
        key: Value::register(&key.name()),
    });
    instrs.push(lookup_instr);

    let args_size = control.maximum_action_arg_length_for_table(ast, table);

    Ok((
        instrs,
        Some(ExpressionValue {
            registers: vec![
                (hit.to_reg(), Type::Bool),
                (variant.to_reg(), Type::Unsigned(16)),
                (args.to_reg(), Type::Unsigned(args_size)),
                (sync_flag_reg.to_reg(), Type::Bitfield(128)),
            ],
            sync_flag: Some(sync_flag),
        }),
    ))
}

fn emit_control_apply_call(
    call: &p4::ast::Call,
    _hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    // get the control being called
    let info = names
        .get(call.lval.root())
        .ok_or(CodegenError::CallParentNotFound(call.clone()))?;

    let control_type_name = match &info.ty {
        p4::ast::Type::UserDefined(name, _) => name.to_owned(),
        _x => {
            return Err(CodegenError::ExpectedControl(
                call.lval.clone(),
                info.ty.clone(),
            ))
        }
    };

    let control = ast
        .get_control(&control_type_name)
        .ok_or(CodegenError::CallParentNotFound(call.clone()))?;

    // determine argument registers
    let mut arg_values = Vec::default();
    for a in &call.args {
        match &a.kind {
            ExpressionKind::Lvalue(lval) => {
                let info = names
                    .get(lval.root())
                    .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;
                if info.ty.is_lookup_result() {
                    continue;
                }
                let reg = ra.get(&lval.name).ok_or(
                    CodegenError::RegisterDoesNotExistForLval(lval.clone()),
                )?;
                arg_values.push(Value::Register(reg));
            }
            ExpressionKind::BitLit(_width, value) => {
                arg_values.push(Value::number(*value as i128));
            }
            ExpressionKind::SignedLit(_width, value) => {
                arg_values.push(Value::number(*value));
            }
            ExpressionKind::IntegerLit(value) => {
                arg_values.push(Value::number(*value));
            }
            ExpressionKind::BoolLit(value) => {
                arg_values.push(Value::number(*value as i128));
            }
            _ => todo!("call argument type {:#?}", a.kind),
        };
    }

    // determine return registers
    let mut returned_registers = Vec::default();
    for (i, p) in control.parameters.iter().enumerate() {
        if p.direction.is_out() {
            if p.ty.is_lookup_result() {
                let arg = match &call.args[i].kind {
                    ExpressionKind::Lvalue(lval) => lval.root().to_owned(),
                    _x => panic!("expected lvalue for out parameter"),
                };
                let hit = ra.alloc(&format!("{}_hit", arg));
                returned_registers.push((hit, htq::ast::Type::Bool));

                let variant = ra.alloc(&format!("{}_variant", arg));
                returned_registers
                    .push((variant, htq::ast::Type::Unsigned(16)));

                let args = ra.alloc(&format!("{}_args", arg));
                let args_size = control
                    .resolve_lookup_result_args_size(&p.name, ast)
                    .ok_or(CodegenError::LookupResultArgSize(p.clone()))?;
                returned_registers
                    .push((args, htq::ast::Type::Bitfield(args_size)));

                let sync = ra.alloc(&format!("{}_sync", arg));
                returned_registers.push((sync, htq::ast::Type::Bitfield(128)));
            } else {
                returned_registers
                    .push((ra.alloc(&p.name), p4_type_to_htq_type(&p.ty)?))
            }
        }
    }

    let call_stmt = Statement::Call(htq::ast::Call {
        fname: format!("{}_{}", call.lval.root(), call.lval.leaf()),
        args: arg_values,
        targets: returned_registers.iter().map(|x| x.0.clone()).collect(),
    });

    Ok((
        vec![call_stmt],
        Some(ExpressionValue {
            registers: returned_registers,
            sync_flag: None,
        }),
    ))
}

fn emit_set_valid_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
    _valid: bool,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    //TODO
    Ok((Vec::default(), None))
    //todo!("set valid call")
}

fn emit_is_valid_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    //TODO
    Ok((Vec::default(), None))
}

fn emit_extern_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!("extern call")
}

fn emit_action_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!("action call")
}

fn emit_lval(
    lval: &Lvalue,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let mut result: Vec<Statement> = Vec::default();

    let info = hlir
        .lvalue_decls
        .get(lval)
        .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;

    let typ = p4_type_to_htq_type(&info.ty)?;

    match &info.decl {
        DeclarationInfo::Parameter(_) => {
            //TODO unify VersionedRegister and RegisterAllocator
            let treg = VersionedRegister::tmp_for_token(&lval.token);
            result.push(Statement::Load(Load {
                target: treg.to_reg(),
                typ: typ.clone(),
                source: Register::new(lval.root()),
                offset: Value::number(0),
            }));
            Ok((result, Some(ExpressionValue::new(treg.to_reg(), typ))))
        }
        DeclarationInfo::ActionParameter(_) => {
            let treg = VersionedRegister::tmp_for_token(&lval.token);
            result.push(Statement::Load(Load {
                target: treg.to_reg(),
                typ: typ.clone(),
                source: Register::new(lval.root()),
                offset: Value::number(0),
            }));
            Ok((result, Some(ExpressionValue::new(treg.to_reg(), typ))))
        }
        DeclarationInfo::StructMember | DeclarationInfo::HeaderMember => {
            let offsets = member_offsets(ast, names, lval)?;
            let treg = VersionedRegister::tmp_for_token(&lval.token);

            // TODO this is terrible, we should have one way to lookup the
            // register
            let name = lval.root();
            let source = if let Ok(source) =
                ra.get(name).ok_or(CodegenError::MissingRegisterForLvalue(
                    lval.clone(),
                    ra.all_registers(),
                )) {
                source
            } else {
                let name = reg_name_for_lval(lval);
                ra.get(&name).ok_or(CodegenError::MissingRegisterForLvalue(
                    lval.clone(),
                    ra.all_registers(),
                ))?
            };

            result.push(Statement::Fget(Fget {
                target: treg.to_reg(),
                typ: typ.clone(),
                source,
                offsets,
            }));
            Ok((result, Some(ExpressionValue::new(treg.to_reg(), typ))))
        }
        DeclarationInfo::Local => {
            let name = reg_name_for_lval(lval);
            let reg =
                ra.get(&name).ok_or(CodegenError::MissingRegisterForLvalue(
                    lval.clone(),
                    ra.all_registers(),
                ))?;
            Ok((result, Some(ExpressionValue::new(reg, typ))))
        }
        other => todo!("emit lval for \n{other:#?}"),
    }
}

fn reg_name_for_lval(lval: &Lvalue) -> String {
    lval.name.replace('.', "_")
}

pub(crate) fn emit_bool_lit(
    value: bool,
    ra: Register,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let instrs = vec![Statement::Rset(Rset {
        target: ra.clone(),
        typ: Type::Bool,
        source: Value::bool(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra, Type::Bool))))
}

pub(crate) fn emit_bit_lit(
    width: u16,
    value: u128,
    ra: Register,
    expr: &Expression,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let value = i128::try_from(value)
        .map_err(|_| CodegenError::NumericConversion(expr.clone()))?;
    let typ = Type::Bitfield(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra, typ))))
}

pub(crate) fn emit_int_lit(
    value: i128,
    ra: Register,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let typ = Type::Signed(128);
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra, typ))))
}

pub(crate) fn emit_signed_lit(
    width: u16,
    value: i128,
    ra: Register,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let typ = Type::Signed(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra, typ))))
}
