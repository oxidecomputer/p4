use std::net::Ipv4Addr;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;
use tests::dynamic_softnpu::{Interface4, SoftNpu};
use tests::management::SwitchManagementRequest;

mod switch_program {
    p4_macro::use_p4!(p4 = "test/src/p4/switch.p4", pipeline_name = "main_pipeline");
}

mod user_program {
    p4_macro::use_p4!(p4 = "test/src/p4/user_program.p4", pipeline_name = "user_program");
}

const SOCKET_PATH: &str = "/tmp/switch-admin.sock";
const BASIC_UDP_PORT: u16 = 4000;
const ET: u16 = 0x0800;

fn main() {
    let mut npu = SoftNpu::new(3, switch_program::main_pipeline::new(3), false);

    let phy0 = npu.phy(0);
    let phy1 = npu.phy(1);
    let phy2 = npu.phy(2);

    npu.run();

    // background traffic generator -- runs forever, independent of CLI commands
    std::thread::spawn(move || {
        let pkt0 = Interface4::new(phy0.clone(), Ipv4Addr::new(10, 0, 0, 1));
        loop {
            pkt0.send_udp(phy1.mac, Ipv4Addr::new(10, 0, 0, 2), BASIC_UDP_PORT, b"UDP pkt sent")
                .ok();
            pkt0.send_geneve(phy1.mac, Ipv4Addr::new(10, 0, 0, 2), phy1.mac, phy0.mac, ET, b"Geneve pkt sent")
                .ok();

            std::thread::sleep(Duration::from_millis(500));

            if phy1.recv_buffer_len() > 0 {
                let frames = phy1.recv();
                println!("[phy1] Received {} UDP pkt", frames.len());
            }
            if phy2.recv_buffer_len() > 0 {
                let frames = phy2.recv();
                println!("[phy2] Received {} Geneve pkt -- user_program active", frames.len());
            } else {
                println!("[phy2] No pkt received -- user_program not active, geneve traffic dropped");
            }
        }
    });

    // management socket -- runs concurrently, handles load/remove whenever they arrive
    let _remove_file = std::fs::remove_file(SOCKET_PATH);
    let sock = UnixDatagram::bind(SOCKET_PATH).expect("failed to bind management socket");
    println!("switch_daemon listening on {SOCKET_PATH}");

    loop {
        let mut buf = [0u8; 1024];
        let n = match sock.recv(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("recv error: {e}");
                continue;
            }
        };

        let req: SwitchManagementRequest = match serde_json::from_slice(&buf[..n]) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("bad request: {e}");
                continue;
            }
        };

        match req {
            SwitchManagementRequest::ProgramLoad(req) => {
                npu.load_user_program(req.program_id, user_program::main_pipeline::new(3));
                println!(">> loaded program {}", req.program_id);
            }
            SwitchManagementRequest::ProgramRemove(req) => {
                npu.remove_user_program(req.program_id);
                println!(">> removed program {}", req.program_id);
            }
        }
    }
}
