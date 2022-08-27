use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    ControlParameter, DeclarationInfo, Direction, Expression, ExpressionKind,
    Lvalue, NameInfo, Parser, Type, UserDefinedType, AST,
};
use p4::hlir::Hlir;
use p4::util::resolve_lvalue;

use control::ControlGenerator;
use header::HeaderGenerator;
use p4struct::StructGenerator;
use parser::ParserGenerator;
use pipeline::PipelineGenerator;

mod control;
mod expression;
mod header;
mod p4struct;
mod parser;
mod pipeline;
mod statement;

/// An object for keeping track of state as we generate code.
#[derive(Default)]
struct Context {
    /// Rust structs we've generated.
    structs: HashMap<String, TokenStream>,

    /// Rust functions we've generated.
    functions: HashMap<String, TokenStream>,

    /// Pipeline structures we've generated.
    pipelines: HashMap<String, TokenStream>,
}

pub struct Settings {
    /// Name to give to the C-ABI constructor.
    pub pipeline_name: String,
}

pub fn sanitize(ast: &mut AST) {
    for h in &mut ast.headers {
        for m in &mut h.members {
            sanitize_string(&mut m.name);
        }
    }
    for s in &mut ast.structs {
        for m in &mut s.members {
            sanitize_string(&mut m.name);
        }
    }
}

pub fn sanitize_string(s: &mut String) {
    //TODO sanitize other problematic rust tokens
    if s == "type" {
        *s = "typ".to_owned();
    }
}

pub fn emit(
    ast: &AST,
    hlir: &Hlir,
    filename: &str,
    settings: Settings,
) -> io::Result<()> {
    let tokens = emit_tokens(ast, hlir, settings);

    //
    // format the code and write it out to a Rust source file
    //
    let f: syn::File = match syn::parse2(tokens.clone()) {
        Ok(f) => f,
        Err(e) => {
            // On failure write generated code to a tempfile
            println!("Code generation produced unparsable code");
            write_to_tempfile(&tokens)?;
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to parse generated code: {:?}", e),
            ));
        }
    };
    fs::write(filename, prettyplease::unparse(&f))?;

    Ok(())
}

fn write_to_tempfile(tokens: &TokenStream) -> io::Result<()> {
    let mut out = tempfile::Builder::new().suffix(".rs").tempfile()?;
    out.write_all(tokens.to_string().as_bytes())?;
    println!("Wrote generated code to {}", out.path().display());
    out.keep()?;
    Ok(())
}

pub fn emit_tokens(ast: &AST, hlir: &Hlir, settings: Settings) -> TokenStream {
    //
    // initialize a context to track state while we generate code
    //

    let mut ctx = Context::default();

    //
    // genearate rust code for the P4 AST
    //

    let mut hg = HeaderGenerator::new(ast, &mut ctx);
    hg.generate();

    let mut sg = StructGenerator::new(ast, &mut ctx);
    sg.generate();

    let mut pg = ParserGenerator::new(ast, hlir, &mut ctx);
    pg.generate();

    let mut cg = ControlGenerator::new(ast, hlir, &mut ctx);
    cg.generate();

    let mut pg = PipelineGenerator::new(ast, hlir, &mut ctx, &settings);
    pg.generate();

    //
    // collect all the tokens we generated into one stream
    //

    // start with use statements
    let mut tokens = quote! {
        use p4rs::{checksum::Checksum, *};
        use colored::*;
        use bitvec::prelude::*;
    };

    //to lib dtrace probes
    tokens.extend(dtrace_probes());

    // structs
    for s in ctx.structs.values() {
        tokens.extend(s.clone());
    }

    // functions
    for s in ctx.functions.values() {
        tokens.extend(s.clone());
    }

    // pipelines
    for p in ctx.pipelines.values() {
        tokens.extend(p.clone());
    }

    tokens
}

fn dtrace_probes() -> TokenStream {
    quote! {
        #[usdt::provider]
        mod dtrace_provider {
            fn parser_accepted(_: &str) {}
            fn parser_dropped() {}
            fn control_dropped(_: &str) {}
            fn control_accepted(_: &str) {}
        }
    }
}

