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

use std::collections::HashMap;
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

    generate_rs(filename, settings).map(Into::into)
}

fn generate_rs(
    filename: String,
    settings: GenerationSettings,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    //TODO gracefull error handling

    let mut ast = AST::default();
    let mut sources = HashMap::new();
    process_file(Arc::new(filename), &mut ast, &mut sources)?;
    p4_rust::sanitize(&mut ast);

    let (hlir, diags) = check::all(&ast);
    check(&sources, &diags)?;

    let tokens = p4_rust::emit_tokens(
        &ast,
        &hlir,
        p4_rust::Settings {
            pipeline_name: settings.pipeline_name.clone(),
        },
    );

    Ok(tokens)
}

fn process_file(
    filename: Arc<String>,
    ast: &mut AST,
    sources: &mut HashMap<Arc<String>, Vec<String>>,
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
                sources,
            )?
        } else {
            process_file(Arc::new(included.clone()), ast, sources)?;
        }
    }

    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    let lxr = lexer::Lexer::new(lines, filename.clone());
    let mut psr = parser::Parser::new(lxr);
    psr.run(ast).unwrap();
    sources.insert(filename, ppr.lines);
    Ok(())
}

// TODO copy pasta from x4c
fn check(
    sources: &HashMap<Arc<String>, Vec<String>>,
    diagnostics: &Diagnostics,
) -> Result<(), syn::Error> {
    let errors = diagnostics.errors();
    if !errors.is_empty() {
        let mut err = Vec::new();
        for e in errors {
            err.push(SemanticError {
                at: e.token.clone(),
                message: e.message.clone(),
                source: sources
                    .get(&e.token.file)
                    .and_then(|lines| lines.get(e.token.line))
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            error::Error::Semantic(err).to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_rejects_replication_outside_ingress() {
        let source = r#"
extern Replicate {
    void replicate(in bit<128> bitmap);
}
struct meta_t {
    bit<128> bitmap;
}
parser parse(inout meta_t m) {
    state start {
        transition accept;
    }
}
control ingress(inout meta_t m) {
    apply { }
}
control egress(inout meta_t m) {
    Replicate() rep;
    apply {
        rep.replicate(m.bitmap);
    }
}
SoftNPU(parse(), ingress(), egress()) main;
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("egress.p4");
        fs::write(&path, source).unwrap();

        let error = generate_rs(
            path.to_str().unwrap().into(),
            GenerationSettings::default(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains(
            "Replicate may only be instantiated in the ingress control"
        ));
        assert!(error.contains("Replicate() rep;"));
        assert!(error.contains(path.to_str().unwrap()));
    }

    #[test]
    fn macro_rejects_width_in_final_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.p4");
        fs::write(&path, "header wide_t {\n    bit<129> field;\n}\n").unwrap();

        let error = generate_rs(
            path.to_str().unwrap().into(),
            GenerationSettings::default(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("Width 129 exceeds the 128-bit compiler limit"));
        assert!(error.contains("bit<129> field;"));
    }

    #[test]
    fn macro_diagnostic_uses_included_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.p4");
        let included = dir.path().join("wide.p4");
        fs::write(&path, "#include <wide.p4>\n").unwrap();
        fs::write(
            &included,
            "\n\n\nheader wide_t {\n    bit<129> included_field;\n}\n",
        )
        .unwrap();

        let error = generate_rs(
            path.to_str().unwrap().into(),
            GenerationSettings::default(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("Width 129 exceeds the 128-bit compiler limit"));
        assert!(error.contains("bit<129> included_field;"));
        assert!(error.contains(included.to_str().unwrap()));
    }

    #[test]
    fn macro_checks_complete_program_after_includes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.p4");
        fs::write(
            &path,
            "#include <structs.p4>\nheader header_t {\n    bit<8> field;\n}\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("structs.p4"),
            "struct headers_t {\n    header_t hdr;\n}\n",
        )
        .unwrap();

        generate_rs(
            path.to_str().unwrap().into(),
            GenerationSettings::default(),
        )
        .unwrap();
    }
}
