use crate::mcast::port_bitmap;
use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};

p4_macro::use_p4!(p4 = "test/src/p4/shift.p4", pipeline_name = "shift");

const HEADER_LEN: usize = 32;

fn assert_egress_shifts(ports: &[u16]) {
    let mut pipeline = main_pipeline::new(4);
    pipeline.add_ingress_bitmap_table_entry(
        "set_bitmap",
        &0u16.to_le_bytes(),
        &port_bitmap(16, ports),
        0,
    );

    let expected: Vec<(u16, u128, u128)> =
        ports.iter().map(|&port| (port, 1u128 << port, 1)).collect();
    let data = [0u8; HEADER_LEN];

    let mut pkt = packet_in::new(&data);
    let packets: Vec<_> = pipeline
        .process_packet(0, &mut pkt)
        .into_iter()
        .map(|(packet, port)| {
            assert_eq!(packet.header_data.len(), HEADER_LEN);
            let read = |offset: usize| {
                u128::from_be_bytes(
                    packet.header_data[offset..offset + 16].try_into().unwrap(),
                )
            };
            (port, read(0), read(16))
        })
        .collect();

    let mut pkt = packet_in::new(&data);
    let headers: Vec<_> = pipeline
        .process_packet_headers(0, &mut pkt)
        .into_iter()
        .map(|(headers, port)| {
            (
                port,
                headers.result.port_mask.load_le::<u128>(),
                headers.result.unshifted.load_le::<u128>(),
            )
        })
        .collect();

    assert_eq!(packets, expected, "packet");
    assert_eq!(headers, expected, "headers");
}

#[test]
fn shift_by_egress_port() {
    assert_egress_shifts(&[1, 2]);
}

#[test]
fn shift_by_higher_egress_port() {
    assert_egress_shifts(&[3]);
}

mod counts {
    use bitvec::field::BitField;
    use p4rs::{packet_in, Pipeline};

    p4_macro::use_p4!(
        p4 = "test/src/p4/shift_counts.p4",
        pipeline_name = "shift_counts",
    );

    const VALUE: u32 = 0x8123_4567;
    const HEADER_LEN: usize = 50;

    struct ShiftResults {
        literal: [u32; 2],
        narrow: [u32; 2],
        wide: [u32; 2],
        large_literal: [u8; 2],
        linear: [u8; 4],
        arithmetic: [u8; 2],
        grouped: [u8; 2],
    }

