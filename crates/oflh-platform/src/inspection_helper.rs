//! Headless Windows inspection, dispatched before terminal or desktop startup.
#[cfg(not(windows))]
use oflh_core::Error;
use oflh_core::Result;
use std::{ffi::OsString, path::PathBuf};

/// Executable and arguments that start a read-only native inspection helper.
/// Custom embedding binaries must dispatch [`run_stdio`] before their normal UI.
#[derive(Clone, Debug)]
pub struct InspectionHelperCommand {
    pub(crate) executable: PathBuf,
    pub(crate) arguments: Vec<OsString>,
}
impl InspectionHelperCommand {
    /// Executable used by this helper configuration.
    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }
    /// Arguments passed before the private request is written to stdin.
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
    /// Configure an embedding executable's headless entry point.
    pub fn new(executable: PathBuf, arguments: Vec<OsString>) -> Self {
        Self {
            executable,
            arguments,
        }
    }
    #[cfg(windows)]
    pub(crate) fn current() -> Result<Self> {
        Ok(Self::new(
            std::env::current_exe()
                .map_err(|error| oflh_core::io("locate inspection helper executable", error))?,
            vec![OsString::from("--oflh-inspection-helper")],
        ))
    }
}

/// Run the headless helper when its hidden argument is present, otherwise return `None`.
/// Call this before interpreting arguments or initializing a terminal/WebView.
pub fn dispatch() -> Option<Result<()>> {
    #[cfg(windows)]
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--oflh-inspection-helper"))
    {
        return Some(run_stdio());
    }
    None
}

/// Run the private, versioned inspection protocol on stdin/stdout without a UI.
/// The operation only observes processes and files; it performs no process actions.
pub fn run_stdio() -> Result<()> {
    #[cfg(windows)]
    {
        crate::windows::handles::run_stdio()
    }
    #[cfg(not(windows))]
    {
        Err(Error::Unavailable(
            "native handle helper requires Windows".into(),
        ))
    }
}
