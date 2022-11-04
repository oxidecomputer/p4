// Copyright 2022 Oxide Computer Company

use crate::ast::{Lvalue, NameInfo, Type, AST};
use std::collections::HashMap;

pub fn resolve_lvalue(
    lval: &Lvalue,
    ast: &AST,
    names: &HashMap<String, NameInfo>,
) -> Result<NameInfo, String> {
    let root = match names.get(lval.root()) {
        Some(name_info) => name_info,
        None => return Err(format!("codegen: unresolved lval {:#?}", lval)),
    };
    let result = match &root.ty {
        Type::Bool => root.clone(),
        Type::Error => root.clone(),
        Type::Bit(_) => root.clone(),
        Type::Varbit(_) => root.clone(),
        Type::Int(_) => root.clone(),
        Type::String => root.clone(),
        Type::ExternFunction => root.clone(),
        Type::Table => root.clone(),
        Type::Void => root.clone(),
        Type::List(_) => root.clone(),
        Type::State => root.clone(),
        Type::UserDefined(name) => {
            if lval.degree() == 1 {
                root.clone()
            } else if let Some(parent) = ast.get_struct(name) {
                resolve_lvalue(&lval.pop_left(), ast, &parent.names())?
            } else if let Some(parent) = ast.get_header(name) {
                resolve_lvalue(&lval.pop_left(), ast, &parent.names())?
            } else if let Some(parent) = ast.get_extern(name) {
                resolve_lvalue(&lval.pop_left(), ast, &parent.names())?
            } else {
                return Err(format!(
                    "codegen: User defined name '{}' does not exist",
                    name
                ));
            }
        }
    };
    Ok(result)
}
