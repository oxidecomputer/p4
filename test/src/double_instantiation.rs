use p4rs::Pipeline;

p4_macro::use_p4!(
    p4 = "p4/examples/codegen/double_instantiation.p4",
    pipeline_name = "double_instantiation",
);

const FOO_ARP: &str = "ingress.foo.resolver.arp";
const BAR_ARP: &str = "ingress.bar.resolver.arp";

fn keys(pipeline: &main_pipeline, table_id: &str) -> Vec<Vec<u8>> {
    pipeline
        .get_table_entries(table_id)
        .unwrap_or_else(|| panic!("no table {table_id}"))
        .into_iter()
        .map(|entry| entry.keyset_data)
        .collect()
}

#[test]
fn each_instance_gets_its_own_table() {
    let pipeline = main_pipeline::new(2);
    let ids = pipeline.get_table_ids();

    assert!(ids.contains(&FOO_ARP), "{ids:?}");
    assert!(ids.contains(&BAR_ARP), "{ids:?}");
}

#[test]
fn instance_tables_hold_separate_entries() {
    let mut pipeline = main_pipeline::new(2);
    assert!(keys(&pipeline, FOO_ARP).is_empty());
    assert!(keys(&pipeline, BAR_ARP).is_empty());

    pipeline.add_table_entry(FOO_ARP, "resolve", &47u32.to_le_bytes(), &[], 0);
    assert_eq!(keys(&pipeline, FOO_ARP), [47u32.to_le_bytes().to_vec()]);
    assert!(
        keys(&pipeline, BAR_ARP).is_empty(),
        "foo entry leaked into bar"
    );

    pipeline.add_table_entry(
        BAR_ARP,
        "resolve",
        &1701u32.to_le_bytes(),
        &[],
        0,
    );
    assert_eq!(keys(&pipeline, FOO_ARP), [47u32.to_le_bytes().to_vec()]);
    assert_eq!(keys(&pipeline, BAR_ARP), [1701u32.to_le_bytes().to_vec()]);
}
