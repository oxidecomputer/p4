p4_macro::use_p4!(
    p4 = "test/src/p4/table_clear.p4",
    pipeline_name = "table_clear",
);

const INGRESS_TBL: &str = "ingress.bakery.muffins";
const EGRESS_TBL: &str = "egress.bakery.muffins";

fn entry_count(pipeline: &main_pipeline, table: &str) -> usize {
    pipeline.get_table_entries(table).unwrap().len()
}

fn add_forward(pipeline: &mut main_pipeline, table: &str, port: u16) {
    pipeline.add_table_entry(
        table,
        "forward",
        &port.to_le_bytes(),
        &1u16.to_le_bytes(),
        0,
    );
}

#[test]
fn clear_table_entries() {
    let mut pipeline = main_pipeline::new(4);

    let ids = pipeline.get_table_ids();
    assert!(ids.contains(&INGRESS_TBL));
    assert!(ids.contains(&EGRESS_TBL));

    for port in 0..3 {
        add_forward(&mut pipeline, INGRESS_TBL, port);
        add_forward(&mut pipeline, EGRESS_TBL, port);
    }
    assert_eq!(entry_count(&pipeline, INGRESS_TBL), 3);
    assert_eq!(entry_count(&pipeline, EGRESS_TBL), 3);

    assert!(pipeline.clear_table_entries(INGRESS_TBL));
    assert_eq!(entry_count(&pipeline, INGRESS_TBL), 0);
    assert_eq!(entry_count(&pipeline, EGRESS_TBL), 3);

    assert!(pipeline.clear_table_entries(INGRESS_TBL));
    assert_eq!(entry_count(&pipeline, INGRESS_TBL), 0);

    assert!(!pipeline.clear_table_entries("egress.bakery.scones"));
    assert_eq!(entry_count(&pipeline, EGRESS_TBL), 3);

    add_forward(&mut pipeline, INGRESS_TBL, 7);
    assert_eq!(entry_count(&pipeline, INGRESS_TBL), 1);

    assert!(pipeline.clear_table_entries(EGRESS_TBL));
    assert_eq!(entry_count(&pipeline, EGRESS_TBL), 0);
    assert_eq!(entry_count(&pipeline, INGRESS_TBL), 1);
}
