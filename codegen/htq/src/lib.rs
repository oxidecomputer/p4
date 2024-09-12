// Copyright 2024 Oxide Computer Company

use std::io::Write;

use error::CodegenError;
use htq::emit::Emit;
use p4::hlir::Hlir;

use crate::error::EmitError;

mod error;

pub fn emit(
    ast: &p4::ast::AST,
    hlir: &Hlir,
    filename: &str,
) -> Result<(), EmitError> {
    let headers: Vec<_> =
        ast.headers
            .iter()
            .map(p4_header_to_htq_header)
            .collect::<Result<Vec<htq::ast::Header>, CodegenError>>()?;

    let tables: Vec<_> = ast
        .controls
        .iter()
        .flat_map(|c| c.tables.iter().map(|t| (c, t)).collect::<Vec<_>>())
        .map(|(c, t)| p4_table_to_htq_table(c, t, hlir))
        .collect::<Result<Vec<htq::ast::Table>, CodegenError>>()?;

    let mut f = std::fs::File::create(filename)?;

    for h in &headers {
        writeln!(f, "{}", h.emit())?;
    }

    for t in &tables {
        writeln!(f, "{}", t.emit())?;
    }

    Ok(())
}

fn p4_header_to_htq_header(
    p4h: &p4::ast::Header,
) -> Result<htq::ast::Header, CodegenError> {
    Ok(htq::ast::Header {
        name: p4h.name.clone(),
        fields: p4h
            .members
            .iter()
            .map(p4_header_member_to_htq_type)
            .collect::<Result<Vec<htq::ast::Type>, CodegenError>>()?,
    })
}

fn p4_header_member_to_htq_type(
    p4f: &p4::ast::HeaderMember,
) -> Result<htq::ast::Type, CodegenError> {
    p4_type_to_htq_type(&p4f.ty)
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

fn p4_match_kind_to_htq_match_kind(
    p4m: &p4::ast::MatchKind,
) -> htq::ast::LookupType {
    match p4m {
        p4::ast::MatchKind::Exact => htq::ast::LookupType::Exact,
        p4::ast::MatchKind::LongestPrefixMatch => htq::ast::LookupType::Lpm,
        p4::ast::MatchKind::Range => htq::ast::LookupType::Range,
        p4::ast::MatchKind::Ternary => htq::ast::LookupType::Ternary,
    }
}

fn p4_table_to_htq_table(
    p4c: &p4::ast::Control,
    p4t: &p4::ast::Table,
    hlir: &Hlir,
) -> Result<htq::ast::Table, CodegenError> {
    let mut action_args = Vec::new();
    for a in &p4t.actions {
        let act = p4c.get_action(&a.name).unwrap();
        action_args.push(
            act.parameters
                .iter()
                .map(|x| p4_type_to_htq_type(&x.ty))
                .collect::<Result<Vec<htq::ast::Type>, CodegenError>>()?,
        );
    }
    Ok(htq::ast::Table {
        name: p4t.name.clone(),
        keyset: p4t
            .key
            .iter()
            .map(|(lval, match_kind)| htq::ast::TableKey {
                typ: p4_type_to_htq_type(
                    &hlir.lvalue_decls.get(lval).unwrap().ty,
                )
                .unwrap(),
                lookup_typ: p4_match_kind_to_htq_match_kind(match_kind),
            })
            .collect(),
        action_args,
    })
}
