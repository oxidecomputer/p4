use crate::{
    Context,
    rust_type, type_size,
};
use p4::ast::{
    AST, Header,
};
use quote::{format_ident, quote};

pub(crate) struct HeaderGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
}

impl<'a> HeaderGenerator<'a> {
    pub(crate) fn new(ast: &'a AST, ctx: &'a mut Context) -> Self {
        Self{ ast, ctx }
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
        let mut offset = 0;
        for member in &h.members {
            let size = type_size(&member.ty);
            let name = format_ident!("{}", member.name);
            let ty = rust_type(&member.ty, true, offset);
            members.push(quote! { pub #name: #ty });
            offset += size;
        }

        let mut generated = quote! {
            #[derive(Debug)]
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
        let mut offset = 0;
        for member in &h.members {
            let name = format_ident!("{}", member.name);
            let size = type_size(&member.ty);
            member_values.push(quote! {
                #name: BitVec::<u8, Msb0>::default()
            });
            let end = offset+size;
            set_statements.push(quote! {
                self.#name = buf.view_bits::<Msb0>()[#offset..#end].to_owned()
            });
            offset += size;
        }

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
            }
        });

        self.ctx.structs.insert(h.name.clone(), generated);
    }
}
