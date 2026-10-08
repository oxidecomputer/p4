use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};

p4_macro::use_p4!(
    p4 = "test/src/p4/slice_assign_empty.p4",
    pipeline_name = "slice_assign_empty",
);

fn assert_outputs(ingress: u16, expected: &[(u16, u16)]) {
    let mut pipeline = main_pipeline::new(4);
    let data = [];

    let mut pkt = packet_in::new(&data);
    let packets: Vec<_> = pipeline
        .process_packet(ingress, &mut pkt)
        .into_iter()
        .map(|(packet, port)| {
            assert_eq!(packet.header_data.len(), 2);
            let value =
                u16::from_be_bytes(packet.header_data[..2].try_into().unwrap());
            (port, value)
        })
        .collect();

    let mut pkt = packet_in::new(&data);
    let headers: Vec<_> = pipeline
        .process_packet_headers(ingress, &mut pkt)
        .into_iter()
        .map(|(headers, port)| (port, headers.result.value.load_le::<u16>()))
        .collect();

    assert_eq!(packets.as_slice(), expected);
    assert_eq!(headers.as_slice(), expected);
}

#[test]
fn full_width_egress_port() {
    assert_outputs(0, &[(1, 0)]);
}

// P4-16 8.25 leaves uninitialized locals unspecified, while SoftNPU zeroes
// them out.
#[test]
fn uninitialized_local_is_zero() {
    assert_outputs(1, &[(1, 1)]);
}

#[test]
fn noncontiguous_egress_port() {
    assert_outputs(2, &[(2, 0)]);
}

#[test]
fn ordered_comparison_is_numeric() {
    assert_outputs(3, &[(1, 1)]);
}

#[test]
fn int_literal_assignments_are_sized() {
    assert_outputs(4, &[(1, 3)]);
}

#[test]
fn int_literal_control_argument_is_sized() {
    assert_outputs(5, &[(1, 7)]);
}

#[test]
fn int_literal_slice_assignments_are_sized() {
    assert_outputs(6, &[(1, 0x0020)]);
}

#[test]
fn sliced_shift_operands_are_owned() {
    assert_outputs(7, &[(1, 4)]);
}

#[test]
fn bit_widening_zero_extends() {
    assert_outputs(8, &[(1, 0x0105)]);
}

#[test]
fn bit_widening_operands_are_owned() {
    assert_outputs(9, &[(1, 0x01ab)]);
}

#[test]
fn zero_width_int_literals_are_empty() {
    assert_outputs(10, &[(1, 4)]);
}
