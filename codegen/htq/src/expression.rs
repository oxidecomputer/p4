use std::collections::HashMap;

use htq::ast::{
    Fget, Load, Lookup, Or, Register, Rset, Shl, Statement, Type, Value,
};
use p4::{
    ast::{
        type_size, DeclarationInfo, Expression, ExpressionKind, Lvalue,
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
    let r = VersionedRegister::tmp_for_token(&expr.token);
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
        xpr => todo!("expression: {xpr:?}"),
    }
}

fn emit_call(
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
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!("control apply call");
}

fn emit_set_valid_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
    _valid: bool,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!("set valid call")
}

fn emit_is_valid_call(
    _call: &p4::ast::Call,
    _hlir: &Hlir,
    _ast: &p4::ast::AST,
    _ra: &mut RegisterAllocator,
    _names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    todo!("is valid call")
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

            let src_root = lval.root();
            let source = ra
                .get(src_root)
                .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;

            result.push(Statement::Fget(Fget {
                target: treg.to_reg(),
                typ: typ.clone(),
                source,
                offsets,
            }));
            Ok((result, Some(ExpressionValue::new(treg.to_reg(), typ))))
        }
        DeclarationInfo::Local => {
            let reg = ra
                .get(&lval.name)
                .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;
            Ok((result, Some(ExpressionValue::new(reg, typ))))
        }
        other => todo!("emit lval for \n{other:#?}"),
    }
}

pub(crate) fn emit_bool_lit(
    value: bool,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let instrs = vec![Statement::Rset(Rset {
        target: ra.clone().to_reg(),
        typ: Type::Bool,
        source: Value::bool(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra.to_reg(), Type::Bool))))
}

pub(crate) fn emit_bit_lit(
    width: u16,
    value: u128,
    ra: VersionedRegister,
    expr: &Expression,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let value = i128::try_from(value)
        .map_err(|_| CodegenError::NumericConversion(expr.clone()))?;
    let typ = Type::Bitfield(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra.to_reg(), typ))))
}

pub(crate) fn emit_int_lit(
    value: i128,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let typ = Type::Signed(128);
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra.to_reg(), typ))))
}

pub(crate) fn emit_signed_lit(
    width: u16,
    value: i128,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Option<ExpressionValue>), CodegenError> {
    let typ = Type::Signed(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, Some(ExpressionValue::new(ra.to_reg(), typ))))
}
