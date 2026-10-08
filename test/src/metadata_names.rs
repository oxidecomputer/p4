use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};

macro_rules! assert_forwarding {
    ($pipeline:expr) => {{
        let mut pipeline = $pipeline;
        let data = [0, 0, 0, 0, 0xaa, 0xbb];

        for (ingress, ports) in
            [(0u16, &[1u16, 2][..]), (1, &[3][..]), (2, &[0, 1, 3][..])]
        {
            let expected: Vec<_> =
                ports.iter().map(|&port| (port, ingress, port)).collect();

            let mut pkt = packet_in::new(&data);
            let packets: Vec<_> = pipeline
                .process_packet(ingress, &mut pkt)
                .into_iter()
                .map(|(packet, port)| {
                    assert_eq!(packet.header_data.len(), 4);
                    assert_eq!(packet.payload_data, &data[4..]);
                    let ingress_port = u16::from_be_bytes(
                        packet.header_data[..2].try_into().unwrap(),
                    );
                    let egress_port = u16::from_be_bytes(
                        packet.header_data[2..].try_into().unwrap(),
                    );
                    (port, ingress_port, egress_port)
                })
                .collect();

            let mut pkt = packet_in::new(&data);
            let headers: Vec<_> = pipeline
                .process_packet_headers(ingress, &mut pkt)
                .into_iter()
                .map(|(headers, port)| {
                    (
                        port,
                        headers.result.ingress_port.load_le::<u16>(),
                        headers.result.egress_port.load_le::<u16>(),
                    )
                })
                .collect();

            assert_eq!(packets, expected);
            assert_eq!(headers, expected);
        }
    }};
}

mod shared_parameter_names {
    use super::*;

    p4_macro::use_p4!(
        p4 = "test/src/p4/metadata_names_shared.p4",
        pipeline_name = "metadata_names_shared",
    );

    #[test]
    fn forwarding() {
        assert_forwarding!(main_pipeline::new(4));
    }
}

mod wrapper_local_names {
    use super::*;

    p4_macro::use_p4!(
        p4 = "test/src/p4/metadata_names_locals.p4",
        pipeline_name = "metadata_names_locals",
    );

    #[test]
    fn forwarding() {
        assert_forwarding!(main_pipeline::new(4));
    }
}
