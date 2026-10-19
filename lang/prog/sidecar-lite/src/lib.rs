// Copyright 2026 Oxide Computer Company

#![allow(clippy::too_many_arguments)]

p4_macro::use_p4!(p4 = "test/src/p4/sidecar-lite.p4", pipeline_name = "main");

#[cfg(test)]
mod tests {
    use super::*;
    use p4rs::{packet_in, Pipeline};

    fn v6_packet() -> [u8; 62] {
        let mut buf = [0u8; 62];
        buf[..6].copy_from_slice(&[0x02, 0, 0, 0, 0, 2]);
        buf[6..12].copy_from_slice(&[0x02, 0, 0, 0, 0, 1]);
        buf[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
        buf[14] = 0x60;
        buf[18..20].copy_from_slice(&8u16.to_be_bytes());
        buf[20] = 59;
        buf[21] = 64;
        buf[22..38].copy_from_slice(&[0xfd; 16]);
        buf[38..54].copy_from_slice(&[0xfe; 16]);
        buf
    }

    #[test]
    fn routing_miss_drops() {
        let mut pipeline = main_pipeline::new(4);
        let buf = v6_packet();
        let mut pkt = packet_in::new(&buf);
        let out = pipeline.process_packet(0, &mut pkt);
        let ports: Vec<u16> = out.iter().map(|(_, p)| *p).collect();
        assert_eq!(ports, Vec::<u16>::new(), "routing miss must drop");
    }

    #[test]
    fn mcast_group_replicates_to_bitmap_ports() {
        let mut pipeline = main_pipeline::new(4);

        let group: std::net::Ipv6Addr = "ff0e::1".parse().unwrap();
        let bitmap: u128 = (1 << 1) | (1 << 2) | (1 << 3);
        pipeline.add_ingress_mcast_mcast_replication_v6_entry(
            "set_port_bitmap",
            &u128::from(group).to_le_bytes(),
            &bitmap.to_le_bytes(),
            0,
        );

        let mut buf = v6_packet();
        buf[..6].copy_from_slice(&[0x33, 0x33, 0x00, 0x00, 0x00, 0x01]);
        buf[38..54].copy_from_slice(&group.octets());

        let mut pkt = packet_in::new(&buf);
        let packet_ports: Vec<u16> = pipeline
            .process_packet(1, &mut pkt)
            .iter()
            .map(|(_, port)| *port)
            .collect();

        let mut pkt = packet_in::new(&buf);
        let header_ports: Vec<u16> = pipeline
            .process_packet_headers(1, &mut pkt)
            .iter()
            .map(|(_, port)| *port)
            .collect();

        assert_eq!(packet_ports, [2, 3], "packet");
        assert_eq!(header_ports, [2, 3], "headers");
    }
}
