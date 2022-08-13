use crate::{
    rust_type,
    statement::{StatementContext, StatementGenerator},
    Context,
};
use p4::ast::{Direction, Parser, State, AST};
use p4::hlir::Hlir;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct ParserGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
    hlir: &'a Hlir,
}

impl<'a> ParserGenerator<'a> {
    pub(crate) fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        ctx: &'a mut Context,
    ) -> Self {
        Self { ast, hlir, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for parser in &self.ast.parsers {
            for state in &parser.states {
                self.generate_state_function(parser, state);
            }
        }
    }

    pub(crate) fn generate_state_function(
        &mut self,
        parser: &Parser,
        state: &State,
    ) -> (TokenStream, TokenStream) {
        let function_name = format_ident!("{}_{}", parser.name, state.name);

        let mut args = Vec::new();
        for arg in &parser.parameters {
            let name = format_ident!("{}", arg.name);
            let typename = rust_type(&arg.ty);
            match arg.direction {
                Direction::Out | Direction::InOut => {
                    args.push(quote! { #name: &mut #typename });
                }
                _ => args.push(quote! { #name: &mut #typename }),
            };
        }

        let body = self.generate_state_function_body(parser, state);

        let signature = quote! {
            (#(#args),*) -> bool
        };

        let function = quote! {
            pub fn #function_name #signature {
                #body
            }
        };

        self.ctx
            .functions
            .insert(function_name.to_string(), function);

        (signature, body)
    }

    fn generate_state_function_body(
        &mut self,
        parser: &Parser,
        state: &State,
    ) -> TokenStream {
        let sg = StatementGenerator::new(
            self.ast,
            self.hlir,
            StatementContext::Parser(parser),
        );
        let mut names = parser.names();
        sg.generate_block(&state.statements, &mut names)
    }
}