    fn outputs(
        pipeline: &mut main_pipeline,
        count: u128,
    ) -> [(&'static str, ShiftResults); 2] {
        let mut data = [0u8; HEADER_LEN + 2];
        data[..16].copy_from_slice(&count.to_be_bytes());
        data[HEADER_LEN..].copy_from_slice(&[0xaa, 0xbb]);

        let mut pkt = packet_in::new(&data);
        let packets = pipeline.process_packet(0, &mut pkt);
        assert_eq!(packets.len(), 1, "count {count}");

        let (packet, port) = &packets[0];
        assert_eq!(*port, 1);
        assert_eq!(packet.header_data.len(), HEADER_LEN);
        assert_eq!(&packet.header_data[..16], &data[..16]);
        assert_eq!(packet.payload_data, &data[HEADER_LEN..]);
        let read = |offset| {
            u32::from_be_bytes(
                packet.header_data[offset..offset + 4].try_into().unwrap(),
            )
        };
        let packet_result = ShiftResults {
            literal: [read(16), read(20)],
            narrow: [read(24), read(28)],
            wide: [read(32), read(36)],
            large_literal: [packet.header_data[40], packet.header_data[41]],
            linear: packet.header_data[42..46].try_into().unwrap(),
            arithmetic: packet.header_data[46..48].try_into().unwrap(),
            grouped: packet.header_data[48..50].try_into().unwrap(),
        };

        let mut pkt = packet_in::new(&data);
        let mut headers = pipeline.process_packet_headers(0, &mut pkt);
        assert_eq!(headers.len(), 1, "count {count}");
        let (headers, port) = headers.pop().unwrap();
        assert_eq!(port, 1);
        assert_eq!(headers.result.count.load_le::<u128>(), count);
        let header_result = ShiftResults {
            literal: [
                headers.result.left_literal.load_le::<u32>(),
                headers.result.right_literal.load_le::<u32>(),
            ],
            narrow: [
                headers.result.left_narrow.load_le::<u32>(),
                headers.result.right_narrow.load_le::<u32>(),
            ],
            wide: [
                headers.result.left_wide.load_le::<u32>(),
                headers.result.right_wide.load_le::<u32>(),
            ],
            large_literal: [
                headers.result.left_large_literal.load_le::<u8>(),
                headers.result.right_large_literal.load_le::<u8>(),
            ],
            linear: [
                headers.result.left_linear.load_le::<u8>(),
                headers.result.right_linear.load_le::<u8>(),
                headers.result.left_right_linear.load_le::<u8>(),
                headers.result.right_left_linear.load_le::<u8>(),
            ],
            arithmetic: [
                headers.result.add_then_shift.load_le::<u8>(),
                headers.result.shift_by_sum.load_le::<u8>(),
            ],
            grouped: [
                headers.result.grouped_shift.load_le::<u8>(),
                headers.result.grouped_comparison.load_le::<u8>(),
            ],
        };

        [("packet", packet_result), ("headers", header_result)]
    }

    #[test]
    fn literal_counts() {
        let mut pipeline = main_pipeline::new(2);
        for (api, result) in outputs(&mut pipeline, 0) {
            assert_eq!(result.literal, [0x0246_8ace, 0x4091_a2b3], "{api}");
        }
    }

    #[test]
    fn narrow_variable_counts() {
        let mut pipeline = main_pipeline::new(2);
        for count in [0u128, 1, 31, 32, 33, 128, 255, 256] {
            let amount = (count & 0xff) as u32;
            let expected = [
                VALUE.checked_shl(amount).unwrap_or(0),
                VALUE.checked_shr(amount).unwrap_or(0),
            ];
            for (api, result) in outputs(&mut pipeline, count) {
                assert_eq!(result.narrow, expected, "{api}, count {count}");
            }
        }
    }

    #[test]
    fn wide_variable_counts() {
        let mut pipeline = main_pipeline::new(2);
        for count in [0u128, 1, 31, 32, 33, 128, 256, 1u128 << 100, u128::MAX] {
            let amount = u32::try_from(count).unwrap_or(u32::MAX);
            let expected = [
                VALUE.checked_shl(amount).unwrap_or(0),
                VALUE.checked_shr(amount).unwrap_or(0),
            ];
            for (api, result) in outputs(&mut pipeline, count) {
                assert_eq!(result.wide, expected, "{api}, count {count}");
            }
        }
    }

    #[test]
    fn large_literal_counts() {
        let mut pipeline = main_pipeline::new(2);
        for (api, result) in outputs(&mut pipeline, 0) {
            assert_eq!(result.large_literal, [0, 0], "{api}");
        }
    }

    #[test]
    fn linear_shifts() {
        let mut pipeline = main_pipeline::new(2);
        for (api, result) in outputs(&mut pipeline, 0) {
            assert_eq!(result.linear, [8, 16, 1, 6], "{api}");
        }
    }

    #[test]
    fn arithmetic_prec() {
        let mut pipeline = main_pipeline::new(2);
        for (api, result) in outputs(&mut pipeline, 0) {
            assert_eq!(result.arithmetic, [6, 4], "{api}");
        }
    }

    #[test]
    fn explicit_grouping() {
        let mut pipeline = main_pipeline::new(2);
        for (api, result) in outputs(&mut pipeline, 0) {
            assert_eq!(result.grouped, [16, 1], "{api}");
        }
    }
}
