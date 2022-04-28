use std::collections::HashMap;
use std::fs;
use std::io;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    AST, Direction, Type, Struct, Header, Parser, State, Transition,
    Statement, Lvalue, ControlParameter,
};
use p4::check::{Diagnostics, Diagnostic, Level};

/// An object for keeping track of state as we generate code.
#[derive(Default)]
struct Context {
    /// Rust structs we've generated.
    structs: HashMap<String, TokenStream>,

    /// Rust functions we've generated.
    functions: HashMap<String, TokenStream>,

    /// Diagnositcs collected during code generation
    diags: Diagnostics,
}


pub fn emit(ast: &AST) -> io::Result<Diagnostics> {


    let (tokens, diags) = emit_tokens(ast);

    //
    // format the code and write it out to a Rust source file
    //
    let f: syn::File = syn::parse2(tokens).unwrap();
    fs::write("out.rs", prettyplease::unparse(&f))?;

    Ok(diags)

}

pub fn emit_tokens(ast: &AST) -> (TokenStream, Diagnostics) {

    //
    // initialize a context to track state while we generate code
    //
    let mut ctx = Context::default();

    //
    // genearate rust code for the P4 AST
    //
    handle_parsers(ast, &mut ctx);

    //
    // collect all the tokens we generated into one stream
    //
    
    // start with use statements
    let mut tokens = quote!{ use p4rs::*; };

    // structs
    for s in ctx.structs.values() {
        tokens.extend(s.clone());
    }

    // functions
    for s in ctx.functions.values() {
        tokens.extend(s.clone());
    }

    (tokens, ctx.diags)

}

fn handle_parsers(ast: &AST, ctx: &mut Context) {

    handle_parser_out_parameters(ast, ctx);
    handle_parser_states(ast, ctx);

}

fn handle_parser_states(ast: &AST, ctx: &mut Context) {
    for parser in &ast.parsers {
        for state in &parser.states {
            generate_parser_state_function(ast, parser, state, ctx);
        }
    }
}

