fn main() {
    slint_build::compile("ui/main.slint").expect("Slint build failed");
    resources();
}

/// Icon, manifest and version info for the program (res/app.rc), compiled with windres from
/// the GNU toolchain (llvm-mingw). Linked into the binaries only, not the library or tests.
fn resources() {
    println!("cargo:rerun-if-changed=res");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("app-res.o");
    let ok = ["windres", "llvm-windres"].iter().any(|tool| {
        std::process::Command::new(tool)
            .current_dir("res")
            .args(["-i", "app.rc", "-O", "coff", "-o"])
            .arg(&out)
            .status()
            .is_ok_and(|s| s.success())
    });
    if ok {
        println!("cargo:rustc-link-arg-bins={}", out.display());
    } else {
        println!("cargo:warning=windres not found: fluentcal.exe is built without icon and manifest");
    }
}
