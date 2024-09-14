// Copyright 2024 Oxide Computer Company

use p4::{
    ast::{DeclarationInfo, Expression, Lvalue},
    lexer::Token,
};
use thiserror::Error;

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
}

#[derive(Error, Debug)]
pub enum EmitError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("codegen error: {0}")]
    Codegen(#[from] CodegenError),
}
