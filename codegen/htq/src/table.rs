// Copyright 2024 Oxide Computer Company

use p4::hlir::Hlir;

use crate::{error::CodegenError, p4_type_to_htq_type};

pub fn p4_match_kind_to_htq_match_kind(
    p4m: &p4::ast::MatchKind,
) -> htq::ast::LookupType {
    match p4m {
        p4::ast::MatchKind::Exact => htq::ast::LookupType::Exact,
        p4::ast::MatchKind::LongestPrefixMatch => htq::ast::LookupType::Lpm,
        p4::ast::MatchKind::Range => htq::ast::LookupType::Range,
        p4::ast::MatchKind::Ternary => htq::ast::LookupType::Ternary,
    }
}

pub(crate) fn p4_table_to_htq_table(
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
