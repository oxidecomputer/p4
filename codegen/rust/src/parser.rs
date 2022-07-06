use crate::{
    Context,
    rust_type,
    type_lifetime,
};
use p4::ast::{
    AST, Parser, State, Direction, Statement, ExpressionKind,
    Transition
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct ParserGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
}

impl<'a> ParserGenerator<'a> {
    pub(crate) fn new(ast: &'a AST, ctx: &'a mut Context) -> Self {
        Self{ ast, ctx }
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
        let mut tokens = self.generate_state_statements(parser, state);
        tokens.extend(self.generate_state_transition(parser, state));
        tokens
    }

    fn generate_state_statements(
        &mut self,
        parser: &Parser,
        state: &State,
    ) -> TokenStream {
        let mut tokens = TokenStream::new();

        for stmt in &state.statements {
            match stmt {
                Statement::Empty => continue,
                Statement::Assignment(_lvalue, _expr) => {
                    todo!("parser state assignment statement");
                }
                Statement::Call(call) => {
                    let lval: Vec<TokenStream> = call
                        .lval
                        .name
                        .split(".")
                        .map(|x| format_ident!("{}", x))
                        .map(|x| quote! { #x })
                        .collect();

                    let mut args = Vec::new();
                    for a in &call.args {
                        match &a.kind {
                            ExpressionKind::Lvalue(lvarg) => {
                                let parts: Vec<&str> =
                                    lvarg.name.split(".").collect();
                                let root = parts[0];
                                let mut mut_arg = false;
                                for parg in &parser.parameters {
                                    if parg.name == root {
                                        match parg.direction {
                                            Direction::Out
                                                | Direction::InOut => {
                                                    mut_arg = true;
                                                }
                                            _ => {}
                                        }
                                    }
                                }
                                let lvref: Vec<TokenStream> = parts
                                    .iter()
                                    .map(|x| format_ident!("{}", x))
                                    .map(|x| quote! { #x })
                                    .collect();
                                if mut_arg {
                                    args.push(quote! { &mut #(#lvref).* });
                                } else {
                                    args.push(quote! { #(#lvref).* });
                                }
                            }
                            x => todo!("extern arg {:?}", x),
                        }
                    }
                    tokens.extend(quote! {
                        #(#lval).* ( #(#args),* );
                    });
                }
                x => todo!("codegen: statement {:?}", x),
            }
        }

        tokens
    }

    fn generate_state_transition(
        &mut self,
        parser: &Parser,
        state: &State,
    ) -> TokenStream {
        match &state.transition {
            Some(Transition::Reference(next_state)) => match next_state.as_str() {
                "accept" => quote! { return true; },
                "reject" => quote! { return false; },
                state_ref => {
                    let state_name = format_ident!("{}_{}", parser.name, state_ref);

                    let mut args = Vec::new();
                    for arg in &parser.parameters {
                        let name = format_ident!("{}", arg.name);
                        args.push(quote! { #name });
                    }
                    quote! { return #state_name( #(#args),* ); }
                }
            },
            Some(Transition::Select(_)) => {
                todo!();
            }
            None => quote! { return false; }, // implicit reject?
        }
    }
}
