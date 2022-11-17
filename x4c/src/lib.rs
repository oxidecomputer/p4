// Copyright 2022 Oxide Computer Company

use anyhow::{anyhow, Result};
use clap::Parser;
use p4::check::Diagnostics;
use p4::{
    ast::AST, check, error, error::SemanticError, lexer, parser, preprocessor,
};
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Parser)]
#[clap(version = "0.1")]
pub struct Opts {
    /// Show parsed lexical tokens.
    #[clap(long)]
    pub show_tokens: bool,

    /// Show parsed abstract syntax tree.
    #[clap(long)]
    pub show_ast: bool,

    /// Show parsed preprocessor info.
    #[clap(long)]
    pub show_pre: bool,

    /// Show high-level intermediate representation info.
    #[clap(long)]
    pub show_hlir: bool,

    /// File to compile.
    pub filename: String,

    /// What target to generate code for.
    #[clap(arg_enum, default_value_t = Target::Rust)]
    pub target: Target,

    /// Just check code, do not compile.
    #[clap(long)]
    pub check: bool,

    /// Filename to write generated code to.
    #[clap(short, long, default_value = "out.rs")]
    pub out: String,
}

#[derive(clap::ArgEnum, Clone)]
pub enum Target {
    Rust,
    RedHawk,
    Docs,
}

pub fn process_file(
    filename: Arc<String>,
    ast: &mut AST,
    opts: &Opts,
) -> Result<()> {

    let contents = fs::read_to_string(&*filename)
        .map_err(|e| anyhow!("read input: {}: {}", &*filename, e))?;

    let ppr = preprocessor::run(&contents, filename.clone())?;
    if opts.show_pre {
        println!("{:#?}", ppr.elements);
    }

    for included in &ppr.elements.includes {

        let path = Path::new(&*included);
        if !path.is_absolute() {
            let parent = Path::new(&*filename).parent().unwrap();
            let joined = parent.join(&included);
            process_file(
                Arc::new(joined.to_str().unwrap().to_string()),
                ast,
                opts)?
        } else {
            process_file(Arc::new(included.clone()), ast, opts)?
        }
    }

    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();

    let mut lxr = lexer::Lexer::new(lines.clone(), filename);
    lxr.show_tokens = opts.show_tokens;

    let mut psr = parser::Parser::new(lxr);
    psr.run(ast)?;
    if opts.show_ast {
        println!("{:#?}", ast);
    }

    let (hlir, diags) = check::all(ast);
    check(&lines, &diags)?;

    if opts.show_hlir {
        println!("{:#?}", hlir);
    }

    Ok(())
}

fn check(lines: &[&str], diagnostics: &Diagnostics) -> Result<()> {
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
        Err(error::Error::Semantic(err))?;
    }
    Ok(())
}
