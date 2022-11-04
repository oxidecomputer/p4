// Copyright 2022 Oxide Computer Company

use crate::{
    expression::ExpressionGenerator, is_header, is_header_member,
    is_rust_reference, rust_type,
};
use p4::ast::{
    Call, Control, DeclarationInfo, Direction, Expression, ExpressionKind,
    NameInfo, Parser, Statement, StatementBlock, Transition, Type, AST,
};
use p4::hlir::Hlir;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) enum StatementContext<'a> {
    Control(&'a Control),
    #[allow(dead_code)]
    Parser(&'a Parser),
}

pub(crate) struct StatementGenerator<'a> {
    hlir: &'a Hlir,
    ast: &'a AST,
    context: StatementContext<'a>,
}

impl<'a> StatementGenerator<'a> {
    pub fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        context: StatementContext<'a>,
    ) -> Self {
        Self { ast, hlir, context }
    }

    pub(crate) fn generate_block(
        &self,
        sb: &StatementBlock,
        names: &mut HashMap<String, NameInfo>,
    ) -> TokenStream {
        let mut ts = TokenStream::new();
        for stmt in &sb.statements {
            ts.extend(self.generate_statement(stmt, names));
        }
        ts
    }

    pub(crate) fn generate_statement(
        &self,
        stmt: &Statement,
        names: &mut HashMap<String, NameInfo>,
    ) -> TokenStream {
        match stmt {
            Statement::Empty => TokenStream::new(),
            Statement::Assignment(lval, xpr) => {
                let eg = ExpressionGenerator::new(self.hlir);

                let lhs = eg.generate_lvalue(lval);

                let rhs = eg.generate_expression(xpr.as_ref());
                let rhs_ty = self
                    .hlir
                    .expression_types
                    .get(xpr.as_ref())
                    .unwrap_or_else(|| {
                        panic!("codegen type not found for {:#?}", xpr)
                    });

                let name_info =
                    self.hlir.lvalue_decls.get(lval).unwrap_or_else(|| {
                        panic!("codegen name not resolved for {:#?}", lval)
                    });

                if is_header_member(lval, self.hlir) {
                    return quote! { #lhs = #rhs.clone(); };
                }

                let rhs = if rhs_ty != &name_info.ty {
                    let converter = self.converter(rhs_ty, &name_info.ty);
                    quote!( #converter(#rhs) )
                } else {
                    rhs
                };

                let rhs = if let Type::Bit(_) = rhs_ty {
                    // TODO eww, to better to figure out precisely when to_owned
                    // and clone are needed
                    quote! { #rhs.to_owned().clone() }
                } else if let Type::UserDefined(_) = rhs_ty {
                    quote! { #rhs.clone() }
                } else {
                    rhs
                };

                if is_rust_reference(lval, names) {
                    quote! { *#lhs = #rhs; }
                } else {
                    quote! { #lhs = #rhs; }
                }
            }
            Statement::Call(c) => match &self.context {
                StatementContext::Control(control) => {
                    let mut ts = TokenStream::new();
                    self.generate_control_body_call(control, c, &mut ts);
                    ts
                }
                StatementContext::Parser(parser) => {
                    let mut ts = TokenStream::new();
                    self.generate_parser_body_call(parser, c, &mut ts);
                    ts
                }
            },
            Statement::If(ifb) => {
                let eg = ExpressionGenerator::new(self.hlir);
                let predicate = eg.generate_expression(ifb.predicate.as_ref());
                let block = self.generate_block(&ifb.block, names);
                let mut ts = quote! {
                    if #predicate { #block }
                };
                for ei in &ifb.else_ifs {
                    let predicate =
                        eg.generate_expression(ei.predicate.as_ref());
                    let block = self.generate_block(&ei.block, names);
                    ts.extend(quote! {else if #predicate { #block }})
                }
                if let Some(eb) = &ifb.else_block {
                    let block = self.generate_block(eb, names);
                    ts.extend(quote! {else { #block }})
                }
                ts
            }
            Statement::Variable(v) => {
                let name = format_ident!("{}", v.name);
                let ty = rust_type(&v.ty);
                let initializer = match &v.initializer {
                    Some(xpr) => {
                        let eg = ExpressionGenerator::new(self.hlir);
                        let ini = eg.generate_expression(xpr.as_ref());
                        let ini_ty =
                            self.hlir.expression_types.get(xpr).unwrap_or_else(
                                || panic!("type for expression {:#?}", xpr),
                            );
                        if ini_ty != &v.ty {
                            let converter = self.converter(ini_ty, &v.ty);
                            quote! { #converter(#ini) }
                        } else {
                            ini
                        }
                    }
                    None => quote! { #ty::default() },
                };
                names.insert(
                    v.name.clone(),
                    NameInfo {
                        ty: v.ty.clone(),
                        decl: DeclarationInfo::Local,
                    },
                );

                //TODO determine for real
                let needs_mut = true;

                if needs_mut {
                    quote! {
                        let mut #name: #ty = #initializer;
                    }
                } else {
                    quote! {
                        let #name: #ty = #initializer;
                    }
                }
            }
            Statement::Constant(c) => {
                let name = format_ident!("{}", c.name);
                let ty = rust_type(&c.ty);
                let eg = ExpressionGenerator::new(self.hlir);
                let initializer =
                    eg.generate_expression(c.initializer.as_ref());
                quote! {
                    let #name: #ty = #initializer;
                }
            }
            Statement::Transition(transition) => {
                let parser = match self.context {
                    StatementContext::Parser(p) => p,
                    _ => {
                        panic!(
                            "transition statement outside parser: {:#?}",
                            transition,
                        )
                    }
                };
                match transition {
                    Transition::Reference(next_state) => {
                        match next_state.name.as_str() {
                            "accept" => quote! { return true; },
                            "reject" => quote! { return false; },
                            state_ref => {
                                let state_name = format_ident!(
                                    "{}_{}",
                                    parser.name,
                                    state_ref
                                );
                                let mut args = Vec::new();
                                for arg in &parser.parameters {
                                    let name = format_ident!("{}", arg.name);
                                    args.push(quote! { #name });
                                }
                                quote! {
                                    softnpu_provider::parser_transition!(||(#state_ref));
                                    return #state_name( #(#args),* );
                                }
                            }
                        }
                    }
                    Transition::Select(_) => {
                        todo!();
                    }
                }
            }
            Statement::Return(xpr) => {
                let eg = ExpressionGenerator::new(self.hlir);
                if let Some(xpr) = xpr {
                    let xp = eg.generate_expression(xpr.as_ref());
                    quote! { return #xp; }
                } else {
                    quote! { return }
                }
            }
        }
    }

    fn generate_parser_body_call(
        &self,
        parser: &Parser,
        c: &Call,
        tokens: &mut TokenStream,
    ) {
        let lval: Vec<TokenStream> = c
            .lval
            .name
            .split('.')
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();

        let mut args = Vec::new();
        for a in &c.args {
            match &a.kind {
                ExpressionKind::Lvalue(lvarg) => {
                    let parts: Vec<&str> = lvarg.name.split('.').collect();
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

    fn generate_control_body_call(
        &self,
        control: &Control,
        c: &Call,
        tokens: &mut TokenStream,
    ) {
        //
        // get the lval reference to the thing being called
        //
        if c.lval.name.split('.').count() < 2 {
            panic!(
                "codegen: bare calls not supported, \
                only <ref>.apply() calls are: {:#?}",
                c
            );
        }
        match c.lval.leaf() {
            "apply" => {
                self.generate_control_apply_body_call(control, c, tokens);
            }
            "setValid" => {
                self.generate_header_set_validity(c, tokens, true);
            }
            "setInvalid" => {
                self.generate_header_set_validity(c, tokens, false);
            }
            "isValid" => {
                self.generate_header_get_validity(c, tokens);
            }
            _ => {
                // assume we are at an extern call

                // TODO check the extern call against defined externs in checker
                // before we get here

                self.generate_control_extern_call(control, c, tokens);
            }
        }
    }

    fn generate_control_extern_call(
        &self,
        _control: &Control,
        c: &Call,
        tokens: &mut TokenStream,
    ) {
        let eg = ExpressionGenerator::new(self.hlir);
        let mut args = Vec::new();

        for a in &c.args {
            let arg_xpr = eg.generate_expression(a.as_ref());
            args.push(arg_xpr);
        }

        let lvref: Vec<TokenStream> = c
            .lval
            .name
            .split('.')
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();

        tokens.extend(quote! {
            #(#lvref).*(#(#args),*);
        })
    }

    fn generate_control_apply_body_call(
        &self,
        control: &Control,
        c: &Call,
        tokens: &mut TokenStream,
    ) {
        let name_info = self
            .hlir
            .lvalue_decls
            .get(&c.lval.pop_right())
            .unwrap_or_else(|| {
                panic!("codegen: lval root for {:#?} not found in hlir", c.lval)
            });

        let control_instance = match &name_info.ty {
            Type::UserDefined(name) => {
                self.ast.get_control(name).unwrap_or_else(|| {
                    panic!("codegen: control {} not found in AST", name,)
                })
            }
            Type::Table => control,
            t => panic!("call references non-user-defined type {:#?}", t),
        };

        let root = c.lval.root();

        // This is a call to another control instance
        if control_instance.name != control.name {
            let eg = ExpressionGenerator::new(self.hlir);
            let mut args = Vec::new();
            for (i, a) in c.args.iter().enumerate() {
                let arg_xpr = eg.generate_expression(a.as_ref());
                match control_instance.parameters[i].direction {
                    Direction::Out | Direction::InOut => {
                        match &a.as_ref().kind {
                            ExpressionKind::Lvalue(lval) => {
                                let name_info = self
                                    .hlir
                                    .lvalue_decls
                                    .get(lval)
                                    .unwrap_or_else(|| {
                                        panic!(
                                        "codegen: lvalue resolve fail {:#?}",
                                        lval
                                    )
                                    });
                                match name_info.decl {
                                    DeclarationInfo::Parameter(_) => {
                                        args.push(arg_xpr);
                                    }
                                    _ => {
                                        args.push(quote! {&mut #arg_xpr});
                                    }
                                }
                            }
                            _ => {
                                args.push(quote! {&mut #arg_xpr});
                            }
                        }
                    }
                    _ => {
                        args.push(arg_xpr);
                    }
                }
            }

            let tables = control_instance.tables(self.ast);
            for (cs, table) in tables {
                let control = cs.last().unwrap();
                let name =
                    format_ident!("{}_table_{}", control.name, table.name,);
                args.push(quote! { #name });
            }

            let cname = &control_instance.name;
            let call = format_ident!("{}_apply", control_instance.name);

            tokens.extend(quote! {
                softnpu_provider::control_apply!(||(#cname));
                #call(#(#args),*);
            });

            return;
        }

        let table = match control_instance.get_table(root) {
            Some(table) => table,
            None => {
                panic!(
                    "codegen: table {} not found in control {} decl: {:#?}",
                    root, control.name, name_info,
                );
            }
        };

        //
        // match an action based on the key material
        //

        let table_name =
            format_ident!("{}_table_{}", control_instance.name, table.name,);

        let table_name_str =
            format!("{}_table_{}", control_instance.name, table.name,);

        let mut action_args = Vec::new();
        for p in &control.parameters {
            let name = format_ident!("{}", p.name);
            action_args.push(quote! { #name });
        }

        for var in &control.variables {
            let name = format_ident!("{}", var.name);
            if let Type::UserDefined(typename) = &var.ty {
                if self.ast.get_extern(typename).is_some() {
                    action_args.push(quote! { &#name });
                }
            }
        }

        let mut selector_components = Vec::new();
        for (lval, _match_kind) in &table.key {
            let lvref: Vec<TokenStream> = lval
                .name
                .split('.')
                .map(|x| format_ident!("{}", x))
                .map(|x| quote! { #x })
                .collect();

            // determine if this lvalue references a header or a struct,
            // if it's a header there's a bit of extra unsrapping we
            // need to do to match the selector against the value.

            let names = control.names();

            if lval.degree() > 1
                && is_header(&lval.pop_right(), self.ast, &names)
            {
                //TODO: to_biguint is bad here, copying on data path
                selector_components.push(quote! {
                    p4rs::bitvec_to_biguint(
                        &#(#lvref).*
                    )
                });
            } else {
                selector_components.push(quote! {
                    p4rs::bitvec_to_biguint(&#(#lvref).*)
                });
            }
        }
        let default_action =
            format_ident!("{}_action_{}", control.name, table.default_action);
        tokens.extend(quote! {
            let matches = #table_name.match_selector(
                &[#(#selector_components),*]
            );
            if matches.len() > 0 {
                softnpu_provider::control_table_hit!(||#table_name_str);
                (matches[0].action)(#(#action_args),*)
            }
        });
        if table.default_action != "NoAction" {
            tokens.extend(quote! {
                else {
                    softnpu_provider::control_table_miss!(||#table_name_str);
                    #default_action(#(#action_args),*);
                }
            });
        } else {
            tokens.extend(quote! {
                else {
                    softnpu_provider::control_table_miss!(||#table_name_str);
                }
            });
        }
    }

    fn generate_header_set_validity(
        &self,
        c: &Call,
        tokens: &mut TokenStream,
        valid: bool,
    ) {
        let lhs: Vec<TokenStream> = c
            .lval
            .pop_right()
            .name
            .split('.')
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();
        if valid {
            tokens.extend(quote! {
                #(#lhs).*.set_valid();
            });
        } else {
            tokens.extend(quote! {
                #(#lhs).*.set_invalid();
            });
        }
    }

    fn generate_header_get_validity(&self, c: &Call, tokens: &mut TokenStream) {
        let lhs: Vec<TokenStream> = c
            .lval
            .pop_right()
            .name
            .split('.')
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();
        tokens.extend(quote! {
            #(#lhs).*.is_valid()
        });
    }

    fn converter(&self, from: &Type, to: &Type) -> TokenStream {
        match (from, to) {
            (Type::Int(_), Type::Bit(_)) => {
                quote! { p4rs::int_to_bitvec }
            }
            _ => todo!("type converter for {} to {}", from, to),
        }
    }

    //XXX
    #[allow(dead_code)]
    fn assign(to: NameInfo, xpr: &Expression) -> TokenStream {
        match to.ty {
            Type::Bool => todo!(),
            Type::Error => todo!(),
            Type::Bit(width) => Self::assign_to_bit(width, xpr),
            Type::Varbit(_) => todo!(),
            Type::Int(_) => todo!(),
            Type::String => todo!(),
            Type::UserDefined(_) => todo!(),
            Type::ExternFunction => todo!(),
            Type::Table => todo!(),
            Type::Void => todo!(),
            Type::List(_) => todo!(),
            Type::State => todo!(),
        }
    }

    //XXX
    #[allow(dead_code)]
    fn assign_to_bit(_width: usize, _xpr: &Expression) -> TokenStream {
        /*
        match xpr {
            Expression::BoolLit(_v) => todo!(),
            Expression::IntegerLit(_v) => todo!(),
            Expression::BitLit(_width, _v) => todo!(),
            Expression::SignedLit(_width, _v) => todo!(),
            Expression::Lvalue(_v) => todo!(),
            Expression::Binary(Box<Expression
        }
        */
        todo!();
    }
}
