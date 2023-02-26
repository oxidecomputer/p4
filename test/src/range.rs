use crate::expect_frames;
use crate::softnpu::{Interface4, RxFrame, SoftNpu};
use std::net::Ipv4Addr;

p4_macro::use_p4!(p4 = "test/src/p4/range.p4", pipeline_name = "range",);

fn v4_range_key(addr: Ipv4Addr) -> [u8; 4] {
    let k: u32 = addr.into();
    k.to_le_bytes()
}

#[test]
fn range() -> Result<(), anyhow::Error> {
    let mut pipeline = main_pipeline::new(4);

    //
    // add table entries
    //

    let begin = v4_range_key("2.0.0.0".parse().unwrap());
    let end = v4_range_key("4.0.0.0".parse().unwrap());
    let mut buf = begin.to_vec();
    buf.extend_from_slice(&end);

    pipeline.add_ingress_power_ranger_entry(
        "forward",
        &buf,
        &0u16.to_le_bytes(),
    );

    let begin = v4_range_key("6.0.0.0".parse().unwrap());
    let end = v4_range_key("8.0.0.0".parse().unwrap());
    let mut buf = begin.to_vec();
    buf.extend_from_slice(&end);

    pipeline.add_ingress_power_ranger_entry(
        "forward",
        &buf,
        &1u16.to_le_bytes(),
    );

    let begin = v4_range_key("10.0.0.0".parse().unwrap());
    let end = v4_range_key("12.0.0.0".parse().unwrap());
    let mut buf = begin.to_vec();
    buf.extend_from_slice(&end);

    pipeline.add_ingress_power_ranger_entry(
        "forward",
        &buf,
        &2u16.to_le_bytes(),
    );

    let begin = v4_range_key("14.0.0.0".parse().unwrap());
    let end = v4_range_key("16.0.0.0".parse().unwrap());
    let mut buf = begin.to_vec();
    buf.extend_from_slice(&end);

    pipeline.add_ingress_power_ranger_entry(
        "forward",
        &buf,
        &3u16.to_le_bytes(),
    );

    //
    // run program
    //

    let mut npu = SoftNpu::new(4, pipeline, false);
    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);
    let phy3 = npu.phy(3);

    let if0 = Interface4::new(phy0.clone(), "1.0.0.1".parse().unwrap());
    let if1 = Interface4::new(phy1.clone(), "1.0.0.2".parse().unwrap());
    let if2 = Interface4::new(phy2.clone(), "1.0.0.3".parse().unwrap());
    let if3 = Interface4::new(phy3.clone(), "1.0.0.4".parse().unwrap());
    let msg = b"muffins!";

    npu.run();

    if1.send(phy1.mac, "11.0.0.0".parse().unwrap(), msg)?;
    expect_frames!(phy2, &[RxFrame::new(phy1.mac, 0x0800, msg)]);

    if0.send(phy0.mac, "14.0.0.1".parse().unwrap(), msg)?;
    expect_frames!(phy3, &[RxFrame::new(phy0.mac, 0x0800, msg)]);

    if3.send(phy3.mac, "7.7.7.7".parse().unwrap(), msg)?;
    expect_frames!(phy1, &[RxFrame::new(phy3.mac, 0x0800, msg)]);

    if2.send(phy2.mac, "3.4.7.7".parse().unwrap(), msg)?;
    expect_frames!(phy0, &[RxFrame::new(phy2.mac, 0x0800, msg)]);

    Ok(())
}
