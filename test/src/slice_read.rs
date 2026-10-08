// Copyright 2026 Oxide Computer Company

use bitvec::field::BitField;
use p4rs::{packet_in, Pipeline};
use pnet::packet::ipv4::{Ipv4Packet, MutableIpv4Packet};

use crate::softnpu::{Interface4, SoftNpu};

p4_macro::use_p4!(
    p4 = "test/src/p4/slice_read.p4",
    pipeline_name = "slice_read",
);

/// Read a sub-byte slice from a multi-byte field and verify the
/// byte-reversal mapping is correct.
#[test]
fn slice_read_top_nibble() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);

    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);

    let if0 = Interface4::new(phy0.clone(), "10.0.0.1".parse().unwrap());

    npu.run();

    // dst IP = 239.171.2.3 = 0xEFAB0203.
    // ipv4.dst[23:20] = top nibble of 0xAB = 0xA.
    if0.send(phy1.mac, "239.171.2.3".parse().unwrap(), b"test")?;

    let frames = phy1.recv();
    let frame = &frames[0];
    let ip = Ipv4Packet::new(&frame.payload).unwrap();

    // The P4 compares ipv4.dst[23:20] == 0xA and sets identification=42
    // if true. With correct byte reversal the top nibble of 0xAB is 0xA,
    // and that branch should be taken. An incorrect [20..24] range
    // reads the bottom nibble (0xB) instead, so the comparison
    // fails, and identification stays at 0.
    assert_eq!(
        ip.get_identification(),
        42,
        "ipv4.dst[23:20] should be 0xA (top nibble of 0xAB)"
    );

    Ok(())
}

fn ipv4_frame(frag_offset: u16) -> [u8; 34] {
    let mut buf = [0u8; 34];
    buf[..6].copy_from_slice(&[0x02, 0, 0, 0, 0, 1]);
    buf[6..12].copy_from_slice(&[0x02, 0, 0, 0, 0, 0]);
    buf[12..14].copy_from_slice(&0x0800u16.to_be_bytes());

    let mut ip = MutableIpv4Packet::new(&mut buf[14..]).unwrap();
    ip.set_version(4);
    ip.set_header_length(5);
    ip.set_total_length(20);
    ip.set_fragment_offset(frag_offset);
    ip.set_ttl(0x35);
    ip.set_source("10.0.0.1".parse().unwrap());
    ip.set_destination("239.171.2.3".parse().unwrap());

    buf
}

#[derive(Debug, PartialEq)]
struct SliceReads {
    port: u16,
    identification: u16,
    diffserv: u8,
    ttl: u8,
    protocol: u8,
    checksum: u16,
}

fn slice_reads(frag_offset: u16) -> SliceReads {
    let mut pipeline = main_pipeline::new(2);
    let buf = ipv4_frame(frag_offset);

    let mut pkt = packet_in::new(&buf);
    let packets: Vec<_> = pipeline
        .process_packet(0, &mut pkt)
        .into_iter()
        .map(|(packet, port)| {
            let ip = Ipv4Packet::new(&packet.header_data[14..]).unwrap();
            SliceReads {
                port,
                identification: ip.get_identification(),
                diffserv: packet.header_data[15],
                ttl: ip.get_ttl(),
                protocol: ip.get_next_level_protocol().0,
                checksum: ip.get_checksum(),
            }
        })
        .collect();

    let mut pkt = packet_in::new(&buf);
    let mut headers: Vec<_> = pipeline
        .process_packet_headers(0, &mut pkt)
        .into_iter()
        .map(|(headers, port)| SliceReads {
            port,
            identification: headers.ipv4.identification.load_le(),
            diffserv: headers.ipv4.diffserv.load_le(),
            ttl: headers.ipv4.ttl.load_le(),
            protocol: headers.ipv4.protocol.load_le(),
            checksum: headers.ipv4.hdr_checksum.load_le(),
        })
        .collect();

    assert_eq!(packets, headers, "packet and header outputs differ");
    assert_eq!(headers.len(), 1, "packet should egress on port 1");
    headers.pop().unwrap()
}

#[test]
fn slice_read_sub_byte_field() {
    let reads = slice_reads(0);
    assert_eq!(reads.port, 1);
    assert_eq!(
        reads.protocol, 0x5b,
        "ipv4.ttl[3:0] should read 0x5, the low nibble of 0x35"
    );
    assert_eq!(reads.ttl, 0x35);
}

#[test]
fn slice_read_non_contiguous() {
    let reads = slice_reads(0x0ab5);
    assert_eq!(
        reads.diffserv, 0xb0,
        "ipv4.dst[19:12] of 0xEFAB0203 should read 0xb0"
    );
    assert_eq!(
        reads.checksum, 0xab,
        "ipv4.frag_offset[11:4] of 0x0ab5 should read 0xab"
    );
}
