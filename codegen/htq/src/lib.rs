// Copyright 2024 Oxide Computer Company

use std::io::Write;

use error::CodegenError;
use header::{p4_header_to_htq_header, p4_struct_to_htq_header};
use htq::emit::Emit;
use p4::hlir::Hlir;
use parser::emit_parser_functions;
use table::p4_table_to_htq_table;

use crate::error::EmitError;

mod error;
mod expression;
mod header;
mod parser;
mod statement;
mod table;

pub fn emit(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    filename: &str,
) -> Result<(), EmitError> {
    let mut headers: Vec<_> =
        ast.headers
            .iter()
            .map(p4_header_to_htq_header)
            .collect::<Result<Vec<htq::ast::Header>, CodegenError>>()?;

    headers.extend(
        ast.structs
            .iter()
            .map(p4_struct_to_htq_header)
            .collect::<Result<Vec<htq::ast::Header>, CodegenError>>()?,
    );

    let tables: Vec<_> = ast
        .controls
        .iter()
        .flat_map(|c| c.tables.iter().map(|t| (c, t)).collect::<Vec<_>>())
        .map(|(c, t)| p4_table_to_htq_table(c, t, hlir))
        .collect::<Result<Vec<htq::ast::Table>, CodegenError>>()?;

    let parser_functions: Vec<_> = emit_parser_functions(ast, hlir)?;

    let mut f = std::fs::File::create(filename)?;

    for h in &headers {
        writeln!(f, "{}", h.emit())?;
    }
    writeln!(f)?;

    for t in &tables {
        writeln!(f, "{}", t.emit())?;
    }
    writeln!(f)?;

    for func in &parser_functions {
        //TODO htq function emit
        writeln!(f, "{}", func.emit())?;
    }
    writeln!(f)?;

    Ok(())
}

fn p4_type_to_htq_type(
    p4ty: &p4::ast::Type,
) -> Result<htq::ast::Type, CodegenError> {
    Ok(match p4ty {
        p4::ast::Type::Bool => htq::ast::Type::Bool,
        p4::ast::Type::Bit(n) => htq::ast::Type::Bitfield(*n),
        p4::ast::Type::Varbit(n) => htq::ast::Type::Bitfield(*n),
        p4::ast::Type::Int(n) => htq::ast::Type::Unsigned(*n),
        p4::ast::Type::UserDefined(name, _) => {
            htq::ast::Type::User(name.clone())
        }
        t @ p4::ast::Type::Table => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::Error => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::Void => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::State => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::Action => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::ExternFunction => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::HeaderMethod => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::Sync(_) => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        t @ p4::ast::Type::List(_) => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        p4::ast::Type::String => todo!("string types not yet supported"),
    })
}
