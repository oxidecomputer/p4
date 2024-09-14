use htq::ast::{Register, Rset, Statement, Type, Value};
use p4::ast::{Expression, ExpressionKind};

// Copyright 2024 Oxide Computer Company
use crate::{error::CodegenError, VersionedRegister};

// Builds a vector of statements that implement the expression. Returns the
// statements and the register the result of the expression is held in.
pub(crate) fn emit_expression(
    expr: &Expression,
) -> Result<(Vec<Statement>, Register, Type), CodegenError> {
    let r = VersionedRegister::for_expression(expr);
    match &expr.kind {
        ExpressionKind::BoolLit(value) => emit_bool_lit(*value, r),
        ExpressionKind::BitLit(width, value) => {
            emit_bit_lit(*width, *value, r, expr)
        }
        ExpressionKind::IntegerLit(value) => emit_int_lit(*value, r),
        ExpressionKind::SignedLit(width, value) => {
            emit_signed_lit(*width, *value, r)
        }
        ExpressionKind::Call(_) => {
            //TODO
            Ok((
                Vec::default(),
                Register::new("badreg"),
                Type::User("badtype".to_owned()),
            ))
        }
        ExpressionKind::Lvalue(_) => {
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
