fn main() {
    let src = [
        "src/bin/hello-world.p4",
        "src/bin/vlan-switch.p4",
        "src/bin/headers.p4",
        "src/bin/softnpu.p4",
        "src/bin/core.p4",
    ];
    for x in src {
        println!("cargo:rerun-if-changed={}", x);
    }
}
