use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=src/native.m");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let object = out.join("native.o");
    let status = Command::new("xcrun")
        .args([
            "clang",
            "-fobjc-arc",
            "-fblocks",
            "-c",
            "src/native.m",
            "-o",
        ])
        .arg(&object)
        .status()
        .expect("compile native bridge");
    assert!(status.success(), "native bridge compilation failed");
    let status = Command::new("ar")
        .arg("crs")
        .arg(out.join("libkeepline_native.a"))
        .arg(object)
        .status()
        .expect("archive native bridge");
    assert!(status.success(), "native bridge archive failed");
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=keepline_native");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=UserNotifications");
}
