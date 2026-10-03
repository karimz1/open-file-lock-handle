fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::build();

    #[cfg(feature = "updater-tests")]
    embed_updater_test_manifest();
}

#[cfg(feature = "updater-tests")]
fn embed_updater_test_manifest() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    // Tauri's app manifest does not reach standalone integration-test executables.
    // Mock app construction links Common Controls v6 APIs; the Windows loader needs
    // this dependency before the Rust test harness can start.
    // See https://github.com/tauri-apps/tauri/issues/11028.
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/windows-updater.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
