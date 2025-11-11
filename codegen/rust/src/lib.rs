// Copyright 2022 Oxide Computer Company

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    ActionParameter, Control, ControlParameter, DeclarationInfo, Direction,
    Expression, ExpressionKind, HeaderMember, Lvalue, MutVisitor, NameInfo,
    Parser, StructMember, Table, Type, AST,
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

pub struct Sanitizer {}

impl Sanitizer {
    pub fn sanitize_string(s: &mut String) {
        //TODO sanitize other problematic rust tokens
        if s == "type" {
            "typ".clone_into(s)
        }
        if s == "match" {
            "match_".clone_into(s)
        }
    }
}

impl MutVisitor for Sanitizer {
    fn struct_member(&self, m: &mut StructMember) {
        Self::sanitize_string(&mut m.name);
    }
    fn header_member(&self, m: &mut HeaderMember) {
        Self::sanitize_string(&mut m.name);
    }
    fn control_parameter(&self, p: &mut ControlParameter) {
        Self::sanitize_string(&mut p.name);
    }
    fn action_parameter(&self, p: &mut ActionParameter) {
        Self::sanitize_string(&mut p.name);
    }
    fn lvalue(&self, lv: &mut Lvalue) {
        Self::sanitize_string(&mut lv.name);
    }
}

pub fn sanitize(ast: &mut AST) {
    let s = Sanitizer {};
    ast.mut_accept(&s);
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
        mod softnpu_provider {
            fn parser_accepted(_: &str) {}
            fn parser_transition(_: &str) {}
            fn parser_dropped() {}
            fn control_apply(_: &str) {}
            fn control_table_hit(_: &str) {}
            fn control_table_miss(_: &str) {}
            fn ingress_dropped(_: &str) {}
            fn ingress_accepted(_: &str) {}
            fn egress_dropped(_: &str) {}
            fn egress_accepted(_: &str) {}
            fn egress_table_hit(_: &str) {}
            fn egress_table_miss(_: &str) {}
            fn action(_: &str) {}
        }
    }
}

#[allow(dead_code)]
fn get_parser_arg<'a>(
    parser: &'a Parser,
    arg_name: &str,
) -> Option<&'a ControlParameter> {
    parser.parameters.iter().find(|&arg| arg.name == arg_name)
}

/// Return the rust type for a given P4 type.
fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(_size) => {
            quote! { BitVec::<u8, Msb0> }
        }
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        //TODO generic types
        Type::UserDefined(name, _) => {
            let typename = format_ident!("{}", name);
            quote! { #typename }
        }
        Type::ExternFunction => {
            todo!("rust type for extern function");
        }
        Type::HeaderMethod => {
            todo!("rust type for header method");
        }
        Type::Table => {
            todo!("rust type for table");
        }
        Type::Void => {
            quote! { () }
        }
        Type::List(_) => todo!("rust type for list"),
        Type::State => {
            todo!("rust type for state");
        }
        Type::Action => {
            todo!("rust type for action");
        }
        Type::Sync(_) => todo!("rust codegen for sync<T>"),
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
            ExpressionKind::IntegerLit(v) => Some(v.leading_ones() as u8),
            ExpressionKind::BitLit(_width, v) => Some(v.leading_ones() as u8),
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
        Type::UserDefined(name, _) => name,
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
            DeclarationInfo::State => false,
            DeclarationInfo::Action => false,
            DeclarationInfo::ActionParameter(_) => false,
        }
    } else {
        false
    }
}

fn qualified_table_name(
    control: Option<&Control>,
    chain: &Vec<(String, &Control)>,
    table: &Table,
) -> String {
    match control {
        Some(control) => {
            format!("{}.{}", control.name, table_qname(chain, table, '.'))
        }
        _ => table_qname(chain, table, '.'),
    }
}

fn qualified_table_function_name(
    control: Option<&Control>,
    chain: &Vec<(String, &Control)>,
    table: &Table,
) -> String {
    match control {
        Some(control) => {
            format!("{}_{}", control.name, table_qname(chain, table, '_'))
        }
        _ => table_qname(chain, table, '_'),
    }
}

fn table_qname(
    chain: &Vec<(String, &Control)>,
    table: &Table,
    sep: char,
) -> String {
    let mut qname = String::new();
    for c in chain {
        if c.0.is_empty() {
            continue;
        }
        qname += &format!("{}{}", c.0, sep);
    }
    qname += &table.name;
    qname
}
