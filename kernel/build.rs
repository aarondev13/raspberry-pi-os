use std::{env, path::PathBuf, process::Command};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let boot_object = out_dir.join("boot.o");
    let boot_archive = out_dir.join("libboot.a");

    let status = Command::new("clang")
        .args(["-c", "asm/boot.S", "-o"])
        .arg(&boot_object)
        .arg("--target=aarch64-none-elf")
        .status()
        .expect("Could not run clang");
    assert!(status.success(), "Assembly compilation failed");

    let status = Command::new("ar")
        .args(["crs"])
        .arg(&boot_archive)
        .arg(&boot_object)
        .status()
        .expect("Could not run ar");
    assert!(status.success(), "Could not create assembly archive");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=boot");
    println!("cargo:rerun-if-changed=asm/boot.S");
    println!("cargo:rerun-if-changed=linker/kernel.ld");
}
