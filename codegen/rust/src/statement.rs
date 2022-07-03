use crate::{
    expression::ExpressionGenerator,
    rust_type,
};
use p4::ast::{
    Statement, StatementBlock,
};
use quote::{format_ident, quote};
use proc_macro2::TokenStream;

pub(crate) struct StatementGenerator { }

impl StatementGenerator {
    pub(crate) fn generate_block(sb: &StatementBlock) -> TokenStream {
        let mut ts = TokenStream::new();
        for stmt in &sb.statements {
            ts.extend(Self::generate_statement(stmt));
        }
        ts
    }

    pub(crate) fn generate_statement(stmt: &Statement) -> TokenStream {
        match stmt {
            Statement::Empty => { TokenStream::new() }
            Statement::Assignment(lval, xpr) => {
                let lhs = ExpressionGenerator::generate_lvalue(lval);
                let rhs = ExpressionGenerator::generate_expression(xpr.as_ref());
                quote!{ #lhs = #rhs; }
            }
            Statement::Call(c) => {
                let lval = ExpressionGenerator::generate_lvalue(&c.lval);
                let args: Vec::<TokenStream> = c.args
                    .iter()
                    .map(|xpr| ExpressionGenerator::generate_expression(xpr.as_ref()))
                    .collect();
                quote!{ #lval(#(#args),*); }
            }
            Statement::If(ifb) => {
                let predicate = 
                    ExpressionGenerator::generate_expression(ifb.predicate.as_ref());
                let block = Self::generate_block(&ifb.block);
                let mut ts = quote! {
                    if #predicate { #block }
                };
                for ei in &ifb.else_ifs {
                    let predicate = ExpressionGenerator::generate_expression(
                        ei.predicate.as_ref()
                    );
                    let block = Self::generate_block(&ei.block);
                    ts.extend(quote!{else if #predicate { #block }})
                }
                if let Some(eb) = &ifb.else_block {
                    let block = Self::generate_block(&eb);
                    ts.extend(quote!{else { #block }})
                }
                ts
            }
            Statement::Variable(v) => {
                let name = format_ident!("{}", v.name);
                let ty = rust_type(&v.ty, false, 0);
                let initializer = match &v.initializer {
                    Some(xpr) =>  {
                        ExpressionGenerator::generate_expression(xpr.as_ref())
                    },
                    None => quote!{ #ty::default() },
                };
                quote!{
                    let #name: #ty = #initializer;
                }
            }
            Statement::Constant(c) => {
                let name = format_ident!("{}", c.name);
                let ty = rust_type(&c.ty, false, 0);
                let initializer = ExpressionGenerator::generate_expression(
                    c.initializer.as_ref()
                );
                quote!{
                    let #name: #ty = #initializer;
                }
            }
        }
    }

}
