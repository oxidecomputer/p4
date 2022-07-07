use std::collections::HashMap;
use crate::{
    expression::ExpressionGenerator,
    rust_type,
    is_rust_reference,
    is_header,
};
use p4::ast::{
    AST, Call, Control, DeclarationInfo, Direction, Expression, ExpressionKind,
    NameInfo, Parser, Statement, StatementBlock, Type
};
use p4::hlir::Hlir;
use quote::{format_ident, quote};
use proc_macro2::TokenStream;

#[derive(Debug)]
pub(crate) enum StatementContext<'a> {
    Control(&'a Control),
    #[allow(dead_code)]
    Parser(&'a Parser),
}

pub(crate) struct StatementGenerator<'a> { 
    hlir: &'a Hlir,
    ast: &'a AST,
    context: StatementContext::<'a>,
}

impl<'a> StatementGenerator<'a> {
    pub fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        context: StatementContext::<'a>
    ) -> Self {
        Self{ ast, hlir, context }
    }

    pub(crate) fn generate_block(
        &self,
        sb: &StatementBlock,
        names: &mut HashMap::<String, NameInfo>,
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
        names: &mut HashMap::<String, NameInfo>,
    ) -> TokenStream {
        match stmt {
            Statement::Empty => { TokenStream::new() }
            Statement::Assignment(lval, xpr) => {
                let eg = ExpressionGenerator::new(self.hlir);

                let lhs = eg.generate_lvalue(lval);

                let rhs = eg.generate_expression(xpr.as_ref());
                let rhs_ty = self
                    .hlir
                    .expression_types
                    .get(xpr.as_ref())
                    .expect(&format!("codegen type not found for {:#?}", xpr));

                let name_info = self
                    .hlir
                    .lvalue_decls
                    .get(lval)
                    .expect(&format!("codegen name not resolved for {:#?}", lval));

                let rhs = if rhs_ty != &name_info.ty {
                    let converter = self.converter(rhs_ty, &name_info.ty);
                    quote!( #converter(#rhs) )
                } else {
                    rhs
                };

                if is_rust_reference(&lval, names) {
                    quote!{ *#lhs = #rhs; }
                } else {
                    quote!{ #lhs = #rhs; }
                }
            }
            Statement::Call(c) => {
                let eg = ExpressionGenerator::new(self.hlir);
                let lval = eg.generate_lvalue(&c.lval.pop_right());

                match &self.context {
                    StatementContext::Control(control) => {
                        let mut ts = TokenStream::new();
                        self.generate_control_apply_body_call(
                            control,
                            c,
                            &mut ts,
                        );
                        ts
                    }
                    StatementContext::Parser(_) => {
                        let args: Vec::<TokenStream> = c.args
                            .iter()
                            .map(|xpr| eg.generate_expression(xpr.as_ref()))
                            .collect();
                        quote!{ #lval(#(#args),*); }
                    }

                }

            }
            Statement::If(ifb) => {
                let eg = ExpressionGenerator::new(self.hlir);
                let predicate = eg.generate_expression(ifb.predicate.as_ref());
                let block = self.generate_block(&ifb.block, names);
                let mut ts = quote! {
                    if #predicate { #block }
                };
                for ei in &ifb.else_ifs {
                    let predicate = eg.generate_expression(ei.predicate.as_ref());
                    let block = self.generate_block(&ei.block, names);
                    ts.extend(quote!{else if #predicate { #block }})
                }
                if let Some(eb) = &ifb.else_block {
                    let block = self.generate_block(&eb, names);
                    ts.extend(quote!{else { #block }})
                }
                ts
            }
            Statement::Variable(v) => {
                let name = format_ident!("{}", v.name);
                let ty = rust_type(&v.ty, false, 0);
                let initializer = match &v.initializer {
                    Some(xpr) =>  {
                        let eg = ExpressionGenerator::new(self.hlir);
                        let ini = eg.generate_expression(xpr.as_ref());
                        let ini_ty = self
                            .hlir
                            .expression_types
                            .get(xpr)
                            .expect(&format!("type for expression {:#?}", xpr));
                        if ini_ty != &v.ty {
                            let converter = self.converter(&ini_ty, &v.ty);
                            quote!{ #converter(#ini) }
                        } else {
                            ini
                        }
                    },
                    None => quote!{ #ty::default() },
                };
                names.insert(v.name.clone(), NameInfo{
                    ty: v.ty.clone(),
                    decl: DeclarationInfo::Local,
                });

                //TODO determine for real
                let needs_mut = true;

                if needs_mut {
                    quote!{
                        let mut #name: #ty = #initializer;
                    }
                } else {
                    quote!{
                        let #name: #ty = #initializer;
                    }
                }
            }
            Statement::Constant(c) => {
                let name = format_ident!("{}", c.name);
                let ty = rust_type(&c.ty, false, 0);
                let eg = ExpressionGenerator::new(self.hlir);
                let initializer = eg.generate_expression(c.initializer.as_ref());
                quote!{
                    let #name: #ty = #initializer;
                }
            }
        }
    }

    fn generate_control_apply_body_call(
        &self,
        control: &Control,
        c: &Call,
        tokens: &mut TokenStream,
    ) {
        //
        // get the lval reference to the thing being called
        //
        let parts: Vec<&str> = c.lval.name.split(".").collect();
        if parts.len() != 2 || parts[1] != "apply" {
            panic!(
                "codegen: only <tablename>.apply() calls are 
             supported in apply blocks right now: {:#?}",
             c
            );
        }

        let name_info = self
            .hlir
            .lvalue_decls
            .get(&c.lval.pop_right()).expect(&format!(
                    "codegen: lval root for {:#?} not found in hlir",
                    c.lval
            ));

        let control_instance = match &name_info.ty {
            Type::UserDefined(name) => self.ast.get_control(name).expect(&format!(
                "codegen: control {} not found in AST",
                name,
            )),
            Type::Table => control,
            t => panic!("call references non-user-defined type {:#?}", t),
        };

        // This is a call to another control instance
        if control_instance.name != control.name {

            let call = format_ident!("{}_apply", parts[0]);
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
                                    .get(lval).expect(&format!(
                                            "codegen: lvalue resolve fail {:#?}",
                                            lval
                                    ));
                                match name_info.decl {
                                    DeclarationInfo::Parameter(_) => {
                                        args.push(arg_xpr);
                                    }
                                    _ => {
                                        args.push(quote!{&mut #arg_xpr});
                                    }
                                }
                            }
                            _ => {
                                args.push(quote!{&mut #arg_xpr});
                            }
                        }
                    }
                    _ => {
                        args.push(arg_xpr);
                    }
                }
            }
            let tbl_arg = format_ident!("{}", parts[0]);
            args.push(quote!{#tbl_arg});

            tokens.extend(quote!{
                #call(#(#args),*);
            });

            return;

        }

        let table = match control_instance.get_table(parts[0]) {
            Some(table) => table,
            None => {
                panic!(
                    "codegen: table {} not found in control {} decl: {:#?}",
                    parts[0],
                    control.name,
                    name_info,
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

            let lvref: Vec<TokenStream> = lval
                .name
                .split(".")
                .map(|x| format_ident!("{}", x))
                .map(|x| quote! { #x })
                .collect();

            // determine if this lvalue references a header or a struct,
            // if it's a header there's a bit of extra unsrapping we
            // need to do to match the selector against the value.

            let names = control.names();

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

    fn converter(
        &self,
        from: &Type,
        to: &Type,
    ) -> TokenStream {
        match (from, to) {
            (Type::Int(_), Type::Bit(_)) => {
                quote!{ p4rs::int_to_bitvec }
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