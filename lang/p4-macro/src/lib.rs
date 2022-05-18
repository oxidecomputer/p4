use std::fs;

use p4::check::Diagnostics;
use p4::{check, error, error::SemanticError, lexer, parser, preprocessor};
use proc_macro::TokenStream;
use syn::{parse, LitStr};

#[proc_macro]
pub fn use_p4(item: TokenStream) -> TokenStream {
    do_use_p4(item).unwrap()
}

fn do_use_p4(item: TokenStream) -> Result<TokenStream, syn::Error> {
    match parse::<LitStr>(item.clone()) {
        Ok(filename) => generate_rs(filename.value()),
        Err(e) => {
            return Err(syn::Error::new(e.span(), "expected filename"));
        }
    }
}

fn generate_rs(filename: String) -> Result<TokenStream, syn::Error> {
    //TODO gracefull error handling

    let contents = fs::read_to_string(filename).unwrap();

    let ppr = preprocessor::run(&contents).unwrap();
    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    let lxr = lexer::Lexer::new(lines.clone());
    let mut psr = parser::Parser::new(lxr);
    let ast = psr.run().unwrap();
    let static_diags = check::all(&ast);
    check(&lines, &static_diags);
    let (tokens, diags) = p4_rust::emit_tokens(&ast);
    check(&lines, &diags);

    Ok(tokens.into())
}

// TODO copy pasta from x4c
fn check(lines: &Vec<&str>, diagnostics: &Diagnostics) {
    let errors = diagnostics.errors();
    if !errors.is_empty() {
        let mut err = Vec::new();
        for e in errors {
            err.push(SemanticError {
                at: e.token.clone(),
                message: e.message.clone(),
                source: lines[e.token.line].into(),
            });
        }
        panic!("{}", error::Error::Semantic(err));
    }
}
