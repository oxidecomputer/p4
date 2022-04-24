use std::fs;
use clap::Parser;
use anyhow::{anyhow, Result};
use p4::{preprocessor, lexer, parser, check};

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
    filename: String
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

    let mut lxr = lexer::Lexer::new(lines);
    lxr.show_tokens = opts.show_tokens;

    let mut psr = parser::Parser::new(lxr);
    let ast = psr.run()?;
    if opts.show_ast {
        println!("{:#?}", ast);
    }

    let diagnostics = check::all(&ast);
    let errors = diagnostics.errors();
    if !errors.is_empty() {
        for e in errors {
            println!("{:?}", e);
        }
        return Err(anyhow!("static checking failed"));
    }


    Ok(())
}
