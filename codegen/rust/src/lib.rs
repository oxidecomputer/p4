use std::collections::HashMap;
use std::fs;
use std::io::{ self, Write };

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    AST, BinOp, ControlParameter, Expression, Lvalue, Parser, Type
};

use header::HeaderGenerator;
use p4struct::StructGenerator;
use parser::ParserGenerator;
use control::ControlGenerator;

mod header;
mod p4struct;
mod parser;
mod control;

/// An object for keeping track of state as we generate code.
#[derive(Default)]
struct Context {
    /// Rust structs we've generated.
    structs: HashMap<String, TokenStream>,

    /// Rust functions we've generated.
    functions: HashMap<String, TokenStream>,
}

pub fn emit(ast: &AST, filename: &str) -> io::Result<()> {
    let tokens = emit_tokens(ast);

    //
    // format the code and write it out to a Rust source file
    //
    let f: syn::File = match syn::parse2(tokens.clone()) {
        Ok(f) => f,
        Err(e) => {
            // On failure write generated code to a tempfile
            println!("Code generation produced unparsable code");
            write_to_tempfile(&tokens)?;
            return Err(
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to parse generated code: {:?}", e),
                )
            );
        }
    };
    fs::write(filename, prettyplease::unparse(&f))?;

    Ok(())
}

fn write_to_tempfile(tokens: &TokenStream) -> io::Result<()> {
    let mut out = tempfile::Builder::new()
        .suffix(".rs")
        .tempfile()?;
    out.write_all(tokens.to_string().as_bytes())?;
    println!("Wrote generated code to {}", out.path().display());
    out.keep()?;
    Ok(())
}

pub fn emit_tokens(ast: &AST) -> TokenStream {
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

    let mut pg = ParserGenerator::new(ast, &mut ctx);
    pg.generate();

    let mut cg = ControlGenerator::new(ast, &mut ctx);
    cg.generate();


    //
    // collect all the tokens we generated into one stream
    //

    // start with use statements
    let mut tokens = quote! {
        use p4rs::*;
        use bitvec::prelude::*;
    };

    // structs
    for s in ctx.structs.values() {
        tokens.extend(s.clone());
    }

    // functions
    for s in ctx.functions.values() {
        tokens.extend(s.clone());
    }

    tokens
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





/// Return the rust type for a given P4 type. To support zero copy pipelines,
/// a P4 type such as `bit<N>` may be one of two types. If it's in a header,
/// then it's a reference type like `bit_slice`. If it's not in a header than
/// it's a value type like `bit`.
fn rust_type(ty: &Type, header_member: bool, _offset: usize) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(_size) => {
            if header_member {
                quote!{ BitSlice<u8, Lsb0> }
            } else {
                quote!{ BitVec<u8, Lsb0> }
            }
        }
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        Type::UserDefined(name) => {
            let typename = format_ident!("{}", name);
            quote! { #typename }
        }
        Type::ExternFunction => { todo!("rust type for extern function"); }
    }
}

fn type_size(ty: &Type) -> usize {
    match ty {
        Type::Bool => 1,
        Type::Error => todo!("generate error size"),
        Type::Bit(size) => *size,
        Type::Int(size) => *size,
        Type::Varbit(size) => *size,
        Type::String => todo!("generate string size"),
        Type::UserDefined(_name) => todo!("generate user defined type size"),
        Type::ExternFunction => { todo!("type size for extern function"); }
    }
}

