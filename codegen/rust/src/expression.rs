use p4::ast::{
    BinOp, Expression, ExpressionKind, Lvalue,
};
use quote::{format_ident, quote};
use proc_macro2::TokenStream;

pub(crate) struct ExpressionGenerator { }

impl ExpressionGenerator {

    pub(crate) fn generate_expression(xpr: &Expression) -> TokenStream {
        match &xpr.kind {
            ExpressionKind::BoolLit(v) => {
                quote!{ #v }
            }
            ExpressionKind::IntegerLit(v) => {
                quote!{ #v.into() }
            }
            ExpressionKind::BitLit(width, v) => {
                Self::generate_bit_literal(*width, *v)
            }
            ExpressionKind::SignedLit(_width, _v) => {
                todo!("generate expression signed lit");
            }
            ExpressionKind::Lvalue(v) => {
                Self::generate_lvalue(v)
            }
            ExpressionKind::Binary(lhs, op, rhs) => {
                let mut ts = TokenStream::new();
                ts.extend(Self::generate_expression(lhs.as_ref()));
                ts.extend(Self::generate_binop(*op));
                ts.extend(Self::generate_expression(rhs.as_ref()));
                ts
            }
            ExpressionKind::Index(lval, xpr) => {
                let mut ts = Self::generate_lvalue(lval);
                ts.extend(Self::generate_expression(xpr.as_ref()));
                ts
            }
            ExpressionKind::Slice(begin, end) => {
                let lhs = Self::generate_expression(begin.as_ref());
                let rhs = Self::generate_expression(end.as_ref());
                quote!{
                    [#lhs..#rhs]
                }
            }
        }
    }

    pub(crate) fn generate_bit_literal(width: u16, value: u128) -> TokenStream {
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

    pub(crate) fn generate_binop(op: BinOp) -> TokenStream {
        match op {
            BinOp::Add => quote! { + },
            BinOp::Subtract=> quote! { - },
            BinOp::Geq => quote! { >= },
            BinOp::Eq => quote! { == },
            BinOp::Mask => quote! { & },
        }
    }

    pub(crate) fn generate_lvalue(lval: &Lvalue) -> TokenStream {
        let lv: Vec<TokenStream> = lval
            .name
            .split(".")
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();

        return quote!{ #(#lv).* };
    }
}
