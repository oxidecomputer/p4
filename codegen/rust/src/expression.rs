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
                    BinOp::Shl => {
                        ts.extend(quote!{
                            p4rs::bitmath::shl_le(#lhs_tks.clone(), #rhs_tks.clone())
                        });
                    }
                    BinOp::Shr => {
                        ts.extend(quote!{
                            p4rs::bitmath::shr_le(#lhs_tks.clone(), #rhs_tks.clone())
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
                // For slices, look up the parent field's bit width
                // so generate_slice can adjust for header.rs byte
                // reversal.
                if let ExpressionKind::Slice(begin, end) = &xpr.kind {
                    let ni =
                        self.hlir.lvalue_decls.get(lval).unwrap_or_else(|| {
                            panic!("unresolved lvalue {:#?} in slice", lval)
                        });

                    let field_width = match &ni.ty {
                        p4::ast::Type::Bit(w)
                        | p4::ast::Type::Varbit(w)
                        | p4::ast::Type::Int(w) => *w,
                        ty => panic!(
                            "slice on non-bit type {:?} reached codegen",
                            ty,
                        ),
                    };
                    let (hi, lo) = Self::slice_bounds(begin, end);
                    if Self::slice_is_contiguous(hi, lo, field_width) {
                        ts.extend(self.generate_slice(begin, end, field_width));
                    } else {
                        // Non-contiguous after byte reversal;
                        // replace the lvalue suffix with arithmetic.
                        return Self::generate_slice_read_arith(&ts, hi, lo);
                    }
                } else {
                    ts.extend(self.generate_expression(xpr.as_ref()));
                }
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

    /// Whether `[hi:lo]` on a field of `field_width` bits can be
    /// expressed as a contiguous bitvec range after byte reversal.
    pub(crate) fn slice_is_contiguous(
        hi: P4Bit,
        lo: P4Bit,
        field_width: FieldWidth,
    ) -> bool {
        if field_width <= 8 {
            return true;
        }
        // Non-byte-multiple widths have an additional bit-shift in
        // header.rs storage that reversed_slice_range does not model.
        if !field_width.is_multiple_of(8) {
            return false;
        }
        reversed_slice_range(hi, lo, field_width).is_some()
    }

    pub(crate) fn generate_slice(
        &self,
        begin: &Expression,
        end: &Expression,
        field_width: FieldWidth,
    ) -> TokenStream {
        let (hi, lo) = Self::slice_bounds(begin, end);

        if field_width > 8 {
            let (r, l) = reversed_slice_range(hi, lo, field_width).expect(
                "non-contiguous slice reads must be handled \
                     by the caller via generate_slice_read_arith",
            );
            quote! { [#r..#l] }
        } else {
            // Fields <= 8 bits are not byte-reversed by header.rs,
            // so the naive P4-to-bitvec mapping is correct.
            let l = hi + 1;
            let r = lo;
            quote! { [#r..#l] }
        }
    }

    /// Emit an arithmetic slice read for non-contiguous slices.
    /// Loads the field as an integer, shifts and masks to extract
    /// the requested bits, then packs into a new bitvec.
    pub(crate) fn generate_slice_read_arith(
        lhs: &TokenStream,
        hi: P4Bit,
        lo: P4Bit,
    ) -> TokenStream {
        let slice_width = hi - lo + 1;
        let mask_val = (1u128 << slice_width) - 1;
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

/// P4 bit position (MSB-first index within a field).
type P4Bit = usize;

/// Width of a P4 header field in bits.
type FieldWidth = usize;

/// Half-open bitvec range `(start, end)` into the storage representation.
type BitvecRange = (usize, usize);

/// Map a P4 slice `[hi:lo]` to a bitvec range in byte-reversed storage.
///
/// header.rs reverses byte order for fields wider than 8 bits. Bit
/// positions within each byte are preserved (Msb0). The mapping from
/// P4 bit positions to storage indices:
///
/// ```text
/// wire_idx     = W - 1 - b
/// wire_byte    = wire_idx / 8
/// bit_in_byte  = wire_idx % 8
/// storage_byte = W/8 - 1 - wire_byte
/// bitvec_idx   = storage_byte * 8 + bit_in_byte
/// ```
///
/// # Returns
///
/// `Some(range)` when the slice maps to a contiguous bitvec range
/// (single-byte slices or byte-aligned multi-byte slices), `None`
/// for non-byte-aligned multi-byte slices where byte reversal makes
/// the bits non-contiguous.
pub(crate) fn reversed_slice_range(
    hi: P4Bit,
    lo: P4Bit,
    field_width: FieldWidth,
) -> Option<BitvecRange> {
    // Wire byte indices for the slice endpoints. P4 bit W-1 is in wire
    // byte 0 (MSB-first), so higher bit numbers map to lower byte indices.
    let wire_byte_hi = (field_width - 1 - hi) / 8;
    let wire_byte_lo = (field_width - 1 - lo) / 8;

    if wire_byte_hi == wire_byte_lo {
        // Single-byte slice: map each endpoint individually.
        let map_bit = |bit_pos: usize| -> usize {
            let wire_idx = field_width - 1 - bit_pos;
            let wire_byte = wire_idx / 8;
            let bit_in_byte = wire_idx % 8;
            let storage_byte = field_width / 8 - 1 - wire_byte;
            storage_byte * 8 + bit_in_byte
        };

        let mapped_hi = map_bit(hi);
        let mapped_lo = map_bit(lo);
        Some((mapped_hi.min(mapped_lo), mapped_hi.max(mapped_lo) + 1))
    } else if (hi + 1).is_multiple_of(8) && lo.is_multiple_of(8) {
        // Multi-byte byte-aligned slice: reversed bytes form a
        // contiguous block.
        let storage_byte_start = field_width / 8 - 1 - wire_byte_lo;
        let storage_byte_end = field_width / 8 - 1 - wire_byte_hi;
        Some((storage_byte_start * 8, (storage_byte_end + 1) * 8))
    } else {
        // Non-byte-aligned multi-byte slice: byte reversal makes the
        // bits non-contiguous, so there is no single bitvec range.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify the reversed slice range mapping against the byte reversal
    // in header.rs. For each case we check that the bitvec range lands
    // on the correct bits in the reversed storage layout.

    // Sub-byte slices within a single wire byte.

    #[test]
    fn slice_32bit_top_nibble() {
        // P4 [31:28] on 32-bit: top nibble of wire byte 0.
        // Storage: wire byte 0 -> storage byte 3.
        // High nibble of storage byte 3 = bitvec [24..28].
        assert_eq!(reversed_slice_range(31, 28, 32), Some((24, 28)));
    }

    #[test]
    fn slice_32bit_bottom_nibble() {
        // P4 [3:0] on 32-bit: bottom nibble of wire byte 3.
        // Storage: wire byte 3 -> storage byte 0.
        // Low nibble (Msb0) of storage byte 0 = bitvec [4..8].
        assert_eq!(reversed_slice_range(3, 0, 32), Some((4, 8)));
    }

    #[test]
    fn slice_16bit_top_nibble() {
        // P4 [15:12] on 16-bit: top nibble of wire byte 0.
        // Storage: wire byte 0 -> storage byte 1.
        // High nibble of storage byte 1 = bitvec [8..12].
        assert_eq!(reversed_slice_range(15, 12, 16), Some((8, 12)));
    }

    // Full-byte slices (single byte).

    #[test]
    fn slice_128bit_top_byte() {
        // P4 [127:120] on 128-bit: wire byte 0 -> storage byte 15.
        // bitvec [120..128].
        assert_eq!(reversed_slice_range(127, 120, 128), Some((120, 128)));
    }

    #[test]
    fn slice_16bit_low_byte() {
        // P4 [7:0] on 16-bit: wire byte 1 -> storage byte 0.
        // bitvec [0..8].
        assert_eq!(reversed_slice_range(7, 0, 16), Some((0, 8)));
    }

    #[test]
    fn slice_32bit_middle_byte() {
        // P4 [23:16] on 32-bit: wire byte 1 -> storage byte 2.
        // bitvec [16..24].
        assert_eq!(reversed_slice_range(23, 16, 32), Some((16, 24)));
    }

    // Multi-byte byte-aligned slices.

    #[test]
    fn slice_128bit_top_two_bytes() {
        // P4 [127:112] on 128-bit: wire bytes 0-1 -> storage bytes 14-15.
        // bitvec [112..128].
        assert_eq!(reversed_slice_range(127, 112, 128), Some((112, 128)));
    }

    #[test]
    fn slice_32bit_top_three_bytes() {
        // P4 [31:8] on 32-bit: wire bytes 0-2 -> storage bytes 1-3.
        // bitvec [8..32].
        assert_eq!(reversed_slice_range(31, 8, 32), Some((8, 32)));
    }

    #[test]
    fn slice_32bit_bottom_two_bytes() {
        // P4 [15:0] on 32-bit: wire bytes 2-3 -> storage bytes 0-1.
        // bitvec [0..16].
        assert_eq!(reversed_slice_range(15, 0, 32), Some((0, 16)));
    }

    #[test]
    fn slice_48bit_upper_24() {
        assert_eq!(reversed_slice_range(47, 24, 48), Some((24, 48)));
    }

    #[test]
    fn slice_non_contiguous_returns_none() {
        assert_eq!(reversed_slice_range(11, 4, 32), None);
        assert_eq!(reversed_slice_range(22, 0, 32), None);
    }
}
