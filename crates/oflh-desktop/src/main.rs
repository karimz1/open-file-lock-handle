#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = oflh_platform::inspection_helper::dispatch() {
        return Ok(result?);
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--version")) {
        println!("{}", oflh_core::version_report("oflh-desktop"));
        return Ok(());
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--admin-terminate")) {
        let arguments: Vec<String> = std::env::args().skip(2).collect();
        let code = oflh_platform::elevation::helper_outcome(oflh_platform::elevation::run_helper(
            &arguments,
        ));
        #[cfg(windows)]
        std::process::exit(code as i32);
        #[cfg(not(windows))]
        {
            println!("{code}");
            return Ok(());
        }
    }
    oflh_desktop::startup::configure_renderer()?;
    oflh_desktop::shell::run()
}
