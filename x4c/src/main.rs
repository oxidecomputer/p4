use anyhow::{anyhow, Result};
use clap::Parser;
use p4::check::Diagnostics;
use p4::{check, error, error::SemanticError, lexer, parser, preprocessor};
use std::fs;

#[derive(Parser)]
#[clap(version = "0.1")]
struct Opts {
    #[clap(long)]
    show_tokens: bool,

    #[clap(long)]
    show_ast: bool,

    #[clap(long)]
    show_pre: bool,

    /// File to compile
    filename: String,

    /// What target to generate code for
    #[clap(arg_enum, default_value_t = Target::Rust)]
    target: Target,

    /// Just check code, do not compile.
    #[clap(long)]
    check: bool
}

#[derive(clap::ArgEnum, Clone)]
enum Target {
    Rust,
    RedHawk,
    Docs,
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let contents = fs::read_to_string(opts.filename)
        .map_err(|e| anyhow!("read input: {}", e))?;

    let ppr = preprocessor::run(&contents)?;
    if opts.show_pre {
        println!("{:#?}", ppr.elements);
    }

    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    //println!("lines\n{:#?}", lines);

    let mut lxr = lexer::Lexer::new(lines.clone());
    lxr.show_tokens = opts.show_tokens;

    let mut psr = parser::Parser::new(lxr);
    let ast = psr.run()?;
    if opts.show_ast {
        println!("{:#?}", ast);
    }

    let static_diags = check::all(&ast);
    check(&lines, &static_diags)?;

    if opts.check {
        return Ok(())
    }

    match opts.target {
        Target::Rust => {
            p4_rust::emit(&ast)?;
        }
        Target::RedHawk => {
            todo!("RedHawk code generator");
        }
        Target::Docs => {
            todo!("Docs code generator");
        }
    }

    Ok(())
}

fn check(lines: &Vec<&str>, diagnostics: &Diagnostics) -> Result<()> {
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