fn generate_parser_state_function(
    ast: &AST,
    parser: &Parser,
    state: &State,
    ctx: &mut Context,
) {


    let function_name = format_ident!("{}_{}", parser.name, state.name);

    let mut args = Vec::new();
    for arg in &parser.parameters {
        let name = format_ident!("{}", arg.name);
        let typename = rust_type(&arg.ty);
        args.push(quote! { #name: &#typename });
    }

    let body = generate_parser_state_function_body(ast, parser, state, ctx);

    let function = quote! {
        fn #function_name(#(#args),*) -> bool {
            #body
        }
    };

    ctx.functions.insert(function_name.to_string(), function);

}

fn generate_parser_state_function_body(
    ast: &AST,
    parser: &Parser,
    state: &State,
    ctx: &mut Context,
) -> TokenStream {

    let mut tokens = generate_parser_state_statements(ast, parser, state, ctx);

    tokens.extend(generate_parser_state_transition(ast, parser, state, ctx));

    tokens

}

fn generate_parser_state_statements(
    ast: &AST,
    parser: &Parser,
    state: &State,
    ctx: &mut Context,
) -> TokenStream {

    let tokens = TokenStream::new();

    for stmt in &state.statements {
        match stmt {
            Statement::Empty => continue,
            Statement::Assignment(_lvalue, _expr) => {
                todo!("parser state assignment statement");
            }
            Statement::Call(call) => {
                match check_parser_state_lvalue(
                    ast,
                    parser,
                    state,
                    &call.lval,
                    ctx
                ) {
                    Ok(_) => {},
                    Err(_) => return tokens, //error added to diagnostics
                }
            }
        }
    }

    tokens

}

enum LvalueKind {
    VariableRef,
    FunctionRef,
    MemberRef,
    MethodRef,
}

fn check_parser_state_lvalue(
    ast: &AST,
    parser: &Parser,
    state: &State,
    lval: &Lvalue,
    ctx: &mut Context,
) -> Result<(),()> {

    // an lvalue can be dot separated e.g. foo.bar.baz, start by getting the
    // root of the lvalue and resolving that.
    let parts: Vec<&str> = lval.name.split(".").collect();
    let root = parts[0];

    // first look in parser parameters
    let ty = match get_parser_arg(parser, root) {
        Some(param) => &param.ty,
        None => {
            // TODO next look in variables for this parser state
            todo!();
        }
    };

    check_lvalue_chain(lval, &parts[1..], ty, ast, ctx)?;


    Ok(())
}

fn check_lvalue_chain(
    lval: &Lvalue,
    parts: &[&str],
    ty: &Type,
    ast: &AST,
    ctx: &mut Context,
) -> Result<(),()> {
    match ty {
        Type::Bool => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type bool does not have a member {}", parts[0]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::Error => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type error does not have a member {}", parts[1]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::Bit(size) => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type bit<{}> does not have a member {}",
                        size,
                        parts[0]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::Varbit(size) => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type varbit<{}> does not have a member {}",
                        size,
                        parts[0]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::Int(size) => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type int<{}> does not have a member {}",
                        size,
                        parts[0]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::String => {
            if parts.len() > 0 { 
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type string does not have a member {}", parts[0]),
                    token: lval.token.clone(),
                });
                return Err(())
            }
        }
        Type::UserDefined(name) => {
            // get the parent type definition from the AST and check for the
            // referenced member
            if let Some(parent) = ast.get_struct(name) {
                for member in &parent.members {
                    if member.name == parts[0] {
                        if parts.len() > 1 {
                            return check_lvalue_chain(
                                lval,
                                &parts[1..],
                                &member.ty,
                                ast,
                                ctx,
                            );
                        }
                    }
                }
            }
            else if let Some(parent) = ast.get_header(name) {
                for member in &parent.members {
                    if member.name == parts[0] {
                        if parts.len() > 1 {
                            return check_lvalue_chain(
                                lval,
                                &parts[1..],
                                &member.ty,
                                ast,
                                ctx,
                            );
                        }
                    }
                }
            }
            else if let Some(parent) = ast.get_extern(name) {
                for method in &parent.methods{
                    if method.name == parts[0] {
                        if parts.len() > 1 {
                            ctx.diags.push(Diagnostic{
                                level: Level::Error,
                                message: format!(
                                    "extern methods do not have members"),
                                token: lval.token.clone(),
                            });
                            return Err(());
                        }
                        return Ok(())
                    }
                }
            }
            else {
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "type {} is not defined", name),
                    token: lval.token.clone(),
                });
            }
            ctx.diags.push(Diagnostic{
                level: Level::Error,
                message: format!(
                    "type {} does not have a member {}", name, parts[0]),
                token: lval.token.clone(),
            });
            return Err(());
             
        }
    };
    Ok(())
}

fn get_parser_arg<'a>(
    parser: &'a Parser,
    arg_name: &str,
) -> Option<&'a ControlParameter> {

    for arg in &parser.parameters {
        if arg.name == arg_name {
            return Some(arg)
        }
    }

    None
}

fn generate_parser_state_transition(
    ast: &AST,
    parser: &Parser,
    state: &State,
    ctx: &mut Context,
) -> TokenStream {

    match &state.transition {
        Some(Transition::Reference(next_state)) => {
            match next_state.as_str() {
                "accept" => quote! { return true; },
                "reject" => quote! { return false; },
                state_ref => {
                    let state_name = format_ident!(
                        "{}_{}", parser.name, state_ref);

                    let mut args = Vec::new();
                    for arg in &parser.parameters {
                        let name = format_ident!("{}", arg.name);
                        args.push(quote! { #name } );
                    }
                    quote! { return #state_name( #(#args),* ); }
                }
            }
        }
        Some(Transition::Select(_)) => {
            todo!();
        }
        None => quote! { return false; } // implicit reject?
    }
}

fn handle_parser_out_parameters(ast: &AST, ctx: &mut Context) {
    // - iterate through parsers and look at headers
    // - generate a Struct object for each struct
    // - generate a Header object for each header

    //
    // iterate through the parsers, looking for out parameters and generating
    // Struct and Header object for the ones we find.
    //
    for parser in &ast.parsers {
        for parameter in &parser.parameters {

            // ignore parameters not in an out direction, we're just generating
            // supporting data structures right now.
            if parameter.direction != Direction::Out {
                continue;
            }
            if let Type::UserDefined(ref typename) = parameter.ty {
                if let Some(decl) = ast.get_struct(typename) {
                    generate_struct(ast, decl, ctx)
                }
                else {
                    // semantic error undefined type
                    ctx.diags.push(Diagnostic{
                        level: Level::Error,
                        message: format!(
                            "Undefined type {}",
                            parameter.ty,
                        ),
                        token: parameter.ty_token.clone(),
                    });
                }
            }
            else {
                // semantic error, out parameters must be structures
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "Out parameter must be a struct, found {}",
                        parameter.ty,
                    ),
                    token: parameter.ty_token.clone(),
                });
                    
            }
        }
    }
}

