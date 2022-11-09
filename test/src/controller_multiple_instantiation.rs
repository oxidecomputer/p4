p4_macro::use_p4!(
    p4 = "test/src/p4/controller_multiple_instantiation.p4",
    pipeline_name = "cmi",
);

#[test]
fn controller_multiple_instantiation() -> Result<(), anyhow::Error> {
    println!("it compiles!");
    Ok(())
}
