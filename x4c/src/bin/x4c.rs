use anyhow::Result;
use clap::Parser;
use p4::ast::AST;
use std::sync::Arc;

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let opts = x4c::Opts::parse();
    let filename = Arc::new(opts.filename.clone());
    let mut ast = AST::default();
    x4c::process_file(filename, &mut ast, &opts)?;
    let (hlir, _) = p4::check::all(&ast);

    if opts.check {
        return Ok(());
    }

    match opts.target {
        x4c::Target::Rust => {
            p4_rust::sanitize(&mut ast);
            p4_rust::emit(
                &ast,
                &hlir,
                &opts.out,
                p4_rust::Settings {
                    pipeline_name: "main".to_owned(),
                },
            )?;
        }
        x4c::Target::RedHawk => {
            todo!("RedHawk code generator");
        }
        x4c::Target::Docs => {
            todo!("Docs code generator");
        }
    }

    Ok(())
}
