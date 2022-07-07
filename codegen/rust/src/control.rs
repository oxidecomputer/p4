use crate::{
    Context,
    rust_type,
    type_lifetime,
    try_extract_prefix_len,
    statement::{StatementGenerator, StatementContext},
    expression::ExpressionGenerator,
};
use p4::ast::{
    Action, AST, Control, ControlParameter, Direction, ExpressionKind,
    KeySetElementValue, MatchKind, Statement, Table, Type
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
        ctx: &'a mut Context
    ) -> Self {
        Self{ ast, hlir, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for control in &self.ast.controls {
            let (mut params, param_types) = self.control_parameters(control);

            for action in &control.actions {
                self.generate_control_action(control, action);
            }

            // local tables
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

            // control instances as variables
            for v in &control.variables {
                if let Type::UserDefined(name) = &v.ty {
                    if let Some(control_inst) = self.ast.get_control(name) {
                        let (_, param_types) = self.control_parameters(control_inst);
                        for table in & control_inst.tables {
                            let n = table.key.len() as usize;
                            let table_type = quote! {
                                p4rs::table::Table::<#n, fn(#(#param_types),*)> 
                            };
                            let name = format_ident!("{}", v.name);
                            params.push(quote! {
                                #name: &#table_type
                            });
                        }
                    }
                }
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

        let mut names = control.names();
        let sg = StatementGenerator::new(
            self.ast, 
            self.hlir,
            StatementContext::Control(control),
        );
        let body = sg.generate_block(&action.statement_block, &mut names);

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
                        //TODO: use hlir?
                        let ty = resolve_lvalue(&k, self.ast, &tm).unwrap().ty;
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
                        let eg = ExpressionGenerator::new(self.hlir);
                        let xpr = eg.generate_expression(e.as_ref());
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
                match &expr.kind {
                    ExpressionKind::IntegerLit(v) => {
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

}