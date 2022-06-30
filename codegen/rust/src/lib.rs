use std::collections::HashMap;
use std::fs;
use std::io::{ self, Write };

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    Action, ActionParameter, Control, ControlParameter, Direction, Expression,
    Header, KeySetElementValue, Lvalue, Parser, State, Statement,
    Struct, Table, Transition, Type, AST, BinOp, MatchKind
};
use p4::check::{Diagnostic, Diagnostics, Level};

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

    if !diags.errors().is_empty() {
        return Ok(diags);
    }

    //
    // format the code and write it out to a Rust source file
    //
    let f: syn::File = match syn::parse2(tokens.clone()) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to parse generated code: {:?}", e);
            let mut out = tempfile::Builder::new()
                .suffix(".rs")
                .tempfile()?;
            out.write_all(tokens.to_string().as_bytes())?;
            println!("Wrote generated code to {}", out.path().display());
            out.keep()?;
            return Err(
                io::Error::new(io::ErrorKind::Other, format!("{:?}", e))
            );
        }
    };
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
    handle_structs(ast, &mut ctx);
    handle_headers(ast, &mut ctx);
    handle_parsers(ast, &mut ctx);
    handle_control_blocks(ast, &mut ctx);

    //
    // collect all the tokens we generated into one stream
    //

    // start with use statements
    let mut tokens = quote! { use p4rs::*; use bitvec::prelude::*; };

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

fn handle_headers(ast: &AST, ctx: &mut Context) {
    for h in &ast.headers {
        generate_header(ast, h, ctx);
    }
}

fn handle_structs(ast: &AST, ctx: &mut Context) {
    for s in &ast.structs {
        generate_struct(ast, s, ctx);
    }
}

