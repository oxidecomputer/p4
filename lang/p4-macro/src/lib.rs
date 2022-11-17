// Copyright 2022 Oxide Computer Company

//! The [`use_p4!`] macro allows for P4 programs to be directly integrated into
//! Rust programs.
//!
//! ```ignore
//! p4_macro::use_p4!("path/to/p4/program.p4");
//! ```
//!
//! This will generate a `main_pipeline` struct that implements the
//! [Pipeline](../p4rs/trait.Pipeline.html) trait. The [`use_p4!`] macro expands
//! directly in to `x4c` compiled code. This includes all data structures,
//! parsers and control blocks.
//!
//! To customize the name of the generated pipeline use the `pipeline_name`
//! parameter.
//!
//! ```ignore
//! p4_macro::use_p4!(p4 = "path/to/p4/program.p4", pipeline_name = "muffin");
//! ```
//! This will result in a `muffin_pipeline` struct being being generated.
//!
//! For documentation on using [Pipeline](../p4rs/trait.Pipeline.html) trait, see the
//! [p4rs](../p4rs/index.html) docs.

use std::fs;
use std::path::Path;
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

/// The `use_p4!` macro uses the `x4c` compiler to generate Rust code from a P4
/// program. The macro itself expands into the generated code. The macro can be
/// called with only the path to the P4 program as an argument or, it can be
/// called with the path to the P4 program plus the name to use for the
/// generated pipeline object.
///
/// For usage examples, see the [p4-macro](index.html) module documentation.
#[proc_macro]
pub fn use_p4(item: TokenStream) -> TokenStream {
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
    let contents = match fs::read_to_string(&*filename) {
        Ok(c) => c,
        Err(e) => panic!("failed to read file {}: {}", filename, e),
    };
    let ppr = preprocessor::run(&contents, filename.clone()).unwrap();
    for included in &ppr.elements.includes {
        let path = Path::new(included);
        if !path.is_absolute() {
            let parent = Path::new(&*filename).parent().unwrap();
            let joined = parent.join(included);
            process_file(
                Arc::new(joined.to_str().unwrap().to_string()),
                ast,
                _settings,
            )?
        } else {
            process_file(Arc::new(included.clone()), ast, _settings)?;
        }
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
