// Copyright 2024 Oxide Computer Company

use htq::ast::Register;
use p4::{
    ast::{
        Call, Control, ControlParameter, DeclarationInfo, Expression, Lvalue,
        Transition, Type,
    },
    lexer::Token,
};
use thiserror::Error;

use crate::{RegisterAllocator, TableContext};

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

    #[error("could not determine lookup result arg size for \n{0:#?}")]
    LookupResultArgSize(ControlParameter),

    #[error("register does not exist for lvalue \n{0:#?}")]
    RegisterDoesNotExistForLval(Lvalue),

    #[error("expected control type for \n{0:#?}\nfound \n{1:#?}")]
    ExpectedControl(Lvalue, Type),

    #[error("a value is required for expression \n{0:#?}")]
    ExpressionValueNeeded(Expression),

    #[error("a singular value is required for expression \n{0:#?}")]
    SingularExpressionValueNeeded(Expression),

    #[error("missing register for lvalue, this is a compiler bug \n{0:#?}\ncurrent registers: \n{1:#?}")]
    MissingRegisterForLvalue(Lvalue, Vec<Register>),

    #[error("table not found in context \nlvalue:\n{0:#?}\ncontext:\n{1:#?}")]
    TableNotFoundInContext(Lvalue, TableContext),

    #[error("indirect action call in parser for \n{0:#?}")]
    IndirectActionCallInParser(Lvalue),

    #[error("no register for parameter {0}\nregisters:\n{1:#?}")]
    NoRegisterForParameter(String, RegisterAllocator),

    #[error("action not found in control\naction:\n{0:#?}\ncontrol:\n{1:#?}")]
    ActionNotFound(Lvalue, Control),

    #[error("transition must be in parser context\n{0:#?}")]
    TransitionOutsideParser(Transition),

    #[error("call does not have enough arguments\n{0:#?}")]
    NotEnoughArgs(Lvalue),

    #[error("expected expression got\n{0:#?}")]
    ExpectedLvalue(Expression),

    #[error("header declaration not found\n{0:#?}")]
    HeaderDeclNotFound(Lvalue),

    #[error("expected header type for lvalue\n{0:#?}")]
    ExpectedHeaderType(Lvalue),

    #[error("header definition for type {0} not found for lvalue\n{1:#?}")]
    HeaderDefnNotFound(String, Lvalue),
}

#[derive(Error, Debug)]
pub enum EmitError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("codegen error: {0}")]
    Codegen(#[from] CodegenError),
}
