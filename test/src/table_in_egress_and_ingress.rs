p4_macro::use_p4!(
    p4 = "test/src/p4/table_in_egress_and_ingress.p4",
    pipeline_name = "table_in_ingresss_and_egress",
);

// This test is just to make sure the above code compiles

#[test]
fn table_in_egress_and_ingress() {
    println!("table in ingress and egress compiles");
}
