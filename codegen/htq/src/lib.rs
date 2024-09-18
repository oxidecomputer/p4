// Copyright 2024 Oxide Computer Company

use std::{collections::HashMap, io::Write};

use control::emit_control_functions;
use error::{CodegenError, FlagAllocationError};
use header::{p4_header_to_htq_header, p4_struct_to_htq_header};
use htq::{ast::Register, emit::Emit};
use p4::{hlir::Hlir, lexer::Token};
use parser::emit_parser_functions;
use table::p4_table_to_htq_table;

use crate::error::EmitError;

mod control;
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
    let mut afa = AsyncFlagAllocator::default();
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

    let parser_functions: Vec<_> = emit_parser_functions(ast, hlir, &mut afa)?;
    let control_functions: Vec<_> =
        emit_control_functions(ast, hlir, &mut afa)?;

    // code generation done, now write out the htq AST to a file

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
        writeln!(f, "{}", func.emit())?;
    }
    writeln!(f)?;

    for func in &control_functions {
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
        p4::ast::Type::Sync(_) => {
            htq::ast::Type::User(String::from("lookup_result"))
            //htq::ast::Type::Signed(128)
            //return Err(CodegenError::NoEquivalentType(t.clone()))
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
        t @ p4::ast::Type::List(_) => {
            return Err(CodegenError::NoEquivalentType(t.clone()))
        }
        p4::ast::Type::String => todo!("string types not yet supported"),
    })
}

#[derive(Default)]
pub(crate) struct RegisterAllocator {
    data: HashMap<String, usize>,
}

impl RegisterAllocator {
    pub(crate) fn alloc(&mut self, name: &str) -> htq::ast::Register {
        match self.data.get_mut(name) {
            Some(rev) => {
                *rev += 1;
                htq::ast::Register::new(&format!("{}.{}", name, *rev))
            }
            None => {
                self.data.insert(name.to_owned(), 0);
                htq::ast::Register::new(name)
            }
        }
    }

    pub(crate) fn get(&self, name: &str) -> Option<htq::ast::Register> {
        self.data
            .get(name)
            .map(|rev| htq::ast::Register::new(&format!("{}.{}", name, rev)))
    }
}

/// The async flag allocator allocates bitmap entries for asynchronous
/// operations. HTQ supports up to 128 flags through an underlying u128 type.
#[derive(Default)]
pub(crate) struct AsyncFlagAllocator {
    flag: u128,
}

impl AsyncFlagAllocator {
    pub(crate) fn allocate(&mut self) -> Result<u128, FlagAllocationError> {
        if self.flag == u128::MAX {
            return Err(FlagAllocationError::Overflow);
        }
        // simple in-order allocation with no deallocation for now,
        // can make this more sophisticated with deallocation and
        // a back-filling allocator later ...
        let pos = self.flag.leading_ones();
        let value = 1 << pos;
        self.flag |= value;
        Ok(value)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VersionedRegister {
    pub(crate) reg: Register,
    pub(crate) version: usize,
}

impl VersionedRegister {
    pub(crate) fn tmp_for_token(tk: &Token) -> Self {
        Self {
            reg: Register::new(&format!("tmp{}_{}", tk.line, tk.col)),
            version: 0,
        }
    }
    pub(crate) fn for_token(prefix: &str, tk: &Token) -> Self {
        Self {
            reg: Register::new(&format!("{}{}_{}", prefix, tk.line, tk.col)),
            version: 0,
        }
    }
    pub(crate) fn for_name(name: &str) -> Self {
        Self {
            reg: Register::new(name),
            version: 0,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn next(&mut self) -> &Self {
        self.version += 1;
        self
    }

    pub(crate) fn name(&self) -> String {
        if self.version == 0 {
            self.reg.0.clone()
        } else {
            format!("{}.{}", self.reg.0, self.version)
        }
    }

    pub(crate) fn to_reg(&self) -> Register {
        Register::new(&self.name())
    }
}

// Codegen context
// TODO more comprehensive ....
pub(crate) enum P4Context<'a> {
    #[allow(dead_code)]
    Parser(&'a p4::ast::Parser),
    Control(&'a p4::ast::Control),
}
