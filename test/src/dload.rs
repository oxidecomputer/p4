p4_macro::use_p4!(
    p4 = "test/src/p4/dynamic_router_noaddr_nbr.p4",
    pipeline_name = "dload",
);

#[test]
fn pipeline_create() -> Result<(), anyhow::Error> {
    let p = unsafe { &mut *_dload_pipeline_create() };

    let port: u16 = 47;
    let data = [0u8; 500];
    let mut pkt: packet_in = packet_in {
        data: &data,
        index: 0,
    };

    // the goal is simply not to explode
    let result = p.process_packet(port, &mut pkt);
    assert!(result.is_none());

    Ok(())
}

use p4rs::packet_in;

#[test]
fn dynamic_load() -> Result<(), anyhow::Error> {
    // see .cargo/config.toml
    let ws = std::env::var("CARGO_WORKSPACE_DIR").unwrap();
    let path = format!("{}/target/debug/libsidecar_lite.so", ws);
    let lib = match unsafe { libloading::Library::new(&path) } {
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

    let port: u16 = 47;
    let data = [0u8; 500];
    let mut pkt: packet_in = packet_in {
        data: &data,
        index: 0,
    };

    // the goal is simply not to explode
    let result = p.process_packet(port, &mut pkt);
    assert!(result.is_none());

    Ok(())
}
