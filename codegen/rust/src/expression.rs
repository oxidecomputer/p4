use p4::ast::{BinOp, DeclarationInfo, Expression, ExpressionKind, Lvalue};
use p4::hlir::Hlir;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct ExpressionGenerator<'a> {
    hlir: &'a Hlir,
}

impl<'a> ExpressionGenerator<'a> {
    pub fn new(hlir: &'a Hlir) -> Self {
        Self { hlir }
    }

    pub(crate) fn generate_expression(&self, xpr: &Expression) -> TokenStream {
        match &xpr.kind {
            ExpressionKind::BoolLit(v) => {
                quote! { #v }
            }
            ExpressionKind::IntegerLit(v) => {
                quote! { #v }
            }
            ExpressionKind::BitLit(width, v) => {
                self.generate_bit_literal(*width, *v)
            }
            ExpressionKind::SignedLit(_width, _v) => {
                todo!("generate expression signed lit");
            }
            ExpressionKind::Lvalue(v) => self.generate_lvalue(v),
            ExpressionKind::Binary(lhs, op, rhs) => {
                let lhs_tks = self.generate_expression(lhs.as_ref());
                let op_tks = self.generate_binop(*op);
                let rhs_tks = self.generate_expression(rhs.as_ref());
                let mut ts = TokenStream::new();
                match op {
                    BinOp::Add => {
                        ts.extend(quote!{
                            p4rs::bitmath::add(#lhs_tks.clone(), #rhs_tks.clone())
                        });
                    }
                    _ => {
                        ts.extend(lhs_tks);
                        ts.extend(op_tks);
                        ts.extend(rhs_tks);
                    }
                }
                ts
            }
            ExpressionKind::Index(lval, xpr) => {
                let mut ts = self.generate_lvalue(lval);
                ts.extend(self.generate_expression(xpr.as_ref()));
                ts
            }
            ExpressionKind::Slice(begin, end) => {
                /*
                let lhs = self.generate_expression(begin.as_ref());
                let rhs = self.generate_expression(end.as_ref());
                */
                let l = match &begin.kind {
                    ExpressionKind::IntegerLit(v) => *v as usize,
                    _ => panic!("slice ranges can only be integer literals"),
                };
                let l = l + 1;
                let r = match &end.kind {
                    ExpressionKind::IntegerLit(v) => *v as usize,
                    _ => panic!("slice ranges can only be integer literals"),
                };
                quote! {
                    [#r..#l]
                }
            }
            ExpressionKind::Call(call) => {
                let lv: Vec<TokenStream> = call
                    .lval
                    .name
                    .split('.')
                    .map(|x| format_ident!("{}", x))
                    .map(|x| quote! { #x })
                    .collect();

                let lvalue = quote! { #(#lv).* };
                let mut args = Vec::new();
                for arg in &call.args {
                    args.push(self.generate_expression(arg));
                }
                quote! {
                    #lvalue(#(#args),*)
                }
            }
        }
    }

    //TODO consistent byte order
    pub(crate) fn generate_bit_literal(
        &self,
        width: u16,
        value: u128,
    ) -> TokenStream {
        assert!(width <= 128);

        let width = width as usize;

        if width <= 8 {
            let v = value as u8;
            quote! { #v.view_bits::<Msb0>().to_bitvec() }
        } else if width <= 16 {
            let v = (value as u16).to_be();
            quote! {
                {
                    let mut x = bitvec![mut u8, Msb0; 0; 16];
                    x.store(#v);
                    x
                }
            }
        } else if width <= 32 {
            let v = value as u32;
            quote! {
                {
                    let mut x = bitvec![mut u8, Msb0; 0; 32];
                    x.store(#v);
                    x
                }
            }
        } else if width <= 64 {
            let v = value as u64;
            quote! {
                {
                    let mut x = bitvec![mut u8, Msb0; 0; 64];
                    x.store(#v);
                    x
                }
            }
        } else if width <= 128 {
            let v = value as u128;
            quote! {
                {
                    let mut x = bitvec![mut u8, Msb0; 0; 128];
                    x.store(#v);
                    x
                }
            }
        } else {
            todo!("bit<x> where x > 128");
        }
    }

    pub(crate) fn generate_binop(&self, op: BinOp) -> TokenStream {
        match op {
            BinOp::Add => quote! { + },
            BinOp::Subtract => quote! { - },
            BinOp::Geq => quote! { >= },
            BinOp::Eq => quote! { == },
            BinOp::NotEq => quote! { != },
            BinOp::Mask => quote! { & },
        }
    }

    pub(crate) fn generate_lvalue(&self, lval: &Lvalue) -> TokenStream {
        let lv: Vec<TokenStream> = lval
            .name
            .split('.')
            .map(|x| format_ident!("{}", x))
            .map(|x| quote! { #x })
            .collect();

        let lvalue = quote! { #(#lv).* };

        let name_info = self
            .hlir
            .lvalue_decls
            .get(lval)
            .unwrap_or_else(|| panic!("declaration info for {:#?}", lval));

        match name_info.decl {
            DeclarationInfo::HeaderMember => quote! {
                #lvalue
            },
            _ => lvalue,
        }
    }
}