#[allow(dead_code)]
fn get_parser_arg<'a>(
    parser: &'a Parser,
    arg_name: &str,
) -> Option<&'a ControlParameter> {
    for arg in &parser.parameters {
        if arg.name == arg_name {
            return Some(arg);
        }
    }
    None
}

/// Return the rust type for a given P4 type.
fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(_size) => {
            quote! { BitVec<u8, Msb0> }
        }
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        Type::UserDefined(name) => {
            let typename = format_ident!("{}", name);
            quote! { #typename }
        }
        Type::ExternFunction => {
            todo!("rust type for extern function");
        }
        Type::Table => {
            todo!("rust type for table");
        }
        Type::Void => {
            quote! { () }
        }
        Type::List(_) => todo!("rust type for list"),
    }
}

fn type_size(ty: &Type, ast: &AST) -> usize {
    match ty {
        Type::Bool => 1,
        Type::Error => todo!("generate error size"),
        Type::Bit(size) => *size,
        Type::Int(size) => *size,
        Type::Varbit(size) => *size,
        Type::String => todo!("generate string size"),
        Type::UserDefined(name) => {
            let mut sz: usize = 0;
            let udt = ast.get_user_defined_type(name).unwrap_or_else(|| {
                panic!("expect user defined type: {}", name)
            });

            match udt {
                UserDefinedType::Struct(s) => {
                    for m in &s.members {
                        sz += type_size(&m.ty, ast);
                    }
                    sz
                }
                UserDefinedType::Header(h) => {
                    for m in &h.members {
                        sz += type_size(&m.ty, ast);
                    }
                    sz
                }
                UserDefinedType::Extern(_) => {
                    todo!("size for extern?");
                }
            }
        }
        Type::ExternFunction => {
            todo!("type size for extern function");
        }
        Type::Table => {
            todo!("type size for table");
        }
        Type::Void => 0,
        Type::List(_) => todo!("type size for list"),
    }
}

// in the case of an expression
//
//   a &&& b
//
// where b is an integer literal interpret b as a prefix mask based on the
// number of leading ones
fn try_extract_prefix_len(expr: &Expression) -> Option<u8> {
    match &expr.kind {
        ExpressionKind::Binary(_lhs, _op, rhs) => match &rhs.kind {
            ExpressionKind::IntegerLit(v) => Some(v.trailing_ones() as u8),
            ExpressionKind::BitLit(_width, v) => Some(v.trailing_ones() as u8),
            ExpressionKind::SignedLit(_width, v) => {
                Some(v.trailing_ones() as u8)
            }
            _ => None,
        },
        _ => None,
    }
}

fn is_header(
    lval: &Lvalue,
    ast: &AST,
    names: &HashMap<String, NameInfo>,
) -> bool {
    //TODO: get from hlir?
    let typename = match resolve_lvalue(lval, ast, names).unwrap().ty {
        Type::UserDefined(name) => name,
        _ => return false,
    };
    ast.get_header(&typename).is_some()
}

fn is_header_member(lval: &Lvalue, hlir: &Hlir) -> bool {
    if lval.degree() >= 1 {
        let name_info = hlir
            .lvalue_decls
            .get(lval)
            .unwrap_or_else(|| panic!("name for lval {:#?}", lval));

        matches!(name_info.decl, DeclarationInfo::HeaderMember)
    } else {
        false
    }
}

// TODO define in terms of hlir rather than names
fn is_rust_reference(lval: &Lvalue, names: &HashMap<String, NameInfo>) -> bool {
    if lval.degree() == 1 {
        let name_info = names
            .get(lval.root())
            .unwrap_or_else(|| panic!("name for lval {:#?}", lval));

        match name_info.decl {
            DeclarationInfo::Parameter(Direction::Unspecified) => false,
            DeclarationInfo::Parameter(Direction::In) => false,
            DeclarationInfo::Parameter(Direction::Out) => true,
            DeclarationInfo::Parameter(Direction::InOut) => true,
            DeclarationInfo::Local => false,
            DeclarationInfo::Method => false,
            DeclarationInfo::StructMember => false,
            DeclarationInfo::HeaderMember => false,
            DeclarationInfo::ControlTable => false,
            DeclarationInfo::ControlMember => false,
        }
    } else {
        false
    }
}
