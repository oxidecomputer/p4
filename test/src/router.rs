use crate::softnpu::{self, Frame, Phy};
use std::sync::Arc;
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::net::Ipv6Addr;
use xfr::{ring, FrameBuffer};

const R: usize = 1024;
const N: usize = 4096;
const F: usize = 1500;

p4_macro::use_p4!("p4/examples/codegen/router.p4");

///
///                           ~~~~~~~~~~
///                           ~        ~
///                           ~   p4   ~
///                           ~        ~
///                           ~~~~~~~~~~
///                               |
///                               ▼
/// *=======*                *==========*                *=======*
/// |       | --- ( rx ) --> |          | <-- ( rx ) --- |       |
/// | phy 1 |                | pipeline |                | phy 2 |
/// |       | <-- ( tx ) --- |          | --- ( tx ) --> |       |
/// *=======*                *==========*                *=======*
///
///
#[test]
fn router() -> Result<(), anyhow::Error> {

    let fb = Arc::new(FrameBuffer::<N, F>::new());

    // ingress rings
    let (rx1_p, rx1_c) = ring::<R, N, F>(fb.clone());
    let (rx2_p, rx2_c) = ring::<R, N, F>(fb.clone());

    // egress rings
    let (tx1_p, tx1_c) = ring::<R, N, F>(fb.clone());
    let (tx2_p, tx2_c) = ring::<R, N, F>(fb.clone());

    // create phys
    let phy1 = Phy::new(1, rx1_p);
    let phy2 = Phy::new(2, rx2_p);

    // run phys
    phy1.run(tx1_c, phy1_egress);
    phy2.run(tx2_c, phy2_egress);

    // run the softnpu with the compiled p4 pipelines
    spawn(move || {
        let rx = &[rx1_c, rx2_c];
        let tx = &[tx1_p, tx2_p];
        softnpu::run(rx, tx, ingress_table_router(), parse_start, ingress_apply);
    });

    // shove some test data through the soft npu
    let ip1: Ipv6Addr = "fd00:1000::1".parse().unwrap();
    let mac1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];

    let ip2: Ipv6Addr = "fd00:2000::1".parse().unwrap();
    let mac2 = [0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC];
    let et = 0x86ed;

    let mut data = [0u8; 256];
    let payload = b"do you know the muffin man?";
    write(&phy1, 99, 0x1701d, payload.len(), payload, 47, 23, ip1, ip2, mac1, mac2);


    let payload = b"the muffin man?";
    write(&phy2, 101, 0x1701c, payload.len(), payload, 74, 32, ip2, ip1, mac2, mac1);


    let payload = b"the muffin man!";
    write(&phy1, 99, 0x1701d, payload.len(), payload, 47, 23, ip1, ip2, mac1, mac2);

    let payload = b"why yes";
    write(&phy2, 101, 0x1701c, payload.len(), payload, 74, 32, ip2, ip1, mac2, mac1);
    let payload = b"i know the muffin man";
    write(&phy2, 101, 0x1701c, payload.len(), payload, 74, 32, ip2, ip1, mac2, mac1);
    let payload = b"the muffin man is me!!!";
    write(&phy2, 101, 0x1701c, payload.len(), payload, 74, 32, ip2, ip1, mac2, mac1);

    sleep(Duration::from_secs(2));

    Ok(())
}

#[cfg(test)]
fn write(
    phy: &Phy<R,N,F>,
    traffic_class: u8,
    flow_label: u32,
    payload_length: usize,
    payload: &[u8],
    next_header: u8,
    hop_limit: u8,
    src: Ipv6Addr,
    dst: Ipv6Addr,
    smac: [u8;6],
    dmac: [u8;6],
) {
    let mut data = [0u8; 256];
    let et = 0x86ed;
    let mut pkt = pnet::packet::ipv6::MutableIpv6Packet::new(&mut data).unwrap();
    pkt.set_traffic_class(traffic_class);
    pkt.set_flow_label(flow_label);
    pkt.set_payload_length(payload_length as u16);
    pkt.set_payload(payload);
    pkt.set_next_header(pnet::packet::ip::IpNextHeaderProtocol(next_header));
    pkt.set_hop_limit(hop_limit);
    pkt.set_source(src);
    pkt.set_destination(dst);
    println!("SEND {:x?}", data);
    phy.write(&[Frame::new(smac, dmac, et, &data)]).expect("phy write");
}

#[cfg(test)]
fn phy1_egress(frame: &[u8]) {
    println!("phy 1 !!! {}", String::from_utf8_lossy(&frame[54..]));
}

#[cfg(test)]
fn phy2_egress(frame: &[u8]) {
    println!("phy 2 !!! {}", String::from_utf8_lossy(&frame[54..]));
}
