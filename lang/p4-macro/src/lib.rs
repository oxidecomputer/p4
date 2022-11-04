// Copyright 2022 Oxide Computer Company

use std::fs;
use std::sync::Arc;

use p4::check::Diagnostics;
use p4::{
    ast::AST, check, error, error::SemanticError, lexer, parser, preprocessor,
};
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

    let mut ast = AST::default();
    process_file(Arc::new(filename), &mut ast, &settings)?;

    let (hlir, _) = check::all(&ast);

    let tokens: TokenStream = p4_rust::emit_tokens(
        &ast,
        &hlir,
        p4_rust::Settings {
            pipeline_name: settings.pipeline_name.clone(),
        },
    )
    .into();

    Ok(tokens)
}

fn process_file(
    filename: Arc<String>,
    ast: &mut AST,
    _settings: &GenerationSettings,
) -> Result<(), syn::Error> {
    let contents = fs::read_to_string(&*filename).unwrap();
    let ppr = preprocessor::run(&contents, filename.clone()).unwrap();
    for included in &ppr.elements.includes {
        process_file(Arc::new(included.clone()), ast, _settings)?;
    }

    let (_, diags) = check::all(ast);
    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    check(&lines, &diags);
    let lxr = lexer::Lexer::new(lines.clone(), filename);
    let mut psr = parser::Parser::new(lxr);
    psr.run(ast).unwrap();
    p4_rust::sanitize(ast);
    Ok(())
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
