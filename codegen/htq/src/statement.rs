// Copyright 2024 Oxide Computer Company

use crate::error::CodegenError;

pub(crate) fn emit_statement(
    stmt: &p4::ast::Statement,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    use p4::ast::Statement as S;
    match stmt {
        S::Empty => Ok(Vec::new()),
        S::Assignment(_lval, _expr) => todo!(),
        S::Call(_call) => todo!(),
        S::If(_if_block) => todo!(),
        S::Variable(_v) => todo!(),
        S::Constant(_c) => todo!(),
        S::Transition(_t) => todo!(),
        S::Return(_r) => todo!(),
    }
}

fn emit_assignment(
    target: &p4::ast::Lvalue,
    source: &p4::ast::Expression,
) -> Result<Vec<htq::ast::Statement>, CodegenError> {
    // Things that can be assigned to are lvalues.
    todo!();
}
