use std::collections::HashMap;
use std::fs;
use std::io;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use p4::ast::{AST, Direction, Type, Struct, Header};
use p4::check::{Diagnostics, Diagnostic, Level};

/// An object for keeping track of state as we generate code.
#[derive(Default)]
struct Context {
    /// Rust structs we've generated.
    structs: HashMap<String, TokenStream>,

    /// Diagnositcs collected during code generation
    diags: Diagnostics,
}


pub fn emit(ast: &AST) -> io::Result<Diagnostics> {

    //
    // initialize a context to track state while we generate code
    //
    let mut ctx = Context::default();

    //
    // genearate rust code for the P4 AST
    //
    handle_parsers(ast, &mut ctx);

    //
    // collect all the tokens we generated into one stream
    //
    
    // start with use statements
    let mut tokens = quote!{ use p4rs::*; };
    for s in ctx.structs.values() {
        tokens.extend(s.clone());
    }

    //
    // format the code and write it out to a Rust source file
    //
    //println!("{:#?}", tokens);
    let f: syn::File = syn::parse2(tokens).unwrap();
    fs::write("out.rs", prettyplease::unparse(&f))?;

    Ok(ctx.diags)

}

fn handle_parsers(ast: &AST, ctx: &mut Context) {

    // - iterate through parsers and look at headers
    // - generate a Struct object for each struct
    // - generate a Header object for each header

    //
    // iterate through the parsers, looking for out parameters and generating
    // Struct and Header object for the ones we find.
    //
    for parser in &ast.parsers {
        for parameter in &parser.parameters {

            // ignore parameters not in an out direction, we're just generating
            // supporting data structures right now.
            if parameter.direction != Direction::Out {
                continue;
            }
            if let Type::UserDefined(ref typename) = parameter.ty {
                if let Some(decl) = ast.get_struct(typename) {
                    generate_struct(ast, decl, ctx)
                }
                else {
                    // semantic error undefined type
                    ctx.diags.push(Diagnostic{
                        level: Level::Error,
                        message: format!(
                            "Undefined type {}",
                            parameter.ty,
                        ),
                        token: parameter.ty_token.clone(),
                    });
                }
            }
            else {
                // semantic error, out parameters must be structures
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "Out parameter must be a struct, found {}",
                        parameter.ty,
                    ),
                    token: parameter.ty_token.clone(),
                });
                    
            }
        }
    }

}

fn generate_struct(ast: &AST, s: &Struct, ctx: &mut Context) {
    for member in &s.members {
        if let Type::UserDefined(ref typename) = member.ty {
            if let Some(decl) = ast.get_header(typename) {
                // only generate code for types we have not already generated
                // code for.
                if !ctx.structs.contains_key(typename) {
                    generate_header(ast, decl, ctx)
                }
            }
            else {
                // semantic error undefined header
                ctx.diags.push(Diagnostic{
                    level: Level::Error,
                    message: format!(
                        "Undefined header {}",
                        member.ty,
                    ),
                    token: member.token.clone(),
                });
            }
        }
        else {
            //TODO support for primitive types in structs
            ctx.diags.push(Diagnostic{
                level: Level::Error,
                message: "Only headers are supported as struct members".into(),
                token: member.token.clone(),
            });
        }
    }
}


fn generate_header(_ast: &AST, h: &Header, ctx: &mut Context) {

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
        #[derive(Debug)]
        pub struct #name<'a> {
            #(#members),*
        }
    };

    //
    // generate a constructor that maps the header onto a byte slice
    //
    
    // generate member assignments
    let mut member_values = Vec::new();
    let mut offset = 0;
    for member in &h.members {
        let name = format_ident!("{}", member.name);
        let size = type_size(&member.ty);
        let required_bytes = if size & 7 > 0 {
            (size >> 3) + 1
        } else {
            size >> 3
        };
        let end = offset + required_bytes;
        let ty = rust_type(&member.ty);
        member_values.push(quote! {
            #name: #ty::new(&buf[#offset..#end])?
        } );
        offset += required_bytes;
    }

    generated.extend(quote! {
        impl<'a> #name<'a> {
            pub fn new(buf: &'a [u8]) -> Result<Self, TryFromSliceError> {
                Ok(Self {
                    #(#member_values),*
                })
            }
        }
    });

    ctx.structs.insert(h.name.clone(), generated);

}

fn rust_type(ty: &Type) -> TokenStream {
    match ty {
        Type::Bool => quote! { bool },
        Type::Error => todo!("generate error type"),
        Type::Bit(size) => quote! { Bit::<'a, #size> },
        Type::Int(_size) => todo!("generate int type"),
        Type::Varbit(_size) => todo!("generate varbit type"),
        Type::String => quote! { String },
        Type::UserDefined(name) => {
            let typename = format_ident!("{}", name);
            quote!{ #typename }
        },
    }
}

fn type_size(ty: &Type) -> usize {
    match ty {
        Type::Bool => 1,
        Type::Error => todo!("generate error size"),
        Type::Bit(size) => *size,
        Type::Int(size) => *size,
        Type::Varbit(size) => *size,
        Type::String => todo!("generate string size"),
        Type::UserDefined(_name) => todo!("generate user defined type size"),
    }
}
