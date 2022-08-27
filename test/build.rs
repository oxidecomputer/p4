fn main() {
    let src = [
        "src/p4/router.p4",
        "src/p4/hub.p4",
        "test/src/p4/dynamic_router.p4",
        "test/src/p4/dynamic_router_noaddr.p4",
        "test/src/p4/dynamic_router_noaddr_nbr.p4",
        "test/src/p4/router.p4",
        "test/src/p4/sidecar-lite.p4",
    ];
    for x in src {
        println!("cargo:rerun-if-changed={}", x);
    }
}
