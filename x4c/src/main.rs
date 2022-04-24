use std::fs;
use clap::Parser;
use anyhow::{anyhow, Result};
use p4rs::{preprocessor, lexer, parser};

#[derive(Parser)]
#[clap(version = "0.1")]
struct Opts {
    #[clap(long)]
    show_tokens: bool,

    /// File to compile
    filename: String
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let contents = fs::read_to_string(opts.filename)
        .map_err(|e| anyhow!("read input: {}", e))?;
    
    let ppr = preprocessor::run(&contents)?;
    println!("{:#?}", ppr.elements);

    let lines: Vec<&str> = ppr.lines.iter().map(|x| x.as_str()).collect();
    //println!("lines\n{:#?}", lines);

    let mut lxr = lexer::Lexer::new(lines);
    lxr.show_tokens = opts.show_tokens;

    let mut psr = parser::Parser::new(lxr);
    let ast = psr.run()?;

    println!("{:#?}", ast);

    Ok(())
}