fn type_lifetime(ast: &AST, ty: &Type) -> TokenStream {
    if requires_lifetime(ast, ty) {
        quote! {<'a>}
    } else {
        quote! {}
    }
}

fn requires_lifetime(ast: &AST, ty: &Type) -> bool {
    match ty {
        Type::Bool
        | Type::Error
        | Type::Bit(_)
        | Type::Int(_)
        | Type::Varbit(_)
        | Type::String => {
            return false;
        }
        Type::UserDefined(typename) => {
            if let Some(_) = ast.get_header(typename) {
                return true;
            }
            if let Some(s) = ast.get_struct(typename) {
                for m in &s.members {
                    if requires_lifetime(ast, &m.ty) {
                        return true;
                    }
                }
                return false;
            }
            if let Some(_x) = ast.get_extern(typename) {
                //TODO this should be determined from the packet_in definition?
                //Externs are strange like this in the sense that they are
                //platform specific. Since we are in the Rust platform specific
                //code generation code here, perhaps this hard coding is ok.
                if typename == "packet_in" {
                    return true;
                }
                if typename == "packet_out" {
                    return true;
                }
                return false;
            }
            return false;
        }
        Type::ExternFunction => { todo!("lifetime for extern function"); }
    }
}

fn generate_expression(
    expr: Box<Expression>,
    ctx: &mut Context,
) -> TokenStream {

    match expr.as_ref() {
        Expression::BoolLit(v) => {
            quote!{ #v.into() }
        }
        Expression::IntegerLit(v) => {
            quote!{ #v.into() }
        }
        Expression::BitLit(width, v) => {
            generate_bit_literal(*width, *v)
        }
        Expression::SignedLit(_width, _v) => {
            todo!("generate expression signed lit");
        }
        Expression::Lvalue(v) => {
            generate_lvalue(v)
        }
        Expression::Binary(lhs, op, rhs) => {
            let mut ts = TokenStream::new();
            ts.extend(generate_expression(lhs.clone(), ctx));
            ts.extend(generate_binop(*op, ctx));
            ts.extend(generate_expression(rhs.clone(), ctx));
            ts
        }
        Expression::Index(lval, xpr) => {
            let mut ts = generate_lvalue(lval);
            ts.extend(generate_expression(xpr.clone(), ctx));
            ts
        }
        Expression::Slice(begin, end) => {
            let lhs = generate_expression(begin.clone(), ctx);
            let rhs = generate_expression(end.clone(), ctx);
            quote!{
                [#lhs..#rhs]
            }
        }
    }

}

// in the case of an expression
//
//   a &&& b
//
// where b is an integer literal interpret b as a prefix mask based on the
// number of leading ones
fn try_extract_prefix_len(
    expr: &Box<Expression>
) -> Option<u8> {

    match expr.as_ref() {
        Expression::Binary(_lhs, _op, rhs) => {
            match rhs.as_ref() {
                Expression::IntegerLit(v) => {
                    Some(v.leading_ones() as u8)
                }
                Expression::BitLit(_width, v) => {
                    Some(v.leading_ones() as u8)
                }
                Expression::SignedLit(_width, v) => {
                    Some(v.leading_ones() as u8)
                }
                _ => { None }
            }
        }
        _ => { None }
    }

}

fn generate_lvalue(lval: &Lvalue) -> TokenStream {

    let lv: Vec<TokenStream> = lval
        .name
        .split(".")
        .map(|x| format_ident!("{}", x))
        .map(|x| quote! { #x })
        .collect();

    return quote!{ #(#lv).* };

}

fn generate_bit_literal(
    width: u16,
    value: u128,
) -> TokenStream {

    assert!(width <= 128);

    let width = width as usize;

    if width <= 8 {
        let v = value as u8;
        return quote! { #v.view_bits::<Lsb0>().to_bitvec() }
    }
    else if width <= 16 {
        let v = value as u16;
        return quote! { #v.view_bits::<Lsb0>().to_bitvec() }
    }
    else if width <= 32 {
        let v = value as u32;
        return quote! { #v.view_bits::<Lsb0>().to_bitvec() }
    }
    else if width <= 64 {
        let v = value as u64;
        return quote! { #v.view_bits::<Lsb0>().to_bitvec() }
    }
    else if width <= 128 {
        let v = value as u128;
        return quote! { 
            {
                let mut x = bitvec![mut u8, Lsb0; 0; 128];
                x.store(#v);
                x
            }
        }
    }
    else {
        todo!("bit<x> where x > 128");
    }
}

fn generate_binop(
    op: BinOp,
    _ctx: &mut Context,
) -> TokenStream {

    match op {
        BinOp::Add => quote! { + },
        BinOp::Subtract=> quote! { - },
        BinOp::Geq => quote! { >= },
        BinOp::Eq => quote! { == },
        BinOp::Mask => quote! { & },
    }

}

fn is_header(
    lval: &Lvalue,
    ast: &AST,
    names: &HashMap::<String, Type>,
) -> bool {

    let typename = match lvalue_type(lval, ast, names) {
        Type::UserDefined(name) => name,
        _ => return false,
    };
    match ast.get_header(&typename) {
        Some(_) => true,
        None => false,
    }
}

fn lvalue_type(
    lval: &Lvalue,
    ast: &AST,
    names: &HashMap::<String, Type>,
) -> Type {
    let root_type = match names.get(lval.root()) {
        Some(ty) => ty,
        None => panic!("codegen: unresolved lval {:#?}", lval),
    };
    match root_type {
        Type::Bool => root_type.clone(),
        Type::Error => root_type.clone(),
        Type::Bit(_) => root_type.clone(),
        Type::Varbit(_) => root_type.clone(),
        Type::Int(_) => root_type.clone(),
        Type::String => root_type.clone(),
        Type::ExternFunction => root_type.clone(),
        Type::UserDefined(name) => {
            if lval.degree() == 1 {
                Type::UserDefined(name.clone())
            }
            else if let Some(parent) = ast.get_struct(name) {
                let mut tm = parent.names();
                for m in &parent.members {
                    tm.insert(m.name.clone(), m.ty.clone());
                }
                lvalue_type(
                    &lval.pop_left(),
                    ast,
                    &tm,
                )
            }
            else if let Some(parent) = ast.get_header(name) {
                let mut tm = parent.names();
                for m in &parent.members {
                    tm.insert(m.name.clone(), m.ty.clone());
                }
                lvalue_type(
                    &lval.pop_left(),
                    ast,
                    &tm,
                )
            }
            else if let Some(parent) = ast.get_extern(name) {
                let mut tm = parent.names();
                for m in &parent.methods {
                    tm.insert(m.name.clone(), Type::ExternFunction);
                }
                lvalue_type(
                    &lval.pop_left(),
                    ast,
                    &tm,
                )
            }
            else {
                panic!("codegen: User defined name '{}' does not exist", name)
            }
        }
    }
}
