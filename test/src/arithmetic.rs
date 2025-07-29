use pnet::packet::ipv4::Ipv4Packet;

use crate::softnpu::{Interface4, SoftNpu};

p4_macro::use_p4!(
    p4 = "test/src/p4/arithmetic.p4",
    pipeline_name = "arithmetic",
);

#[test]
fn arithmetic() -> Result<(), anyhow::Error> {
    let pipeline = main_pipeline::new(2);

    let mut npu = SoftNpu::new(2, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);

    let if0 = Interface4::new(phy0.clone(), "1.0.0.1".parse().unwrap());
    let msg = b"muffins!";

    npu.run();
    if0.send(phy1.mac, "2.0.0.1".parse().unwrap(), msg)?;

    let frames = phy1.recv();
    let frame = &frames[0];
    let ip = Ipv4Packet::new(&frame.payload).unwrap();
    assert_eq!(ip.get_identification(), 15);

    Ok(())
}
