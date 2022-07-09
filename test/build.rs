fn main() {
    let src = "src/p4/router.p4";
    println!("cargo:rerun-if-changed={}", src);
}
