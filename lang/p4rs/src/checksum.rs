// Copyright 2022 Oxide Computer Company

use bitvec::prelude::*;

#[derive(Default)]
pub struct Csum(u16);

impl Csum {
    pub fn add(&mut self, a: u8, b: u8) {
        let x = u16::from_be_bytes([a, b]);
        let (mut result, overflow) = self.0.overflowing_add(x);
        if overflow {
            result += 1;
        }
        self.0 = result;
    }
    pub fn add128(&mut self, data: [u8; 16]) {
        self.add(data[0], data[1]);
        self.add(data[2], data[3]);
        self.add(data[4], data[5]);
        self.add(data[6], data[7]);
        self.add(data[8], data[9]);
        self.add(data[10], data[11]);
        self.add(data[12], data[13]);
        self.add(data[14], data[15]);
    }
    pub fn add32(&mut self, data: [u8; 4]) {
        self.add(data[0], data[1]);
        self.add(data[2], data[3]);
    }
    pub fn add16(&mut self, data: [u8; 2]) {
        self.add(data[0], data[1]);
    }
    pub fn result(&self) -> u16 {
        !self.0
    }
}

pub fn udp6_checksum(data: &[u8]) -> u16 {
    let src = &data[8..24];
    let dst = &data[24..40];
    let udp_len = &data[4..6];
    let next_header = &data[6];
    let src_port = &data[40..42];
    let dst_port = &data[42..44];
    let payload_len = &data[44..46];
    let payload = &data[48..];

    let mut csum = Csum(0);

    for i in (0..src.len()).step_by(2) {
        csum.add(src[i], src[i + 1]);
    }
    for i in (0..dst.len()).step_by(2) {
        csum.add(dst[i], dst[i + 1]);
    }
    csum.add(udp_len[0], udp_len[1]);
    //TODO assuming no jumbo
    csum.add(0, *next_header);
    csum.add(src_port[0], src_port[1]);
    csum.add(dst_port[0], dst_port[1]);
    csum.add(payload_len[0], payload_len[1]);

    for i in (0..payload.len()).step_by(2) {
        csum.add(payload[i], payload[i + 1]);
    }
    if payload.len() % 2 == 1 {
        csum.add(payload[payload.len() - 1], 0);
    }

    csum.result()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pnet::packet::udp;
    use std::net::Ipv6Addr;

    #[test]
    fn udp_checksum() {
        let mut packet = [0u8; 200];

        //
        // ipv6
        //

        packet[0] = 6; // version = 6
        packet[5] = 160; // 160 byte payload (200 - payload=40)
        packet[6] = 17; // next header = udp
        packet[7] = 255; // hop limit = 255

        // src = fd00::1
        packet[8] = 0xfd;
        packet[23] = 0x01;

        // dst = fd00::2
        packet[24] = 0xfd;
        packet[39] = 0x02;

        //
        // udp
        //

        packet[41] = 47; // source port = 47
        packet[43] = 74; // dstination port = 74
        packet[45] = 160; // udp header + payload = 160 bytes
        for i in 46..200 {
            packet[i] = ((i as f32) * (3.14 / 32.0) * 10.0) as u8;
        }

        let x = udp6_checksum(&packet);

        let p = udp::UdpPacket::new(&packet[40..]).unwrap();
        let src: Ipv6Addr = "fd00::1".parse().unwrap();
        let dst: Ipv6Addr = "fd00::2".parse().unwrap();
        let y = udp::ipv6_checksum(&p, &src, &dst);

        assert_eq!(x, y);
    }
}

pub trait Checksum {
    fn csum(&self) -> BitVec<u8, Lsb0>;
}

impl Checksum for BitVec<u8, Lsb0> {
    fn csum(&self) -> BitVec<u8, Lsb0> {
        let x: u128 = self.load();
        let buf = x.to_be_bytes();
        let mut c: u16 = 0;
        for i in (0..16).step_by(2) {
            c += u16::from_be_bytes([buf[i], buf[i + 1]])
        }
        let c = !c;
        let mut result = bitvec![u8, Lsb0; 0u8, 16];
        result.store(c);
        result
    }
}