fn handle_parsers(ast: &AST, ctx: &mut Context) {
    // parsers
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
        let typename = rust_type(&arg.ty, false, 0);
        let lifetime = type_lifetime(ast, &arg.ty);
        match arg.direction {
            Direction::Out | Direction::InOut => {
                args.push(quote! { #name: &mut #typename #lifetime });
            }
            _ => args.push(quote! { #name: &mut #typename #lifetime }),
        };
    }

    let body = generate_parser_state_function_body(ast, parser, state, ctx);

    let function = quote! {
        pub fn #function_name<'a>(#(#args),*) -> bool {
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
    let mut tokens = TokenStream::new();

    for stmt in &state.statements {
        match stmt {
            Statement::Empty => continue,
            Statement::Assignment(_lvalue, _expr) => {
                todo!("parser state assignment statement");
            }
            Statement::Call(call) => {
                match check_parser_state_lvalue(ast, parser, &call.lval, ctx) {
                    Ok(_) => {
                        let lval: Vec<TokenStream> = call
                            .lval
                            .name
                            .split(".")
                            .map(|x| format_ident!("{}", x))
                            .map(|x| quote! { #x })
                            .collect();

                        let mut args = Vec::new();
                        for a in &call.args {
                            match a.as_ref() {
                                Expression::Lvalue(lvarg) => {
                                    let parts: Vec<&str> =
                                        lvarg.name.split(".").collect();
                                    let root = parts[0];
                                    let mut mut_arg = false;
                                    for parg in &parser.parameters {
                                        if parg.name == root {
                                            match parg.direction {
                                                Direction::Out
                                                | Direction::InOut => {
                                                    mut_arg = true;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    let lvref: Vec<TokenStream> = parts
                                        .iter()
                                        .map(|x| format_ident!("{}", x))
                                        .map(|x| quote! { #x })
                                        .collect();
                                    if mut_arg {
                                        args.push(quote! { &mut #(#lvref).* });
                                    } else {
                                        args.push(quote! { #(#lvref).* });
                                    }
                                }
                                x => todo!("extern arg {:?}", x),
                            }
                        }
                        tokens.extend(quote! {
                            #(#lval).* ( #(#args),* );
                        });
                    }
                    Err(_) => return tokens, //error added to diagnostics
                }
            }
        }
    }

    tokens
}

fn check_parser_state_lvalue(
    ast: &AST,
    parser: &Parser,
    lval: &Lvalue,
    ctx: &mut Context,
) -> Result<(), ()> {
    // an lvalue can be dot separated e.g. foo.bar.baz, start by getting the
    // root of the lvalue and resolving that.
    let parts: Vec<&str> = lval.name.split(".").collect();
    let root = parts[0];

    // first look in parser parameters for the root of the lvalue
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
) -> Result<Vec<Type>, ()> {
    match ty {
        Type::Bool => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type bool does not have a member {}",
                        parts[0]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::Error => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type error does not have a member {}",
                        parts[1]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::Bit(size) => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type bit<{}> does not have a member {}",
                        size, parts[0]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::Varbit(size) => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type varbit<{}> does not have a member {}",
                        size, parts[0]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::Int(size) => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type int<{}> does not have a member {}",
                        size, parts[0]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::String => {
            if parts.len() > 0 {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!(
                        "type string does not have a member {}",
                        parts[0]
                    ),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            return Ok(vec![ty.clone()]);
        }
        Type::UserDefined(name) => {
            // get the parent type definition from the AST and check for the
            // referenced member
            if let Some(parent) = ast.get_struct(name) {
                for member in &parent.members {
                    if member.name == parts[0] {
                        if parts.len() > 1 {
                            let mut types = check_lvalue_chain(
                                lval,
                                &parts[1..],
                                &member.ty,
                                ast,
                                ctx,
                            )?;
                            types.push(member.ty.clone());
                            return Ok(types);
                        } else {
                            return Ok(vec![member.ty.clone()]);
                        }
                    }
                }
            } else if let Some(parent) = ast.get_header(name) {
                for member in &parent.members {
                    if member.name == parts[0] {
                        if parts.len() > 1 {
                            let mut types = check_lvalue_chain(
                                lval,
                                &parts[1..],
                                &member.ty,
                                ast,
                                ctx,
                            )?;
                            types.push(member.ty.clone());
                            return Ok(types);
                        } else {
                            return Ok(vec![member.ty.clone()]);
                        }
                    }
                }
            } else if let Some(parent) = ast.get_extern(name) {
                for method in &parent.methods {
                    if method.name == parts[0] {
                        if parts.len() > 1 {
                            ctx.diags.push(Diagnostic {
                                level: Level::Error,
                                message: format!(
                                    "extern methods do not have members"
                                ),
                                token: lval.token.clone(),
                            });
                            return Err(());
                        } else {
                            return Ok(vec![ty.clone()]); //TODO function/method type?
                        }
                    }
                }
            } else {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!("type {} is not defined", name),
                    token: lval.token.clone(),
                });
                return Err(());
            }
            ctx.diags.push(Diagnostic {
                level: Level::Error,
                message: format!(
                    "type {} does not have a member {}",
                    name, parts[0]
                ),
                token: lval.token.clone(),
            });
            return Err(());
        }
    };
}

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

fn get_control_arg<'a>(
    control: &'a Control,
    arg_name: &str,
) -> Option<&'a ControlParameter> {
    for arg in &control.parameters {
        if arg.name == arg_name {
            return Some(arg);
        }
    }
    None
}

fn get_action_arg<'a>(
    action: &'a Action,
    arg_name: &str,
) -> Option<&'a ActionParameter> {
    for arg in &action.parameters {
        if arg.name == arg_name {
            return Some(arg);
        }
    }
    None
}

fn generate_parser_state_transition(
    _ast: &AST,
    parser: &Parser,
    state: &State,
    _ctx: &mut Context,
) -> TokenStream {
    match &state.transition {
        Some(Transition::Reference(next_state)) => match next_state.as_str() {
            "accept" => quote! { return true; },
            "reject" => quote! { return false; },
            state_ref => {
                let state_name = format_ident!("{}_{}", parser.name, state_ref);

                let mut args = Vec::new();
                for arg in &parser.parameters {
                    let name = format_ident!("{}", arg.name);
                    args.push(quote! { #name });
                }
                quote! { return #state_name( #(#args),* ); }
            }
        },
        Some(Transition::Select(_)) => {
            todo!();
        }
        None => quote! { return false; }, // implicit reject?
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
                if let Some(_decl) = ast.get_struct(typename) {
                } else {
                    // semantic error undefined type
                    ctx.diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Undefined type {}", parameter.ty,),
                        token: parameter.ty_token.clone(),
                    });
                }
            } else {
                // semantic error, out parameters must be structures
                ctx.diags.push(Diagnostic {
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

    let mut needs_lifetime = false;

    for member in &s.members {
        let name = format_ident!("{}", member.name);
        match &member.ty {
            Type::UserDefined(ref typename) => {
                if let Some(decl) = ast.get_header(typename) {
                    // only generate code for types we have not already generated
                    // code for.
                    if !ctx.structs.contains_key(typename) {
                        generate_header(ast, decl, ctx)
                    }
                    let ty = format_ident!("{}", typename);
                    members.push(quote! { pub #name: #ty::<'a> });
                    needs_lifetime = true;
                } else {
                    // semantic error undefined header
                    ctx.diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Undefined header {}", member.ty,),
                        token: member.token.clone(),
                    });
                }
            }
            Type::Bit(_size) => {
                members.push(quote! { pub #name: BitVec::<u8, Lsb0> });
            }
            x => {
                todo!("struct member {}", x)
            }
        }
    }

    let name = format_ident!("{}", s.name);

    let lifetime = if needs_lifetime {
        quote! { <'a> }
    } else {
        quote! {}
    };

    let structure = quote! {
        #[derive(Debug)]
        pub struct #name #lifetime {
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
    let mut offset = 0;
    for member in &h.members {
        let size = type_size(&member.ty);
        let name = format_ident!("{}", member.name);
        let ty = rust_type(&member.ty, true, offset);
        members.push(quote! { pub #name: Option::<&'a mut #ty> });
        offset += size;
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
        /*
        let required_bytes = if (size + offset) & 7 > 0 || size & 7 > 0 {
            (size >> 3) + 1
        } else {
            size >> 3
        };
        let ty = rust_type(&member.ty, true, offset);
        */
        member_values.push(quote! {
            #name: None
        });
        //let off = offset >> 3;
        let end = offset+size;
        set_statements.push(quote! {
            self.#name = Some(
                &mut (*std::ptr::slice_from_raw_parts_mut(
                    buf.as_mut_ptr(),
                    buf.len(),
                )).view_bits_mut::<Lsb0>()[#offset..#end]
            )
        });
        offset += size;
    }

    generated.extend(quote! {
        impl<'a> Header<'a> for #name<'a> {
            fn new() -> Self {
                Self {
                    #(#member_values),*
                }
            }
            fn set(&mut self, buf: &'a mut [u8]) -> Result<(), TryFromSliceError> {
                unsafe {
                    #(#set_statements);*;
                }
                Ok(())
            }
            fn size() -> usize {
                #offset
            }
        }
    });

    ctx.structs.insert(h.name.clone(), generated);
}

fn handle_control_blocks(ast: &AST, ctx: &mut Context) {
    for control in &ast.controls {
        let (mut params, param_types) = control_parameters(ast, control, ctx);

        for action in &control.actions {
            generate_control_action(ast, control, action, ctx);
        }
        for table in &control.tables {
            let (type_tokens, table_tokens) =
                generate_control_table(ast, control, table, &param_types, ctx);
            let name = format_ident!("{}_table_{}", control.name, table.name);
            ctx.functions.insert(
                name.to_string(),
                quote! {
                    pub fn #name<'a>() -> #type_tokens {
                        #table_tokens
                    }
                },
            );
            let name = format_ident!("{}", table.name);
            params.push(quote! {
                #name: &#type_tokens
            });
        }

        let name = format_ident!("{}_apply", control.name);
        let apply_body = generate_control_apply_body(ast, control, ctx);
        ctx.functions.insert(
            name.to_string(),
            quote! {
                pub fn #name<'a>(#(#params),*) {
                    #apply_body
                }
            },
        );
    }
}
fn generate_control_apply_body(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
) -> TokenStream {
    let mut tokens = TokenStream::new();

    for stmt in &control.apply.statements {
        match stmt {
            Statement::Call(c) => {
                // TODO only supporting tably apply calls for now

                //
                // get the referenced table
                //
                let parts: Vec<&str> = c.lval.name.split(".").collect();
                if parts.len() != 2 || parts[1] != "apply" {
                    ctx.diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!(
                            "Only <tablename>.apply() calls are
                            supported in apply blocks right now"
                        ),
                        token: c.lval.token.clone(),
                    });
                    return tokens;
                }
                let table = match control.get_table(parts[0]) {
                    Some(table) => table,
                    None => {
                        ctx.diags.push(Diagnostic {
                            level: Level::Error,
                            message: format!(
                                "Table {} not found in control {}",
                                parts[0], control.name,
                            ),
                            token: c.lval.token.clone(),
                        });
                        return tokens;
                    }
                };

                //
                // match an action based on the key material
                //

                // TODO only supporting direct matches right now and implicitly
                // assuming all match kinds are direct
                let table_name: Vec<TokenStream> = table
                    .name
                    .split(".")
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote! { #x })
                    .collect();

                let mut action_args = Vec::new();
                for p in &control.parameters {
                    let name = format_ident!("{}", p.name);
                    action_args.push(quote! { #name });
                }

                let mut selector_components = Vec::new();
                for (lval, _match_kind) in &table.key {

                    //TODO check lvalue ref here, should already be checked at
                    //this point?
                    let lvref: Vec<TokenStream> = lval
                        .name
                        .split(".")
                        .map(|x| format_ident!("{}", x))
                        .map(|x| quote! { #x })
                        .collect();

                    // determine if this lvalue references a header or a struct,
                    // if it's a header there's a bit of extra unsrapping we
                    // need to do to match the selector against the value.

                    let parts: Vec<&str> = lval.name.split(".").collect();
                    //should already be checked?
                    let param = control.get_parameter(parts[0]).unwrap();

                    let ty = match check_lvalue_chain(
                        lval,
                        &parts[1..],
                        &param.ty,
                        ast,
                        ctx,
                    ) {
                        Ok(ty) => {
                            ty
                        }
                        Err(_) => {
                            // diagnostics have been added to context so just
                            // bail with an empty result
                            return quote! { };
                        }
                    };
                    let is_header = {
                        if ty.len() < 2 {
                            false
                        } else {
                            match &ty[1] {
                                Type::UserDefined(name) => {
                                    match ast.get_header(&name) {
                                        Some(_) => true,
                                        None => {
                                            false
                                        }
                                    }
                                },
                                _ => false,
                            }
                        }
                    };

                    if is_header {
                        //TODO: to_bitvec is bad here, copying on data path
                        selector_components.push(quote!{
                            p4rs::bitvec_to_biguint(
                                &#(#lvref).*.as_ref().unwrap().to_bitvec()
                            )
                        });
                    } else {
                        selector_components.push(quote!{
                            p4rs::bitvec_to_biguint(&#(#lvref).*)
                        });
                    }

                }
                tokens.extend(quote! {
                    let matches = #(#table_name).*.match_selector(
                        &[#(#selector_components),*]
                    );
                    if matches.len() > 0 { 
                        (matches[0].action)(#(#action_args),*)
                    }
                })
            }
            x => todo!("control apply statement {:?}", x),
        }
    }

    tokens
}

fn control_parameters(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut params = Vec::new();
    let mut types = Vec::new();

    for arg in &control.parameters {
        // if the type is user defined, check to ensure it's defined
        match arg.ty {
            Type::UserDefined(ref typename) => {
                match ast.get_user_defined_type(typename) {
                    Some(_udt) => {
                        let name = format_ident!("{}", arg.name);
                        let ty = rust_type(&arg.ty, false, 0);
                        let lifetime = type_lifetime(ast, &arg.ty);
                        match &arg.direction {
                            Direction::Out | Direction::InOut => {
                                params
                                    .push(quote! { #name: &mut #ty #lifetime });
                                types.push(quote! { &mut #ty #lifetime });
                            }
                            _ => {
                                params.push(quote! { #name: &#ty #lifetime });
                                types.push(quote! { &#ty #lifetime });
                            }
                        }
                    }
                    None => {
                        ctx.diags.push(Diagnostic {
                            level: Level::Error,
                            message: format!("Undefined type {}", typename),
                            token: arg.ty_token.clone(),
                        });
                        return (params, types);
                    }
                }
            }
            _ => {
                let name = format_ident!("{}", arg.name);
                let ty = rust_type(&arg.ty, false, 0);
                params.push(quote! { #name: &#ty });
            }
        }
    }

    (params, types)
}

fn generate_control_action(
    ast: &AST,
    control: &Control,
    action: &Action,
    ctx: &mut Context,
) {
    let name = format_ident!("{}_action_{}", control.name, action.name);
    let (mut params, _) = control_parameters(ast, control, ctx);

    for arg in &action.parameters {
        // if the type is user defined, check to ensure it's defined
        if let Type::UserDefined(ref typename) = arg.ty {
            match ast.get_user_defined_type(typename) {
                Some(_) => {
                    let name = format_ident!("{}", arg.name);
                    let ty = rust_type(&arg.ty, false, 0);
                    params.push(quote! { #name: #ty });
                }
                None => {
                    ctx.diags.push(Diagnostic {
                        level: Level::Error,
                        message: format!("Undefined type {}", typename),
                        token: arg.ty_token.clone(),
                    });
                    return;
                }
            }
        } else {
            let name = format_ident!("{}", arg.name);
            let ty = rust_type(&arg.ty, false, 0);
            params.push(quote! { #name: #ty });
        }
    }

    let body = generate_control_action_body(ast, control, action, ctx);

    ctx.functions.insert(
        name.to_string(),
        quote! {
            fn #name<'a>(#(#params),*) {
                #body
            }
        },
    );
}

fn generate_control_table(
    ast: &AST,
    control: &Control,
    table: &Table,
    control_param_types: &Vec<TokenStream>,
    ctx: &mut Context,
) -> (TokenStream, TokenStream) {
    let mut key_type_tokens: Vec<TokenStream> = Vec::new();
    let mut key_types: Vec<Type> = Vec::new();
    for (k, _) in &table.key {
        let parts: Vec<&str> = k.name.split(".").collect();
        let root = parts[0];

        // try to find the root of the key as an argument to the control block.
        // TODO: are there other places to look for this?
        match get_control_arg(control, root) {
            Some(param) => {
                if parts.len() > 1 {
                    match check_lvalue_chain(
                        &k,
                        &parts[1..],
                        &param.ty,
                        ast,
                        ctx,
                    ) {
                        Ok(ty) => {
                            key_types.push(ty[0].clone());
                            key_type_tokens.push(rust_type(&ty[0], false, 0));
                        }
                        Err(_) => {
                            // diagnostics have been added to context so just
                            // bail with an empty result
                            return (quote! {}, quote! {});
                        }
                    }
                }
            }
            None => {
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!("Table key '{}' undefined", root),
                    token: k.token.clone(),
                });
                return (quote! {}, quote! {});
            }
        }
    }

    let table_name = format_ident!("{}_table", table.name);
    /*
    let key_type = quote! { (#(#key_type_tokens),*) };
    let table_type = quote! {
        std::collections::HashMap::<#key_type, &'a dyn Fn(#(#control_param_types),*)>
    };
    */
    let n = table.key.len() as usize;
    let table_type = quote! { p4rs::table::Table::<#n, fn(#(#control_param_types),*)> };

    /*
    let mut tokens = quote! {
        let mut #table_name: #table_type = std::collections::HashMap::new();
    };
    */
    let mut tokens = quote! {
        let mut #table_name: #table_type = #table_type::new();
    };

    if table.const_entries.is_empty() {
        tokens.extend(quote! { #table_name });
        return (table_type, tokens);
    }

    for entry in &table.const_entries {
        let mut keyset = Vec::new();
        for (i, k) in entry.keyset.iter().enumerate() {
            match &k.value {
                KeySetElementValue::Expression(e) => {
                    let xpr = generate_expression(e.clone(), ctx);
                    let ks = match table.key[i].1 {
                        MatchKind::Exact => {
                            let k = format_ident!("{}", "Exact");
                            quote!{
                                p4rs::table::Key::#k(p4rs::bitvec_to_biguint(&#xpr))
                            }
                        }
                        MatchKind::Ternary => {
                            let k = format_ident!("{}", "Ternary");
                            quote!{
                                p4rs::table::Key::#k(p4rs::bitvec_to_biguint(&#xpr))
                            }
                        }
                        MatchKind::LongestPrefixMatch => {
                            let len = match try_extract_prefix_len(e) {
                                Some(len) => len,
                                None => {
                                    ctx.diags.push(Diagnostic {
                                        level: Level::Error,
                                        message: format!(
                                            "coult not determine prefix len for key",
                                        ),
                                        token: k.token.clone(),
                                    });
                                    return (quote! {}, quote! {});
                                }
                            };
                            let k = format_ident!("{}", "Lpm");
                            quote!{
                                p4rs::table::Key::#k(p4rs::table::Prefix{
                                    addr: bitvec_to_ip6addr(&(#xpr)),
                                    len: #len,
                                })
                            }
                        }
                    };
                    keyset.push(ks);
               },
                x => todo!("key set element {:?}", x),
            }
        }

        let action = match control.get_action(&entry.action.name) {
            Some(action) => action,
            None => {
                ctx.diags.push(Diagnostic {
                    level: Level::Error,
                    message: format!("action {} not found", entry.action.name),
                    token: entry.action.token.clone(),
                });
                return (quote! {}, quote! {});
            }
        };

        let mut action_fn_args = Vec::new();
        for arg in &control.parameters {
            let a = format_ident!("{}", arg.name);
            action_fn_args.push(quote! { #a });
        }

        let action_fn_name =
            format_ident!("{}_action_{}", control.name, entry.action.name);
        for (i, expr) in entry.action.parameters.iter().enumerate() {
            match expr.as_ref() {
                Expression::IntegerLit(v) => {
                    //XXX let tytk = rust_type(&action.parameters[i].ty, false, 0);
                    match &action.parameters[i].ty {
                        Type::Bit(n) => {
                            if *n <= 8 {
                                let v = *v as u8;
                                //XXX action_fn_args.push(quote! { #tytk::from(#v) });
                                action_fn_args.push(quote!{
                                    #v.view_bits::<Lsb0>().to_bitvec()
                                });
                            }
                        }
                        x => todo!("action praam expression type {:?}", x),
                    }
                }
                x => todo!("action parameter type {:?}", x),
            }
        }

        let mut closure_params = Vec::new();
        for x in &control.parameters {
            let name = format_ident!("{}", x.name);
            closure_params.push(quote! { #name });
        }

        /*
        tokens.extend(quote! {
            #table_name.insert((#(#keyset),*), &|#(#closure_params),*|{
                #action_fn_name(#(#action_fn_args),*);
            });
        });
        */
        tokens.extend(quote! {
            #table_name.entries.insert(p4rs::table::TableEntry::<#n, fn(#(#control_param_types),*)>{
                key: [#(#keyset),*],
                priority: 0,
                name: "your name here".into(),
                action: |#(#closure_params),*| {
                    #action_fn_name(#(#action_fn_args),*);
                },
            });
        })
    }

    tokens.extend(quote! { #table_name });

    (table_type, tokens)
}

fn generate_control_action_body(
    ast: &AST,
    control: &Control,
    action: &Action,
    ctx: &mut Context,
) -> TokenStream {
    let mut ts = TokenStream::new();

    for statement in &action.statement_block.statements {
        match statement {
            Statement::Empty => {
                continue;
            }
            Statement::Assignment(lval, expr) => {
                // check the lhs
                let parts: Vec<&str> = lval.name.split(".").collect();
                let root = parts[0];
                match get_control_arg(control, root) {
                    Some(param) => {
                        if parts.len() > 1 {
                            match check_lvalue_chain(
                                &lval,
                                &parts[1..],
                                &param.ty,
                                ast,
                                ctx,
                            ) {
                                Ok(_) => {}
                                Err(_) => return quote! {},
                            }
                        }
                    }
                    None => match get_action_arg(action, root) {
                        Some(_param) => {}
                        None => {
                            todo!();
                        }
                    },
                };
                let lhs: Vec<TokenStream> = parts
                    .iter()
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote! { #x })
                    .collect();

                // check the rhs
                let rhs = match expr.as_ref() {
                    Expression::Lvalue(rhs_lval) => {
                        let parts: Vec<&str> =
                            rhs_lval.name.split(".").collect();
                        let root = parts[0];
                        match get_control_arg(control, root) {
                            Some(param) => {
                                if parts.len() > 1 {
                                    match check_lvalue_chain(
                                        &lval,
                                        &parts[1..],
                                        &param.ty,
                                        ast,
                                        ctx,
                                    ) {
                                        Ok(_) => {}
                                        Err(_) => return quote! {},
                                    }
                                }
                                &rhs_lval.name
                            }
                            None => match get_action_arg(action, root) {
                                Some(_) => &rhs_lval.name,
                                None => {
                                    todo!();
                                }
                            },
                        }
                    }
                    x => {
                        todo!("action assignment rhs {:?}", x);
                    }
                };
                let rhs: Vec<TokenStream> = rhs
                    .split(".")
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote! { #x })
                    .collect();

                ts.extend(quote! { #(#lhs).* = #(#rhs ).* });
            }
            Statement::Call(_) => {
                todo!("handle control action function/method calls");
            }
        }
    }

    ts
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
            //XXX let off = offset % 8;
            if header_member {
                //XXX quote! { bit_slice::<'a, #size, #off> }
                quote!{ BitSlice<u8, Lsb0> }
            } else {
                quote!{ BitVec<u8, Lsb0> }
                //XXX quote! { bit::<#size> }
            }
        }
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        Type::UserDefined(name) => {
            let typename = format_ident!("{}", name);
            quote! { #typename }
        }
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
    }
}

fn generate_expression(
    expr: Box<Expression>,
    ctx: &mut Context,
) -> TokenStream {

    match expr.as_ref() {
        Expression::IntegerLit(v) => {
            quote!{ #v.into() }
        }
        Expression::BitLit(width, v) => {
            generate_bit_literal(*width, *v)
        }
        Expression::SignedLit(_width, _v) => {
            todo!("generate expression signed lit");
        }
        Expression::Lvalue(_v) => {
            todo!("generate expression lvalue");
        }
        Expression::Binary(lhs, op, rhs) => {
            let mut ts = TokenStream::new();
            ts.extend(generate_expression(lhs.clone(), ctx));
            ts.extend(generate_binop(*op, ctx));
            ts.extend(generate_expression(rhs.clone(), ctx));
            ts
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
