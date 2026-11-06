//! CI supplies the built application (and an installed XDG desktop entry on
//! Linux), so this exercises native lookup against real application artwork.
#[test]
#[ignore = "requires OFLH_ICON_SMOKE_EXECUTABLE pointing to a built/installed app"]
fn native_application_icon_is_renderable() {
    let executable = std::env::var_os("OFLH_ICON_SMOKE_EXECUTABLE")
        .expect("CI must supply the built application path");
    let executable = std::path::Path::new(&executable);
    assert!(executable.is_file(), "built application is missing");
    let icon =
        oflh_icons::native_icon(executable).expect("built app must have its application icon");
    assert!(!icon.bytes.is_empty());
    assert!(icon.bytes.len() <= oflh_icons::MAX_ICON_BYTES);
    match icon.mime {
        "image/png" => assert!(icon.bytes.starts_with(b"\x89PNG\r\n\x1a\n")),
        "image/svg+xml" => assert!(std::str::from_utf8(&icon.bytes).unwrap().contains("<svg")),
        mime => panic!("unsupported icon MIME type {mime}"),
    }
    if let Some(output) = std::env::var_os("OFLH_ICON_DATA_URL_FILE") {
        std::fs::write(output, icon.data_url()).unwrap();
    }
    assert!(
        icon.data_url()
            .starts_with(&format!("data:{};base64,", icon.mime))
    );
}
