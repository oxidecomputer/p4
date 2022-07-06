use std::collections::HashMap;
use crate::{
    expression::ExpressionGenerator,
    rust_type,
    is_rust_reference,
};
use p4::ast::{
    DeclarationInfo, Expression, NameInfo, Statement, StatementBlock, Type
};
use p4::hlir::Hlir;
use quote::{format_ident, quote};
use proc_macro2::TokenStream;

pub(crate) struct StatementGenerator<'a> { 
    hlir: &'a Hlir,
}

impl<'a> StatementGenerator<'a> {
    pub fn new(hlir: &'a Hlir) -> Self {
        Self{ hlir }
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
                if is_rust_reference(&lval, names) {
                    quote!{ *#lhs = #rhs; }
                } else {
                    quote!{ #lhs = #rhs; }
                }
            }
            Statement::Call(c) => {
                let eg = ExpressionGenerator::new(self.hlir);
                let lval = eg.generate_lvalue(&c.lval);
                let args: Vec::<TokenStream> = c.args
                    .iter()
                    .map(|xpr| eg.generate_expression(xpr.as_ref()))
                    .collect();
                quote!{ #lval(#(#args),*); }
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
                quote!{
                    let #name: #ty = #initializer;
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
