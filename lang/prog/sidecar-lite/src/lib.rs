// Copyright 2022 Oxide Computer Company

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
}
