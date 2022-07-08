use crate::{
    Context,
    rust_type,
    type_lifetime,
    statement::{StatementGenerator, StatementContext},
};
use p4::ast::{
    AST, Parser, State, Direction
};
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
        Self{ ast, hlir, ctx }
    }

    pub(crate) fn generate(&mut self) {
        for parser in &self.ast.parsers {
            for state in &parser.states {
                self.generate_state_function(parser, state);
            }
        }
    }

    fn generate_state_function(
        &mut self,
        parser: &Parser,
        state: &State,
    ) {
        let function_name = format_ident!("{}_{}", parser.name, state.name);

        let mut args = Vec::new();
        for arg in &parser.parameters {
            let name = format_ident!("{}", arg.name);
            let typename = rust_type(&arg.ty, false, 0);
            let lifetime = type_lifetime(self.ast, &arg.ty);
            match arg.direction {
                Direction::Out | Direction::InOut => {
                    args.push(quote! { #name: &mut #typename #lifetime });
                }
                _ => args.push(quote! { #name: &mut #typename #lifetime }),
            };
        }

        let body = self.generate_state_function_body(parser, state);

        let function = quote! {
            pub fn #function_name<'a>(#(#args),*) -> bool {
                #body
            }
        };

        self.ctx.functions.insert(function_name.to_string(), function);
    }

    fn generate_state_function_body(
        &mut self, 
        parser: &Parser,
        state: &State,
    ) -> TokenStream {
        /*
        let tokens = self.generate_state_statements(parser, state);
        tokens
        */
        let sg = StatementGenerator::new(
            self.ast,
            self.hlir,
            StatementContext::Parser(parser),
        );
        let mut names = parser.names();
        sg.generate_block(&state.statements, &mut names)
    }
}
