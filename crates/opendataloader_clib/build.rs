use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");
    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo");
    let header = PathBuf::from(&crate_dir).join("include/opendataloder_clib.h");
    std::fs::create_dir_all(header.parent().expect("header has a parent")).expect("create header directory");
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("generate C header")
        .write_to_file(header);
}
