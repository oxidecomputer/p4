// Copyright 2022 Oxide Computer Company

use crate::{
    expression::ExpressionGenerator,
    qualified_table_function_name, rust_type,
    statement::{StatementContext, StatementGenerator},
    try_extract_prefix_len, Context,
};
use p4::ast::{
    Action, Control, ControlParameter, Direction, ExpressionKind,
    KeySetElementValue, MatchKind, Statement, Table, Type, AST,
};
use p4::hlir::Hlir;
use p4::util::resolve_lvalue;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct ControlGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
    hlir: &'a Hlir,
}

impl<'a> ControlGenerator<'a> {
    pub(crate) fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        ctx: &'a mut Context,
    ) -> Self {
        Self { ast, hlir, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for control in &self.ast.controls {
            self.generate_control(control);
        }

        let ingress = match self.ast.get_control("ingress") {
            Some(i) => i,
            None => return,
        };
        let tables = ingress.tables(self.ast);
        for (cs, t) in tables {
            let qtn = qualified_table_function_name(&cs, t);
            let qtfn = qualified_table_function_name(&cs, t);
            let control = cs.last().unwrap().1;
            let (_, mut param_types) = self.control_parameters(control);
            for var in &control.variables {
                if let Type::UserDefined(typename) = &var.ty {
                    if self.ast.get_extern(typename).is_some() {
                        let extern_type = format_ident!("{}", typename);
                        param_types.push(quote! {
                            &p4rs::externs::#extern_type
                        })
                    }
                }
            }
            let (type_tokens, table_tokens) =
                self.generate_control_table(control, t, &param_types);
            let qtn = format_ident!("{}", qtn);
            let qtfn = format_ident!("{}", qtfn);
            self.ctx.functions.insert(
                qtn.to_string(),
                quote! {
                    pub fn #qtfn() -> #type_tokens {
                        #table_tokens
                    }
                },
            );
        }
    }

    pub(crate) fn generate_control(
        &mut self,
        control: &Control,
    ) -> (TokenStream, TokenStream) {
        let (mut params, _param_types) = self.control_parameters(control);

        for action in &control.actions {
            if action.name == "NoAction" {
                continue;
            }
            self.generate_control_action(control, action);
        }

        let tables = control.tables(self.ast);
        for (cs, table) in tables {
            let c = cs.last().unwrap().1;
            let qtn = qualified_table_function_name(&cs, table);
            let (_, mut param_types) = self.control_parameters(c);
            for var in &c.variables {
                if let Type::UserDefined(typename) = &var.ty {
                    if self.ast.get_extern(typename).is_some() {
                        let extern_type = format_ident!("{}", typename);
                        param_types.push(quote! {
                            &p4rs::externs::#extern_type
                        })
                    }
                }
            }
            let n = table.key.len() as usize;
            let table_type = quote! {
                p4rs::table::Table::<
                    #n,
                    std::sync::Arc<dyn Fn(#(#param_types),*)>
                    >
            };
            let qtn = format_ident!("{}", qtn);
            params.push(quote! {
                #qtn: &#table_type
            });
        }

        let name = format_ident!("{}_apply", control.name);
        let apply_body = self.generate_control_apply_body(control);
        let sig = quote! {
            (#(#params),*)
        };
        self.ctx.functions.insert(
            name.to_string(),
            quote! {
                pub fn #name #sig {
                    #apply_body
                }
            },
        );

        (sig, apply_body)
    }

    pub(crate) fn control_parameters(
        &mut self,
        control: &Control,
    ) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut params = Vec::new();
        let mut types = Vec::new();

        for arg in &control.parameters {
            match arg.ty {
                Type::UserDefined(ref typename) => {
                    match self.ast.get_user_defined_type(typename) {
                        Some(_udt) => {
                            let name = format_ident!("{}", arg.name);
                            let ty = rust_type(&arg.ty);
                            match &arg.direction {
                                Direction::Out | Direction::InOut => {
                                    params.push(quote! {
                                        #name: &mut #ty
                                    });
                                    types.push(quote! { &mut #ty });
                                }
                                _ => {
                                    params.push(quote! {
                                        #name: &#ty
                                    });
                                    types.push(quote! { &#ty });
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
                    let ty = rust_type(&arg.ty);
                    match &arg.direction {
                        Direction::Out | Direction::InOut => {
                            params.push(quote! { #name: &mut #ty });
                            types.push(quote! { &mut #ty });
                        }
                        _ => {
                            params.push(quote! { #name: &#ty });
                            types.push(quote! { &#ty });
                        }
                    }
                }
            }
        }

        (params, types)
    }

    fn generate_control_action(&mut self, control: &Control, action: &Action) {
        let name = format_ident!("{}_action_{}", control.name, action.name);
        let (mut params, _) = self.control_parameters(control);

        for var in &control.variables {
            if let Type::UserDefined(typename) = &var.ty {
                if self.ast.get_extern(typename).is_some() {
                    let name = format_ident!("{}", var.name);
                    let extern_type = format_ident!("{}", typename);
                    params.push(quote! {
                        #name: &p4rs::externs::#extern_type
                    })
                }
            }
        }

        for arg in &action.parameters {
            // if the type is user defined, check to ensure it's defined
            if let Type::UserDefined(ref typename) = arg.ty {
                match self.ast.get_user_defined_type(typename) {
                    Some(_) => {
                        let name = format_ident!("{}", arg.name);
                        let ty = rust_type(&arg.ty);
                        params.push(quote! { #name: #ty });
                    }
                    None => {
                        panic!(
                            "codegen: undefined type {} for arg {:#?}",
                            typename, arg,
                        );
                    }
                }
            } else {
                let name = format_ident!("{}", arg.name);
                let ty = rust_type(&arg.ty);
                params.push(quote! { #name: #ty });
            }
        }

        let mut names = control.names();
        let sg = StatementGenerator::new(
            self.ast,
            self.hlir,
            StatementContext::Control(control),
        );
        let body = sg.generate_block(&action.statement_block, &mut names);

        let __name = name.to_string();

        self.ctx.functions.insert(
            name.to_string(),
            quote! {
                pub fn #name(#(#params),*) {

                    //TODO <<<< DTRACE <<<<<<
                    //Generate dtrace prbes that allow us to trace control
                    //action flows.
                    //println!("####{}####", #__name);

                    #body
                }
            },
        );
    }

    pub(crate) fn generate_control_table(
        &mut self,
        control: &Control,
        table: &Table,
        control_param_types: &Vec<TokenStream>,
    ) -> (TokenStream, TokenStream) {
        let mut key_type_tokens: Vec<TokenStream> = Vec::new();
        let mut key_types: Vec<Type> = Vec::new();

        for (k, _) in &table.key {
            let parts: Vec<&str> = k.name.split('.').collect();
            let root = parts[0];

            // try to find the root of the key as an argument to the control block.
            // TODO: are there other places to look for this?
            match Self::get_control_arg(control, root) {
                Some(_param) => {
                    if parts.len() > 1 {
                        let tm = control.names();
                        //TODO: use hlir?
                        let ty = resolve_lvalue(k, self.ast, &tm).unwrap().ty;
                        key_types.push(ty.clone());
                        key_type_tokens.push(rust_type(&ty));
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
            p4rs::table::Table::<
                #n,
                std::sync::Arc<dyn Fn(#(#control_param_types),*)>
            >
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
                        let eg = ExpressionGenerator::new(self.hlir);
                        let xpr = eg.generate_expression(e.as_ref());
                        let ks = match table.key[i].1 {
                            MatchKind::Exact => {
                                let k = format_ident!("{}", "Exact");
                                quote! {
                                    p4rs::table::Key::#k(
                                        p4rs::bitvec_to_biguint(&#xpr))
                                }
                            }
                            MatchKind::Ternary => {
                                let k = format_ident!("{}", "Ternary");
                                quote! {
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
                                quote! {
                                    p4rs::table::Key::#k(p4rs::table::Prefix{
                                        addr: bitvec_to_ip6addr(&(#xpr)),
                                        len: #len,
                                    })
                                }
                            }
                            MatchKind::Range => {
                                let k = format_ident!("Range");
                                quote! {
                                    p4rs::table::Key::#k(#xpr)
                                }
                            }
                        };
                        keyset.push(ks);
                    }
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
                match &expr.kind {
                    ExpressionKind::IntegerLit(v) => {
                        match &action.parameters[i].ty {
                            Type::Bit(n) => {
                                if *n <= 8 {
                                    let v = *v as u8;
                                    action_fn_args.push(quote! {
                                        #v.view_bits::<Msb0>().to_bitvec()
                                    });
                                }
                            }
                            x => {
                                todo!("action int lit expression type {:?}", x)
                            }
                        }
                    }
                    ExpressionKind::BitLit(width, v) => {
                        match &action.parameters[i].ty {
                            Type::Bit(n) => {
                                let n = *n as usize;
                                if n != *width as usize {
                                    panic!(
                                        "{:?} not compatible with {:?}",
                                        expr.kind, action.parameters[i],
                                    );
                                }
                                let size = n as usize;
                                action_fn_args.push(quote! {{
                                    let mut x = bitvec![mut u8, Msb0; 0; #size];
                                    x.store_be(#v);
                                    x
                                }});
                            }
                            x => {
                                todo!("action bit lit expression type {:?}", x)
                            }
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

                let action: std::sync::Arc<dyn Fn(#(#control_param_types),*)> =
                    std::sync::Arc::new(|#(#closure_params),*| {
                        #action_fn_name(#(#action_fn_args),*);
                    });

                #table_name.entries.insert(
                    p4rs::table::TableEntry::<
                        #n,
                        std::sync::Arc<dyn Fn(#(#control_param_types),*)>,
                    >{
                        key: [#(#keyset),*],
                        priority: 0,
                        name: "your name here".into(),
                        action,

                        //TODO actual data, does this actually matter for
                        //constant entries?
                        action_id: String::new(),
                        parameter_data: Vec::new(),
                    });
            })
        }

        tokens.extend(quote! { #table_name });

        (table_type, tokens)
    }

    fn generate_control_apply_stmt(
        &mut self,
        control: &Control,
        stmt: &Statement,
        tokens: &mut TokenStream,
    ) {
        let mut names = control.names();
        let sg = StatementGenerator::new(
            self.ast,
            self.hlir,
            StatementContext::Control(control),
        );
        tokens.extend(sg.generate_statement(stmt, &mut names));
    }

    fn generate_control_apply_body(
        &mut self,
        control: &Control,
    ) -> TokenStream {
        let mut tokens = TokenStream::new();

        for var in &control.variables {
            //TODO check in checker that externs are actually defined by
            //SoftNPU.
            if let Type::UserDefined(typename) = &var.ty {
                if self.ast.get_extern(typename).is_some() {
                    let name = format_ident!("{}", var.name);
                    let extern_type = format_ident!("{}", typename);
                    tokens.extend(quote! {
                        let #name = p4rs::externs::#extern_type::new();
                    })
                }
            }
        }

        for stmt in &control.apply.statements {
            self.generate_control_apply_stmt(control, stmt, &mut tokens);
        }
        tokens
    }

    fn get_control_arg<'b>(
        control: &'b Control,
        arg_name: &str,
    ) -> Option<&'b ControlParameter> {
        control.parameters.iter().find(|&arg| arg.name == arg_name)
    }
}
