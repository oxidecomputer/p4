// Copyright 2026 Oxide Computer Company

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
                    BinOp::Subtract => {
                        ts.extend(quote!{
                            p4rs::bitmath::sub_le(#lhs_tks.clone(), #rhs_tks.clone())
                        })
                    }
                    BinOp::Mod => {
                        ts.extend(quote!{
                            p4rs::bitmath::mod_le(#lhs_tks.clone(), #rhs_tks.clone())
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
                    BinOp::BitOr | BinOp::BitAnd | BinOp::Xor | BinOp::Mask => {
                        ts.extend(quote! {
                            {
                                let __lhs = #lhs_tks.clone();
                                let __rhs = #rhs_tks.clone();
                                __lhs #op_tks __rhs
                            }
                        });
                    }
                    BinOp::Shl | BinOp::Shr => {
                        let rhs_tks = match &rhs.kind {
                            ExpressionKind::IntegerLit(value) => {
                                self.generate_bit_literal(
                                    128,
                                    u128::try_from(*value).expect(
                                        "negative shift count reached codegen",
                                    ),
                                )
                            }
                            _ => rhs_tks,
                        };

                        let shift = match op {
                            BinOp::Shl => quote! { p4rs::bitmath::shl_le },
                            BinOp::Shr => quote! { p4rs::bitmath::shr_le },
                            _ => unreachable!(),
                        };

                        ts.extend(quote! {
                            #shift(#lhs_tks.clone(), #rhs_tks.clone())
                        });
                    }
                    BinOp::Lt | BinOp::Gt | BinOp::Leq | BinOp::Geq => {
                        ts.extend(quote! {
                            p4rs::bitmath::Unsigned(&#lhs_tks)
                                #op_tks p4rs::bitmath::Unsigned(&#rhs_tks)
                        });
                    }
                }
                quote! { (#ts) }
            }
            ExpressionKind::Index(lval, xpr) => {
                let mut ts = self.generate_lvalue(lval);
                if let ExpressionKind::Slice(begin, end) = &xpr.kind {
                    let (hi, lo) = Self::slice_bounds(begin, end);
                    return Self::generate_slice_read_arith(&ts, hi, lo);
                }
                ts.extend(self.generate_expression(xpr.as_ref()));
                ts
            }
            ExpressionKind::Slice(_begin, _end) => {
                // The HLIR rejects bare slices outside an Index
                // expression, so this is unreachable for well-typed
                // programs.
                unreachable!("bare Slice reached codegen");
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

    /// Extract compile-time hi and lo from slice bound expressions.
    pub(crate) fn slice_bounds(
        begin: &Expression,
        end: &Expression,
    ) -> (P4Bit, P4Bit) {
        let hi: P4Bit = match &begin.kind {
            ExpressionKind::IntegerLit(v) => *v as usize,
            _ => panic!("slice ranges can only be integer literals"),
        };
        let lo: P4Bit = match &end.kind {
            ExpressionKind::IntegerLit(v) => *v as usize,
            _ => panic!("slice ranges can only be integer literals"),
        };
        (hi, lo)
    }

    /// Emit an arithmetic slice read.
    ///
    /// This loads the field as an integer, shifts and masks to extract
    /// the requested bits, then packs everything into a new bitvec.
    pub(crate) fn generate_slice_read_arith(
        lhs: &TokenStream,
        hi: P4Bit,
        lo: P4Bit,
    ) -> TokenStream {
        let slice_width = hi - lo + 1;
        let mask_val = slice_mask(slice_width);
        quote! {
            {
                let __v: u128 = #lhs.load_le();
                let __extracted = (__v >> #lo) & #mask_val;
                let mut __out = bitvec![u8, Msb0; 0; #slice_width];
                __out.store_le(__extracted);
                __out
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
            BinOp::Shl => quote! { << },
            BinOp::Shr => quote! { >> },
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

/// P4 bit position within a field, where bit 0 is the least significant bit.
type P4Bit = usize;

/// Return a mask of the low `width` bits, for `width` in `1..=128`.
pub(crate) fn slice_mask(width: usize) -> u128 {
    u128::MAX >> (128 - width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_mask_widths() {
        assert_eq!(slice_mask(1), 0b1);
        assert_eq!(slice_mask(4), 0xf);
        assert_eq!(slice_mask(12), 0xfff);
        assert_eq!(slice_mask(127), u128::MAX >> 1);
        assert_eq!(slice_mask(128), u128::MAX);
    }
}
