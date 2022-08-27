use std::fs;

use p4::check::Diagnostics;
use p4::{check, error, error::SemanticError, lexer, parser, preprocessor};
use proc_macro::TokenStream;
use serde::Deserialize;
use serde_tokenstream::ParseWrapper;
use syn::{parse, LitStr};

#[derive(Deserialize)]
struct MacroSettings {
    p4: ParseWrapper<LitStr>,
    pipeline_name: ParseWrapper<LitStr>,
}

struct GenerationSettings {
    pipeline_name: String,
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self {
            pipeline_name: "main".to_owned(),
        }
    }
}

#[proc_macro]
pub fn use_p4(item: TokenStream) -> TokenStream {
    //do_use_p4(item).unwrap()
    match do_use_p4(item) {
        Err(err) => err.to_compile_error().into(),
        Ok(out) => out,
    }
}

fn do_use_p4(item: TokenStream) -> Result<TokenStream, syn::Error> {
    let (filename, settings) =
        if let Ok(filename) = parse::<LitStr>(item.clone()) {
            (filename.value(), GenerationSettings::default())
        } else {
            let MacroSettings { p4, pipeline_name } =
                serde_tokenstream::from_tokenstream(&item.into())?;
            (
                p4.into_inner().value(),
                GenerationSettings {
                    pipeline_name: pipeline_name.into_inner().value(),
                },
            )
        };

    generate_rs(filename, settings)
}

fn generate_rs(
    filename: String,
    settings: GenerationSettings,
) -> Result<TokenStream, syn::Error> {
    //TODO gracefull error handling

    let contents = fs::read_to_string(filename).unwrap();

    let ppr = preprocessor::run(&contents).unwrap();
    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    let lxr = lexer::Lexer::new(lines.clone());
    let mut psr = parser::Parser::new(lxr);
    let ast = psr.run().unwrap();
    let (hlir, diags) = check::all(&ast);
    check(&lines, &diags);
    let tokens = p4_rust::emit_tokens(
        &ast,
        &hlir,
        p4_rust::Settings {
            pipeline_name: settings.pipeline_name,
        },
    );

    Ok(tokens.into())
}

// TODO copy pasta from x4c
fn check(lines: &[&str], diagnostics: &Diagnostics) {
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
