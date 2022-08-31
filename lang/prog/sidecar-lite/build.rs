fn main() {
    let src = ["../../../test/src/p4/sidecar-lite.p4"];
    for x in src {
        println!("cargo:rerun-if-changed={}", x);
    }
}
