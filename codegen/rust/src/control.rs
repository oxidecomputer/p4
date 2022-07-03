use crate::{
    Context,
    rust_type,
    type_lifetime,
    generate_expression,
    lvalue_type,
    try_extract_prefix_len,
    generate_lvalue,
    is_header,
};
use p4::ast::{
    Action, ActionParameter, AST, Call, Control, ControlParameter, Direction,
    Expression, IfBlock, KeySetElementValue, MatchKind, Statement, Table, Type
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct ControlGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
}

impl<'a> ControlGenerator<'a> {
    pub(crate) fn new(ast: &'a AST, ctx: &'a mut Context) -> Self {
        Self{ ast, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for control in &self.ast.controls {
            let (mut params, param_types) = self.control_parameters(control);

            for action in &control.actions {
                self.generate_control_action(control, action);
            }
            for table in &control.tables {
                let (type_tokens, table_tokens) =
                    self.generate_control_table(control, table, &param_types);
                let name = format_ident!("{}_table_{}", control.name, table.name);
                self.ctx.functions.insert(
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
            let apply_body = self.generate_control_apply_body(control);
            self.ctx.functions.insert(
                name.to_string(),
                quote! {
                    pub fn #name<'a>(#(#params),*) {
                        #apply_body
                    }
                },
            );
        }
    }

    fn control_parameters(
        &mut self,
        control: &Control,
    ) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut params = Vec::new();
        let mut types = Vec::new();

        for arg in &control.parameters {
            // if the type is user defined, check to ensure it's defined
            match arg.ty {
                Type::UserDefined(ref typename) => {
                    match self.ast.get_user_defined_type(typename) {
                        Some(_udt) => {
                            let name = format_ident!("{}", arg.name);
                            let ty = rust_type(&arg.ty, false, 0);
                            let lifetime = type_lifetime(self.ast, &arg.ty);
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
        &mut self,
        control: &Control,
        action: &Action,
    ) {
        let name = format_ident!("{}_action_{}", control.name, action.name);
        let (mut params, _) = self.control_parameters(control);

        for arg in &action.parameters {
            // if the type is user defined, check to ensure it's defined
            if let Type::UserDefined(ref typename) = arg.ty {
                match self.ast.get_user_defined_type(typename) {
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

        let body = self.generate_control_action_body(control, action);

        self.ctx.functions.insert(
            name.to_string(),
            quote! {
                fn #name<'a>(#(#params),*) {
                    #body
                }
            },
        );
    }

    fn generate_control_table(
        &mut self,
        control: &Control,
        table: &Table,
        control_param_types: &Vec<TokenStream>,
    ) -> (TokenStream, TokenStream) {

        let mut key_type_tokens: Vec<TokenStream> = Vec::new();
        let mut key_types: Vec<Type> = Vec::new();

        for (k, _) in &table.key {
            let parts: Vec<&str> = k.name.split(".").collect();
            let root = parts[0];

            // try to find the root of the key as an argument to the control block.
            // TODO: are there other places to look for this?
            match Self::get_control_arg(control, root) {
                Some(_param) => {
                    if parts.len() > 1 {
                        let tm = control.names();
                        let ty = lvalue_type(&k, self.ast, &tm);
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
                        let xpr = generate_expression(e.clone(), self.ctx);
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
        &mut self,
        control: &Control,
        action: &Action,
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
                            match Self::get_control_arg(control, root) {
                                Some(_param) => {
                                    let rhs: Vec<TokenStream> = rhs_lval.name
                                        .split(".")
                                        .map(|x| format_ident!("{}", x))
                                        .map(|x| quote! { #x })
                                        .collect();
                                    quote!{ #(#rhs ).* }
                                }
                                None => match Self::get_action_arg(action, root) {
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
                            generate_expression(Box::new(x.clone()), self.ctx)
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

    fn generate_control_apply_body_call(
        &mut self,
        control: &Control,
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

            let mut names = self.ast.names();
            names.extend(control.names());

            if is_header(&lval.pop_right(), self.ast, &names) {
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
        &mut self,
        control: &Control,
        if_block: &IfBlock,
        tokens: &mut TokenStream,
    ) {

        let xpr = generate_expression(if_block.predicate.clone(), self.ctx);

        let mut stmt_tokens = TokenStream::new();
        for s in &if_block.block.statements {
            self.generate_control_apply_stmt(control, s, &mut stmt_tokens)
        };

        // TODO statement block variables

        tokens.extend(quote! {
            if #xpr {
                #stmt_tokens
            }
        });

    }

    fn generate_control_apply_stmt(
        &mut self,
        control: &Control,
        stmt: &Statement,
        tokens: &mut TokenStream,
    ) {
        match stmt {
            Statement::Call(c) => {
                self.generate_control_apply_body_call(
                    control,
                    c,
                    tokens
                );
            }
            Statement::If(if_block) => {
                self.generate_control_apply_body_if_block(
                    control,
                    if_block,
                    tokens
                );
            }
            Statement::Assignment(lval, xpr) => {
                let lhs = generate_lvalue(lval);
                let rhs = generate_expression(xpr.clone(), self.ctx);
                tokens.extend(quote!{ #lhs = #rhs; });
            }
            x => todo!("control apply statement {:?}", x),
        }
    }

    fn generate_control_apply_body(
        &mut self,
        control: &Control,
    ) -> TokenStream {
        let mut tokens = TokenStream::new();

        for stmt in &control.apply.statements {
            self.generate_control_apply_stmt(control, stmt, &mut tokens);
        }

        tokens
    }

    fn get_control_arg<'b>(
        control: &'b Control,
        arg_name: &str,
    ) -> Option<&'b ControlParameter> {
        for arg in &control.parameters {
            if arg.name == arg_name {
                return Some(arg);
            }
        }
        None
    }

    fn get_action_arg<'b>(
        action: &'b Action,
        arg_name: &str,
    ) -> Option<&'b ActionParameter> {
        for arg in &action.parameters {
            if arg.name == arg_name {
                return Some(arg);
            }
        }
        None
    }
}
