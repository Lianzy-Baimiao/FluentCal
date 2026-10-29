fn main() {
    tauri_build::build();
    #[cfg(windows)]
    embed_test_manifest();
}

/// tauri-build embeds the application manifest (common controls v6) only into binaries.
/// Test executables link the same Win32 imports (e.g. TaskDialogIndirect) and fail to start
/// without it, so give integration tests the same manifest.
#[cfg(windows)]
fn embed_test_manifest() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = out.join("test-app.manifest");
    std::fs::write(
        &manifest,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
    )
    .unwrap();
    let rc = out.join("test-manifest.rc");
    let path = manifest.display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 24 \"{path}\"\n")).unwrap();
    let _ = embed_resource::compile_for_tests(&rc, embed_resource::NONE).manifest_optional();
}
