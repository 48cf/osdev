use std::path::PathBuf;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(|p| PathBuf::from(p))
        .unwrap();

    let linker_script_path = manifest_dir.join("linker.ld");

    println!("cargo:rustc-link-arg=-T{}", linker_script_path.display());
    println!("cargo:rerun-if-changed={}", linker_script_path.display());
}
