#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("oflh-desktop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    oflh_desktop::startup::configure_renderer()?;
    oflh_desktop::shell::run()
}
