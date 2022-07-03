use crate::{
    Context,
};
use p4::ast::{
    AST, Struct, Type,
};
use quote::{format_ident, quote};

pub(crate) struct StructGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
}

impl<'a> StructGenerator<'a> {
    pub(crate) fn new(ast: &'a AST, ctx: &'a mut Context) -> Self {
        Self{ ast, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for s in &self.ast.structs {
            self.generate_struct(s);
        }
    }

    fn generate_struct(&mut self, s: &Struct) {
        let mut members = Vec::new();

        let mut needs_lifetime = false;

        for member in &s.members {
            let name = format_ident!("{}", member.name);
            match &member.ty {
                Type::UserDefined(ref typename) => {
                    if let Some(_) = self.ast.get_header(typename) {
                        let ty = format_ident!("{}", typename);
                        members.push(quote! { pub #name: #ty::<'a> });
                        needs_lifetime = true;
                    } else {
                        panic!("Struct member {:#?} undefined in {:#?}", member, s);
                    }
                }
                Type::Bit(_size) => {
                    members.push(quote! { pub #name: BitVec::<u8, Lsb0> });
                }
                x => {
                    todo!("struct member {}", x)
                }
            }
        }

        let name = format_ident!("{}", s.name);

        let lifetime = if needs_lifetime {
            quote! { <'a> }
        } else {
            quote! {}
        };

        let structure = quote! {
            #[derive(Debug)]
            pub struct #name #lifetime {
                #(#members),*
            }
        };
        self.ctx.structs.insert(s.name.clone(), structure);
    }
}
