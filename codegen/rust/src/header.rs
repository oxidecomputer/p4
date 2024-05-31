// Copyright 2022 Oxide Computer Company

use crate::{rust_type, type_size, Context};
use p4::ast::{Header, AST};
use quote::{format_ident, quote};

pub(crate) struct HeaderGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
}

impl<'a> HeaderGenerator<'a> {
    pub(crate) fn new(ast: &'a AST, ctx: &'a mut Context) -> Self {
        Self { ast, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for h in &self.ast.headers {
            self.generate_header(h);
        }
    }

    fn generate_header(&mut self, h: &Header) {
        let name = format_ident!("{}", h.name);

        //
        // genrate a rust struct for the header
        //

        // generate struct members
        let mut members = Vec::new();
        for member in &h.members {
            let name = format_ident!("{}", member.name);
            let ty = rust_type(&member.ty);
            members.push(quote! { pub #name: #ty });
        }

        let mut generated = quote! {
            #[derive(Debug, Default, Clone)]
            pub struct #name {
                pub valid: bool,
                #(#members),*
            }
        };

        //
        // generate a constructor that maps the header onto a byte slice
        //

        // generate member assignments
        let mut member_values = Vec::new();
        let mut set_statements = Vec::new();
        let mut to_bitvec_statements = Vec::new();
        let mut checksum_statements = Vec::new();
        let mut dump_statements = Vec::new();
        let fmt = "{} ".repeat(h.members.len() * 2);
        let fmt = fmt.trim();
        let mut offset = 0;
        for member in &h.members {
            let name = format_ident!("{}", member.name);
            let name_s = &member.name;
            let size = type_size(&member.ty, self.ast);
            member_values.push(quote! {
                #name: BitVec::<u8, Msb0>::default()
            });
            let end = offset + size;
            set_statements.push(quote! {
                self.#name = {
                    let mut b = buf.view_bits::<Msb0>()[#offset..#end].to_owned();
                    // NOTE this barfing and then unbarfing a vec is to handle
                    // the p4 confused-endian data model.
                    if #end-#offset > 8 {
                        let mut v = b.into_vec();
                        v.reverse();
                        if ((#end-#offset) % 8) != 0 {
                            if let Some(x) = v.iter_mut().last() {
                                *x <<= (#offset % 8);
                            }
                        }
                        let mut b = BitVec::<u8, Msb0>::from_vec(v);
                        b.resize(#end-#offset, false);
                        b
                    } else {
                        b
                    }
                }
            });
            to_bitvec_statements.push(quote! {
                // NOTE this barfing and then unbarfing a vec is to handle
                // the p4 confused-endian data model.
                if #end-#offset > 8 {
                    let mut v = self.#name.clone().into_vec();
                    if ((#end-#offset) % 8) != 0 {
                        if let Some(x) = v.iter_mut().last() {
                            *x >>= ((#end - #offset) % 8);
                        }
                    }
                    v.reverse();
                    let n = (#end-#offset);
                    let m = n%8;
                    let mut b = BitVec::<u8, Msb0>::from_vec(v);
                    x[#offset..#end] |= &b[m..];
                } else {
                    x[#offset..#end] |= self.#name.to_owned();
                }

            });
            checksum_statements.push(quote! {
                csum = p4rs::bitmath::add_le(csum.clone(), self.#name.csum())
            });
            dump_statements.push(quote! {
                #name_s.cyan(),
                p4rs::dump_bv(&self.#name)
            });

            offset += size;
        }
        let dump = quote! {
            format!(#fmt, #(#dump_statements),*)
        };

        //TODO perhaps we should just keep the whole header as one bitvec so we
        //don't need to construct a consolidated bitvec like to_bitvec does?
        generated.extend(quote! {
            impl Header for #name {
                fn new() -> Self {
                    Self {
                        valid: false,
                        #(#member_values),*
                    }
                }
                fn set(
                    &mut self,
                    buf: &[u8]
                ) -> Result<(), TryFromSliceError> {
                    #(#set_statements);*;
                    Ok(())
                }
                fn size() -> usize {
                    #offset
                }
                fn set_valid(&mut self) {
                    self.valid = true;
                }
                fn set_invalid(&mut self) {
                    self.valid = false;
                }
                fn is_valid(&self) -> bool {
                    self.valid
                }
                fn to_bitvec(&self) -> BitVec<u8, Msb0> {
                    let mut x = bitvec![u8, Msb0; 0u8; Self::size()];
                    #(#to_bitvec_statements);*;
                    x
                }
            }

            impl Checksum for #name {
                fn csum(&self) -> BitVec::<u8, Msb0> {
                    let mut csum = BitVec::new();
                    #(#checksum_statements);*;
                    csum
                }
            }

            impl #name {
                fn setValid(&mut self) {
                    self.valid = true;
                }
                fn setInvalid(&mut self) {
                    self.valid = false;
                }
                fn isValid(&self) -> bool {
                    self.valid
                }
                fn dump(&self) -> String {
                    if self.isValid() {
                        #dump
                    } else {
                        "∅".to_owned()
                    }
                }
            }
        });

        self.ctx.structs.insert(h.name.clone(), generated);
    }
}
