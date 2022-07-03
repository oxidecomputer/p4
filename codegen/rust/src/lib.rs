use std::collections::HashMap;
use std::fs;
use std::io::{ self, Write };

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{
    Action, AST, ActionParameter, BinOp, Call, Control, ControlParameter,
    Direction, Expression, Header, IfBlock, KeySetElementValue, Lvalue,
    MatchKind, Parser, State, Statement, Struct, Table, Transition, Type
};

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
    handle_structs(ast, &mut ctx);
    handle_headers(ast, &mut ctx);
    handle_parsers(ast, &mut ctx);
    handle_control_blocks(ast, &mut ctx);

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
    _ast: &AST,
    parser: &Parser,
    state: &State,
    _ctx: &mut Context,
) -> TokenStream {
    let mut tokens = TokenStream::new();

    for stmt in &state.statements {
        match stmt {
            Statement::Empty => continue,
            Statement::Assignment(_lvalue, _expr) => {
                todo!("parser state assignment statement");
            }
            Statement::Call(call) => {
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
            x => todo!("codegen: statement {:?}", x),
        }
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
                    panic!("Struct member {:#?} undefined in {:#?}", member, s);
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
        member_values.push(quote! {
            #name: None
        });
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
            fn set(
                &mut self,
                buf: &'a mut [u8]
            ) -> Result<(), TryFromSliceError> {
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

fn generate_control_apply_body_call(
    ast: &AST,
    control: &Control,
    _ctx: &mut Context,
    c: &Call,
    tokens: &mut TokenStream,
) {
    // TODO only supporting tably apply calls for now
    
    //
    // get the referenced table
    //
    let parts: Vec<&str> = c.lval.name.split(".").collect();
    if parts.len() != 2 || parts[1] != "apply" {
        panic!(
            "codegen: only <tablename>.apply() calls are 
             supported in apply blocks right now: {:#?}",
             c
        );
    }
    let table = match control.get_table(parts[0]) {
        Some(table) => table,
        None => {
            panic!(
                "codegen: table {} not found in control {}",
                parts[0],
                control.name,
            );
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

        if is_header(lval, ast) {
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
    });
}

fn generate_control_apply_body_if_block(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
    if_block: &IfBlock,
    tokens: &mut TokenStream,
) {

    let xpr = generate_expression(if_block.predicate.clone(), ctx);

    let mut stmt_tokens = TokenStream::new();
    for s in &if_block.block.statements {
        generate_control_apply_stmt(ast, control, ctx, s, &mut stmt_tokens)
    };

    // TODO statement block variables

    tokens.extend(quote! {
        if #xpr {
            #stmt_tokens
        }
    });

}

fn generate_control_apply_stmt(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
    stmt: &Statement,
    tokens: &mut TokenStream,
) {
    match stmt {
        Statement::Call(c) => {
            generate_control_apply_body_call(
                ast,
                control,
                ctx,
                c,
                tokens
            );
        }
        Statement::If(if_block) => {
            generate_control_apply_body_if_block(
                ast,
                control,
                ctx,
                if_block,
                tokens
            );
        }
        Statement::Assignment(lval, xpr) => {
            let lhs = generate_lvalue(lval);
            let rhs = generate_expression(xpr.clone(), ctx);
            tokens.extend(quote!{ #lhs = #rhs; });
        }
        x => todo!("control apply statement {:?}", x),
    }
}

fn generate_control_apply_body(
    ast: &AST,
    control: &Control,
    ctx: &mut Context,
) -> TokenStream {
    let mut tokens = TokenStream::new();

    for stmt in &control.apply.statements {
        generate_control_apply_stmt(ast, control, ctx, stmt, &mut tokens);
    }

    tokens
}

fn control_parameters(
    ast: &AST,
    control: &Control,
    _ctx: &mut Context,
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
                        // if this is a generic type, skip for now
                        if control.is_type_parameter(typename) {
                            continue;
                        }
                        panic!("Undefined type {}", typename);
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
                    panic!(
                        "codegen: undefined type {} for arg {:#?}",
                        typename,
                        arg,
                    );
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
            Some(_param) => {
                if parts.len() > 1 {
                    let tm = control.names();
                    let ty = lvalue_type(&k, ast, &tm);
                    key_types.push(ty.clone());
                    key_type_tokens.push(rust_type(&ty, false, 0));
                }
            }
            None => {
                panic!("bug: control arg undefined {:#?}", root)
            }
        }
    }

    let table_name = format_ident!("{}_table", table.name);
    let n = table.key.len() as usize;
    let table_type = quote! {
        p4rs::table::Table::<#n, fn(#(#control_param_types),*)> 
    };

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
                                p4rs::table::Key::#k(
                                    p4rs::bitvec_to_biguint(&#xpr))
                            }
                        }
                        MatchKind::Ternary => {
                            let k = format_ident!("{}", "Ternary");
                            quote!{
                                p4rs::table::Key::#k(
                                    p4rs::bitvec_to_biguint(&#xpr))
                            }
                        }
                        MatchKind::LongestPrefixMatch => {
                            let len = match try_extract_prefix_len(e) {
                                Some(len) => len,
                                None => {
                                    panic!(
                                        "codegen: coult not determine prefix 
                                        len for key {:#?}",
                                        table.key[i].1,
                                    );
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
                panic!("codegen: action {} not found", entry.action.name);
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
                    match &action.parameters[i].ty {
                        Type::Bit(n) => {
                            if *n <= 8 {
                                let v = *v as u8;
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

        tokens.extend(quote! {
            #table_name.entries.insert(
                p4rs::table::TableEntry::<#n, fn(#(#control_param_types),*)>{
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
    _ast: &AST,
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
                let lhs: Vec<TokenStream> = lval.parts()
                    .iter()
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote! { #x })
                    .collect();

                let rhs = match expr.as_ref() {
                    Expression::Lvalue(rhs_lval) => {
                        let parts: Vec<&str> =
                            rhs_lval.name.split(".").collect();
                        let root = parts[0];
                        match get_control_arg(control, root) {
                            Some(_param) => {
                                let rhs: Vec<TokenStream> = rhs_lval.name
                                    .split(".")
                                    .map(|x| format_ident!("{}", x))
                                    .map(|x| quote! { #x })
                                    .collect();
                                quote!{ #(#rhs ).* }
                            }
                            None => match get_action_arg(action, root) {
                                Some(_) => {
                                    let rhs: Vec<TokenStream> = rhs_lval.name
                                        .split(".")
                                        .map(|x| format_ident!("{}", x))
                                        .map(|x| quote! { #x })
                                        .collect();
                                    quote!{ #(#rhs ).* }
                                }
                                None => {
                                    panic!(
                                        "codegen: arg {} not found for action
                                        {:#?}",
                                        root,
                                        action,
                                    );
                                }
                            },
                        }
                    }
                    // otherwise just run the expression generator
                    x => {
                       generate_expression(Box::new(x.clone()), ctx)
                    }
                };

                ts.extend(quote! { #(#lhs).* = #rhs });
            }
            Statement::Call(_) => {
                todo!("handle control action function/method calls");
            }
            x => todo!("codegen: statement {:?}", x),
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
) -> bool {
    let parts = lval.parts();
    if parts.len() < 2 {
        return false;
    }
    match ast.get_header(parts[parts.len()-2]) {
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
        None => panic!("checker bug: unresolved lval {:#?}", lval),
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
            if let Some(parent) = ast.get_struct(name) {
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