fn generate_struct(ast: &AST, s: &Struct, ctx: &mut Context) {

    let mut members = Vec::new();

    for member in &s.members {
        if let Type::UserDefined(ref typename) = member.ty {
            if let Some(decl) = ast.get_header(typename) {
                // only generate code for types we have not already generated
                // code for.
                if !ctx.structs.contains_key(typename) {
                    generate_header(ast, decl, ctx)
                }
                let name = format_ident!("{}", member.name);
                let ty = format_ident!("{}", typename);
                members.push(quote!{ #name: #ty::<'a> });
            }
            else {
                // semantic error undefined header
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "Undefined header {}",
                        member.ty,
                    ),
                    token: member.token.clone(),
                });
            }
        }
        else {
            //TODO support for primitive types in structs
            ctx.diags.push(Diagnostic{
                level: Level::Error,
                message: "Only headers are supported as struct members".into(),
                token: member.token.clone(),
            });
        }
    }

    let name = format_ident!("{}", s.name);

    let structure = quote! {
        #[derive(Debug)]
        pub struct #name<'a> {
            #(#members),*
        }
    };
    ctx.structs.insert(s.name.clone(), structure);
}


fn generate_header(_ast: &AST, h: &Header, ctx: &mut Context) {

    let name = format_ident!("{}", h.name);

    //
    // genrate a rust struct for the header
    //
    
    // generate struct members
    let mut members = Vec::new();
    for member in &h.members {
        let name = format_ident!("{}", member.name);
        let ty = rust_type(&member.ty);
        members.push(quote! { pub #name: #ty });
    }

    let mut generated = quote! {
        #[derive(Debug)]
        pub struct #name<'a> {
            #(#members),*
        }
    };

    //
    // generate a constructor that maps the header onto a byte slice
    //
    
    // generate member assignments
    let mut member_values = Vec::new();
    let mut set_statements = Vec::new();
    let mut offset = 0;
    for member in &h.members {
        let name = format_ident!("{}", member.name);
        let size = type_size(&member.ty);
        let required_bytes = if size & 7 > 0 {
            (size >> 3) + 1
        } else {
            size >> 3
        };
        let end = offset + required_bytes;
        let ty = rust_type(&member.ty);
        member_values.push(quote! {
            #name: #ty::new(&buf[#offset..#end])?
        });
        set_statements.push(quote! {
            self.#name = #ty::new(&buf[#offset..#end])?
        });
        offset += required_bytes;
    }

    generated.extend(quote! {
        impl<'a> #name<'a> {
            pub fn new(buf: &'a [u8]) -> Result<Self, TryFromSliceError> {
                Ok(Self {
                    #(#member_values),*
                })
            }
        }
        impl<'a> Header<'a> for #name<'a> {
            fn set(&mut self, buf: &'a [u8]) -> Result<(), TryFromSliceError> {
                #(#set_statements);*;
                Ok(())
            }
        }
    });

    ctx.structs.insert(h.name.clone(), generated);

}

fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(size) => quote! { Bit::<'a, #size> },
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        Type::UserDefined(name) => {
            let typename = format_ident!("{}", name);
            quote!{ #typename }
        },
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
    }
}
