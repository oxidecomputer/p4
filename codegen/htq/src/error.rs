// Copyright 2024 Oxide Computer Company

use p4::{
    ast::{Call, Control, DeclarationInfo, Expression, Lvalue, Type},
    lexer::Token,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlagAllocationError {
    #[error("flag overflow: count exceeds 128")]
    Overflow,
}

#[derive(Error, Debug)]
pub enum CodegenError {
    #[error("There is no equivalent htq type for {0}")]
    NoEquivalentType(p4::ast::Type),

    #[error("undefined lvalue \n{0:#?}")]
    UndefinedLvalue(Lvalue),

    #[error("cannot assign to \n{0:#?}")]
    InvalidAssignment(DeclarationInfo),

    #[error("cannot convert numeric type \n{0:#?}\n to u128")]
    NumericConversion(Expression),

    #[error(
        "no type information for \n{0:#?}\nthis is likely a type checking bug"
    )]
    UntypedExpression(Expression),

    #[error(
        "parent {0} for member for \n{1:#?}\nnot found: this is likely a front end bug"
    )]
    MemberParentNotFound(String, Lvalue),

    #[error(
        "expected parent of lvalue \n{0:#?}\nto be a struct: this is likely a front end bug"
    )]
    ExpectedStructParent(Lvalue),

    #[error(
        "expected parent of lvalue \n{0:#?}\nto be a header: this is likely a front end bug"
    )]
    ExpectedHeaderParent(Lvalue),

    #[error("offset for struct or header member \n{0:#?}\nnot found")]
    MemberOffsetNotFound(Lvalue),

    #[error("header member {0}\n{1:#?}\nnot found")]
    MemberNotFound(String, Lvalue),

    #[error("user defined type {0} not found \n{1:#?}")]
    UserDefinedTypeNotFound(String, Token),

    #[error("cannot calculate offset into extern {0} \n{1:#?}")]
    CannotOffsetExtern(String, Token),

    #[error("expressions appearing on the RHS of an assignment must have a value\n{0:#?}")]
    AssignmentExpressionRequiresValue(Expression),

    #[error("parent for call \n{0:#?}\nnot found")]
    CallParentNotFound(Call),

    #[error("cannot make a call on parent type {1}\n{0:#?}")]
    InvalidCallParent(Call, Type),

    #[error("calls are not supported in parsers\n{0:#?}")]
    CallInParser(Expression),

    #[error("table {0} not found in control \n{1:#?}")]
    TableNotFound(String, Control),

    #[error("flag allocation: {0}")]
    FlagAllocation(#[from] FlagAllocationError),

    #[error("key extraction produced no value for \n{0:#?}")]
    KeyExtractionProducedNoValue(Lvalue),
}

#[derive(Error, Debug)]
pub enum EmitError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("codegen error: {0}")]
    Codegen(#[from] CodegenError),
}
