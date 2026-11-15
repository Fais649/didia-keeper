// Compiles the Didia patch (an Objective-C dylib) that Didia Keeper places inside Didia.
use std::{env, path::PathBuf, process::Command};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("libdidia.dylib");
    println!("cargo:rerun-if-changed=patch/didia_patch.m");
    let status = Command::new("clang")
        .args([
            "-arch", "arm64", "-dynamiclib", "-fobjc-arc", "-O2",
            "-mmacosx-version-min=13.0",
            "-framework", "Cocoa", "-framework", "QuartzCore", "-Wno-objc-string-concatenation",
            "-install_name", "@executable_path/../Frameworks/libdidia.dylib",
            "-o",
        ])
        .arg(&out)
        .arg("patch/didia_patch.m")
        .status()
        .expect("clang is needed to build the Didia patch");
    assert!(status.success(), "building the Didia patch failed");
}
