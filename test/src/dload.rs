p4_macro::use_p4!(
    p4 = "test/src/p4/dynamic_router_noaddr_nbr.p4",
    pipeline_name = "dload",
);

#[test]
fn pipeline_create() -> Result<(), anyhow::Error> {
    let p = unsafe { &mut *_dload_pipeline_create() };

    let port: u8 = 47;
    let data = [0u8; 500];
    let mut pkt: packet_in = packet_in {
        data: &data,
        index: 0,
    };

    let result = p.process_packet(port, &mut pkt);
    println!("{:#?}", result);

    Ok(())
}

use p4rs::packet_in;

#[test]
fn dynamic_load() -> Result<(), anyhow::Error> {
    let lib = match unsafe { libloading::Library::new("/tmp/p4.so") } {
        Ok(l) => l,
        Err(e) => {
            panic!("failed to load p4 program: {}", e);
        }
    };
    let func: libloading::Symbol<
        unsafe extern "C" fn() -> *mut dyn p4rs::Pipeline,
    > = match unsafe { lib.get(b"_main_pipeline_create") } {
        Ok(f) => f,
        Err(e) => {
            panic!("failed to load _main_pipeline_create func: {}", e);
        }
    };

    let mut p = unsafe { Box::from_raw(func()) };

    let port: u8 = 47;
    let data = [0u8; 500];
    let mut pkt: packet_in = packet_in {
        data: &data,
        index: 0,
    };

    let result = p.process_packet(port, &mut pkt);
    println!("{:#?}", result);

    Ok(())
}
