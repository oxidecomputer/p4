// Copyright 2024 Oxide Computer Company

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodegenError {
    #[error("There is no equivalent htq type for {0}")]
    NoEquivalentType(p4::ast::Type),
}

#[derive(Error, Debug)]
pub enum EmitError {
    #[error("io error {0}")]
    Io(#[from] std::io::Error),

    #[error("codegen error {0}")]
    Codegen(#[from] CodegenError),
}
