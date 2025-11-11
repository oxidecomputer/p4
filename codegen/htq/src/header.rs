// Copyright 2024 Oxide Computer Company

use crate::{error::CodegenError, p4_type_to_htq_type};

pub(crate) fn p4_header_to_htq_header(
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

pub(crate) fn p4_struct_to_htq_header(
    p4s: &p4::ast::Struct,
) -> Result<htq::ast::Header, CodegenError> {
    Ok(htq::ast::Header {
        name: p4s.name.clone(),
        fields: p4s
            .members
            .iter()
            .map(p4_struct_member_to_htq_type)
            .collect::<Result<Vec<htq::ast::Type>, CodegenError>>()?,
    })
}

pub(crate) fn p4_header_member_to_htq_type(
    p4f: &p4::ast::HeaderMember,
) -> Result<htq::ast::Type, CodegenError> {
    p4_type_to_htq_type(&p4f.ty)
}

pub(crate) fn p4_struct_member_to_htq_type(
    p4f: &p4::ast::StructMember,
) -> Result<htq::ast::Type, CodegenError> {
    p4_type_to_htq_type(&p4f.ty)
}
