// Copyright 2022 Oxide Computer Company

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

    if opts.check {
        return Ok(());
    }

    match opts.target {
        x4c::Target::Rust => {
            // NOTE: it's important to sanitize *before* generating hlir as the
            // sanitization process can change lvalue names.
            p4_rust::sanitize(&mut ast);
            let (hlir, _) = p4::check::all(&ast);
            p4_rust::emit(
                &ast,
                &hlir,
                &format!("{}.rs", opts.out),
                p4_rust::Settings {
                    pipeline_name: "main".to_owned(),
                },
            )?;
        }
        x4c::Target::Htq => {
            let (hlir, _) = p4::check::all(&ast);
            p4_htq::emit(&ast, &hlir, &format!("{}.htq", opts.out))?;
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
