// Copyright 2022 Oxide Computer Company

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
                            p4rs::bitmath::add_le(#lhs_tks.clone(), #rhs_tks.clone())
                        });
                    }
                    BinOp::Eq | BinOp::NotEq => {
                        let lhs_tks_ = match &lhs.as_ref().kind {
                            ExpressionKind::Lvalue(lval) => {
                                let name_info = self
                                    .hlir
                                    .lvalue_decls
                                    .get(lval)
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "declaration info for {:#?}",
                                            lval
                                        )
                                    });
                                match name_info.decl {
                                    DeclarationInfo::ActionParameter(_) => {
                                        quote! {
                                            &#lhs_tks
                                        }
                                    }
                                    _ => lhs_tks,
                                }
                            }
                            _ => lhs_tks,
                        };
                        let rhs_tks_ = match &rhs.as_ref().kind {
                            ExpressionKind::Lvalue(lval) => {
                                let name_info = self
                                    .hlir
                                    .lvalue_decls
                                    .get(lval)
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "declaration info for {:#?}",
                                            lval
                                        )
                                    });
                                match name_info.decl {
                                    DeclarationInfo::ActionParameter(_) => {
                                        quote! {
                                            &#rhs_tks
                                        }
                                    }
                                    _ => rhs_tks,
                                }
                            }
                            _ => rhs_tks,
                        };
                        ts.extend(lhs_tks_);
                        ts.extend(op_tks);
                        ts.extend(rhs_tks_);
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
            ExpressionKind::List(elements) => {
                let mut parts = Vec::new();
                for e in elements {
                    parts.push(self.generate_expression(e));
                }
                quote! {
                    &[ #(&#parts),* ]
                }
            }
        }
    }

    pub(crate) fn generate_bit_literal(
        &self,
        width: u16,
        value: u128,
    ) -> TokenStream {
        assert!(width <= 128);

        let width = width as usize;

        quote! {
            {
                let mut x = bitvec![mut u8, Msb0; 0; #width];
                x.store_le(#value);
                x
            }
        }
    }

    pub(crate) fn generate_binop(&self, op: BinOp) -> TokenStream {
        match op {
            BinOp::Add => quote! { + },
            BinOp::Subtract => quote! { - },
            BinOp::Mod => quote! { % },
            BinOp::Geq => quote! { >= },
            BinOp::Gt => quote! { > },
            BinOp::Leq => quote! { <= },
            BinOp::Lt => quote! { < },
            BinOp::Eq => quote! { == },
            BinOp::NotEq => quote! { != },
            BinOp::Mask => quote! { & },
            BinOp::BitAnd => quote! { & },
            BinOp::BitOr => quote! { | },
            BinOp::Xor => quote! { ^ },
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
            /*
            DeclarationInfo::ActionParameter(_) => quote! {
                &#lvalue
            },
            */
            _ => lvalue,
        }
    }
}
