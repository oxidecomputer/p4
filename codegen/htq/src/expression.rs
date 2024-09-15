use std::collections::HashMap;

use htq::ast::{Fget, Load, Register, Rset, Statement, Type, Value};
use p4::{
    ast::{DeclarationInfo, Expression, ExpressionKind, Lvalue, NameInfo},
    hlir::Hlir,
};

// Copyright 2024 Oxide Computer Company
use crate::{
    error::CodegenError, p4_type_to_htq_type, statement::member_offsets,
    RegisterAllocator, VersionedRegister,
};

// Builds a vector of statements that implement the expression. Returns the
// statements and the register the result of the expression is held in.
pub(crate) fn emit_expression(
    expr: &Expression,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let r = VersionedRegister::for_token(&expr.token);
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
        ExpressionKind::Call(_) => {
            //TODO
            Ok((
                Vec::default(),
                Register::new("badreg"),
                Type::User("badtype".to_owned()),
            ))
        }
        xpr => todo!("expression: {xpr:?}"),
    }
}

fn emit_lval(
    lval: &Lvalue,
    hlir: &Hlir,
    ast: &p4::ast::AST,
    ra: &mut RegisterAllocator,
    names: &HashMap<String, NameInfo>,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let mut result: Vec<Statement> = Vec::default();

    let info = hlir
        .lvalue_decls
        .get(lval)
        .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;

    let typ = p4_type_to_htq_type(&info.ty)?;

    match &info.decl {
        DeclarationInfo::Parameter(_) => {
            let treg = VersionedRegister::for_token(&lval.token);
            result.push(Statement::Load(Load {
                target: treg.to_reg(),
                typ: typ.clone(),
                source: Register::new(lval.root()),
                offset: Value::number(0),
            }));
            Ok((result, treg.to_reg(), typ))
        }
        DeclarationInfo::ActionParameter(_) => {
            let treg = VersionedRegister::for_token(&lval.token);
            result.push(Statement::Load(Load {
                target: treg.to_reg(),
                typ: typ.clone(),
                source: Register::new(lval.root()),
                offset: Value::number(0),
            }));
            Ok((result, treg.to_reg(), typ))
        }
        DeclarationInfo::StructMember | DeclarationInfo::HeaderMember => {
            let offsets = member_offsets(ast, names, lval)?;
            let treg = VersionedRegister::for_token(&lval.token);

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
            Ok((result, treg.to_reg(), typ))
        }
        DeclarationInfo::Local => {
            let reg = ra
                .get(&lval.name)
                .ok_or(CodegenError::UndefinedLvalue(lval.clone()))?;
            Ok((result, reg, typ))
        }
        other => todo!("emit lval for \n{other:#?}"),
    }
}

pub(crate) fn emit_bool_lit(
    value: bool,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let instrs = vec![Statement::Rset(Rset {
        target: ra.clone().to_reg(),
        typ: Type::Bool,
        source: Value::bool(value),
    })];
    Ok((instrs, ra.to_reg(), Type::Bool))
}

pub(crate) fn emit_bit_lit(
    width: u16,
    value: u128,
    ra: VersionedRegister,
    expr: &Expression,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let value = i128::try_from(value)
        .map_err(|_| CodegenError::NumericConversion(expr.clone()))?;
    let typ = Type::Bitfield(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, ra.to_reg(), typ))
}

pub(crate) fn emit_int_lit(
    value: i128,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let typ = Type::Signed(128);
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, ra.to_reg(), typ))
}

pub(crate) fn emit_signed_lit(
    width: u16,
    value: i128,
    ra: VersionedRegister,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let typ = Type::Signed(usize::from(width));
    let instrs = vec![Statement::Rset(Rset {
        typ: typ.clone(),
        target: ra.clone().to_reg(),
        source: Value::number(value),
    })];
    Ok((instrs, ra.to_reg(), typ))
}
