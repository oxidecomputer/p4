use std::collections::HashMap;
use std::fs;
use std::io;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    AST, Direction, Type, Struct, Header, Parser, State, Transition,
    Statement, Lvalue, ControlParameter, Expression, Control, Action,
    ActionParameter, Table, KeySetElement,
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
    handle_structs(ast, &mut ctx);
    handle_headers(ast, &mut ctx);
    handle_parsers(ast, &mut ctx);
    handle_control_blocks(ast, &mut ctx);


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
        let typename = rust_type(&arg.ty, false);
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
        fn #function_name<'a>(#(#args),*) -> bool {
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
                    &call.lval,
                    ctx
                ) {
                    Ok(_) => {
                        let lval: Vec<TokenStream> = call.lval.name
                            .split(".")
                            .map(|x| format_ident!("{}", x))
                            .map(|x| quote!{ #x })
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
                                                Direction::Out | Direction::InOut => {
                                                    mut_arg = true;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    let lvref: Vec<TokenStream> = parts.iter()
                                        .map(|x| format_ident!("{}", x))
                                        .map(|x| quote!{ #x })
                                        .collect();
                                    if mut_arg {
                                        args.push(quote! { &mut #(#lvref).* });
                                    } else {
                                        args.push(quote! { #(#lvref).* });
                                    }
                                }
                                x => todo!("extern arg {:?}", x)
                            }
                        }
                        return quote! {
                            #(#lval).* ( #(#args),* );
                        };
                    },
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
) -> Result<(),()> {

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
) -> Result<Type,()> {
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
            return Ok(ty.clone())
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
            return Ok(ty.clone())
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
            return Ok(ty.clone())
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
            return Ok(ty.clone())
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
            return Ok(ty.clone())
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
            return Ok(ty.clone())
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
                        } else {
                            return Ok(member.ty.clone())
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
                        } else {
                            return Ok(member.ty.clone())
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
                        } else {
                            return Ok(ty.clone()) //TODO function/method type?
                        }
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
                return Err(());
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

fn get_control_arg<'a>(
    control: &'a Control,
    arg_name: &str,
) -> Option<&'a ControlParameter> {
    for arg in &control.parameters {
        if arg.name == arg_name {
            return Some(arg)
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
            return Some(arg)
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
                if let Some(_decl) = ast.get_struct(typename) { }
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
                    members.push(quote!{ #name: #ty::<'a> });
                    needs_lifetime = true;
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
            Type::Bit(size) => {
                members.push(quote!{ #name: bit::<#size> });
            }
            x => {
                todo!("struct member {}", x)
            }
        }
    }

    let name = format_ident!("{}", s.name);

    let lifetime = if needs_lifetime {
        quote!{ <'a> }
    } else {
        quote!{}
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
    for member in &h.members {
        let name = format_ident!("{}", member.name);
        let ty = rust_type(&member.ty, true);
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
        let ty = rust_type(&member.ty, true);
        member_values.push(quote! {
            //#name: #ty::new(&mut buf[#offset..#end])?
            #name: unsafe {
                #ty::new(&mut*std::ptr::slice_from_raw_parts_mut(
                    buf.as_mut_ptr().add(#offset), #end))? 
            }
        });
        set_statements.push(quote! {
            //self.#name = #ty::new(&mut buf[#offset..#end])?
            self.#name = unsafe {
                #ty::new(&mut*std::ptr::slice_from_raw_parts_mut(
                    buf.as_mut_ptr().add(#offset), #end))? 
            }
        });
        offset += required_bytes;
    }

    generated.extend(quote! {
        impl<'a> Header<'a> for #name<'a> {
            fn new(buf: &'a mut [u8]) -> Result<Self, TryFromSliceError> {
                Ok(Self {
                    #(#member_values),*
                })
            }
            fn set(&mut self, buf: &'a mut [u8]) -> Result<(), TryFromSliceError> {
                #(#set_statements);*;
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
        let mut params = control_parameters(ast, control, ctx);

        for action in &control.actions {
            generate_control_action(ast, control, action, ctx);
        }
        for table in &control.tables {
            let (type_tokens, table_tokens) =
                generate_control_table(ast, control, table, ctx);
            let name = format_ident!("{}_table_{}", control.name, table.name);
            ctx.functions.insert(name.to_string(), quote!{
                fn #name<'a>(#(#params),*) -> #type_tokens {
                    #table_tokens
                }
            });
            let name = format_ident!("table_{}", table.name);
            params.push(quote!{
                #name: &#type_tokens
            });
        }

        let name = format_ident!("{}_apply", control.name);

        ctx.functions.insert(name.to_string(), quote!{
            fn #name<'a>(#(#params),*) {
            }
        });
    }

}

fn control_parameters(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
) -> Vec<TokenStream> {
    let mut params = Vec::new();

    for arg in &control.parameters {
        // if the type is user defined, check to ensure it's defined
        match arg.ty {
            Type::UserDefined(ref typename) => {
                match ast.get_user_defined_type(typename) {
                    Some(_udt) => {
                        let name = format_ident!("{}", arg.name);
                        let ty = rust_type(&arg.ty, false);
                        let lifetime = type_lifetime(ast, &arg.ty);
                        match &arg.direction {
                            Direction::Out | Direction::InOut => {
                                params.push(quote!{ #name: &mut #ty #lifetime });
                            }
                            _ => {
                                params.push(quote!{ #name: &#ty #lifetime });
                            }
                        }
                    }
                    None => {
                        ctx.diags.push(Diagnostic{
                            level: Level::Error,
                            message: format!("Undefined type {}", typename),
                            token: arg.ty_token.clone(),
                        });
                        return params;
                    }
                }
            }
            _ => {
                let name = format_ident!("{}", arg.name);
                let ty = rust_type(&arg.ty, false);
                params.push(quote!{ #name: &#ty });
            }
        }
    }

    params
}

fn generate_control_action(
    ast: &AST,
    control: &Control,
    action: &Action,
    ctx: &mut Context,
) {

    let name = format_ident!("{}_action_{}", control.name, action.name);
    let mut params = control_parameters(ast, control, ctx);

    for arg in &action.parameters {
        // if the type is user defined, check to ensure it's defined
        if let Type::UserDefined(ref typename) = arg.ty {
            match ast.get_user_defined_type(typename) {
                Some(_) => {
                    let name = format_ident!("{}", arg.name);
                    let ty = rust_type(&arg.ty, false);
                    params.push(quote!{ #name: #ty });
                }
                None => {
                    ctx.diags.push(Diagnostic{
                        level: Level::Error,
                        message: format!("Undefined type {}", typename),
                        token: arg.ty_token.clone(),
                    });
                    return;
                }
            }
        } else {
            let name = format_ident!("{}", arg.name);
            let ty = rust_type(&arg.ty, false);
            params.push(quote!{ #name: #ty });
        }
    }

    let body = generate_control_action_body(ast, control, action, ctx);

    ctx.functions.insert(name.to_string(), quote!{
        fn #name<'a>(#(#params),*) {
            #body
        }
    });

}

fn generate_control_table(
    ast: &AST,
    control: &Control,
    table: &Table,
    ctx: &mut Context,
) -> (TokenStream, TokenStream) {

    let mut key_type_tokens: Vec<TokenStream> = Vec::new();
    let mut key_types: Vec<Type> = Vec::new();
    for k in table.key.keys() {

        let parts: Vec<&str> = k.name.split(".").collect();
        let root = parts[0];

        match get_control_arg(control, root) {
            Some(param) => {
                if parts.len() > 1 {
                    match check_lvalue_chain(
                        &k, &parts[1..], &param.ty, ast, ctx) {
                        Ok(ty) => {
                            key_types.push(ty.clone());
                            key_type_tokens.push(rust_type(&ty, false));
                        }
                        Err(_) => {
                            //TODO diagnostics
                            return (quote!{}, quote!{})
                        }
                    }
                }
            }
            None => { 
                todo!();
            }
        }
    }

    let table_name = format_ident!("{}_table", table.name);
    let key_type = quote!{ (#(#key_type_tokens),*) };
    let table_type = quote!{ std::collections::HashMap::<#key_type, &'static dyn Fn()> };

    let mut tokens = quote!{
        let mut #table_name: #table_type = std::collections::HashMap::new();
    };

    if table.const_entries.is_empty() {
        tokens.extend(quote!{ #table_name });
        return (table_type, tokens);
    }

    for entry in &table.const_entries {

        let mut keyset = Vec::new();
        for (i, k) in entry.keyset.iter().enumerate() {
            match k {
                KeySetElement::Expression(e) => {
                    match e.as_ref() {
                        Expression::IntegerLit(v) => {
                            let tytk = &key_type_tokens[i];
                            match &key_types[i] {
                                Type::Bit(n) => {
                                    if *n <= 8 {
                                        let v = *v as u8;
                                        keyset.push(quote!{ #tytk::from(#v) });
                                    }
                                }
                                x => todo!("keyset expression type {:?}", x),
                            }
                        }
                        x => todo!("const entry keyset expression {:?}", x),
                    }
                }
                x => todo!("key set element {:?}", x),
            }
        }

        tokens.extend(quote!{
            #table_name.insert((#(#keyset),*), &||{2+2;});
        });
        
    }

    tokens.extend(quote!{ #table_name });

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
                                &lval, &parts[1..], &param.ty, ast, ctx) {
                                Ok(_) => {}
                                Err(_) => return quote!{},
                            }
                        }
                    }
                    None => {
                        match get_action_arg(action, root) {
                            Some(_param) => { }
                            None => {
                                todo!();
                            }
                        }
                    }
                };
                let lhs: Vec<TokenStream> = parts.iter()
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote!{ #x })
                    .collect();

                // check the rhs
                let rhs = match expr.as_ref() {
                    Expression::Lvalue(rhs_lval) => {
                        let parts: Vec<&str> = rhs_lval.name.split(".").collect();
                        let root = parts[0];
                        match get_control_arg(control, root) {
                            Some(param) => {
                                if parts.len() > 1 {
                                    match check_lvalue_chain(
                                        &lval, &parts[1..], &param.ty, ast, ctx) {
                                        Ok(_) => {}
                                        Err(_) => return quote!{},
                                    }
                                }
                                &rhs_lval.name
                            }
                            None => {
                                match get_action_arg(action, root) {
                                    Some(_) => &rhs_lval.name,
                                    None => {
                                        todo!();
                                    }
                                }
                            }
                        }
                    }
                    x => {
                        todo!("action assignment rhs {:?}", x);
                    }
                };
                let rhs: Vec<TokenStream> = rhs
                    .split(".")
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote!{ #x })
                    .collect();

                ts.extend(quote!{ #(#lhs).* = #(#rhs ).* });
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
fn rust_type(ty: &Type, header_member: bool) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(size) => {
            if header_member {
                quote! { bit_slice::<'a, #size> }
            } else {
                quote! { bit::<#size> }
            }
        }
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

fn type_lifetime(ast: &AST, ty: &Type) -> TokenStream {
    if requires_lifetime(ast, ty) {
        quote!{<'a>}
    } else {
        quote!{}
    }
}

fn requires_lifetime(ast: &AST, ty: &Type) -> bool {
    match ty {
        Type::Bool | Type::Error | Type::Bit(_) | Type::Int(_) |
        Type::Varbit(_) | Type::String => {
            return false;
        },
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
