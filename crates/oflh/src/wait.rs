//! Headless waiting for files, folders, and local ports.
use crate::{CliError, check};
use oflh_tui::{Language, StartOptions};
use std::{ffi::OsString, time::Duration};

pub const EXIT_TIMEOUT: u8 = 4;
pub const EXIT_CANCELLED: u8 = 130;

pub struct Options {
    pub language: Language,
    pub path: Option<OsString>,
    pub ports: bool,
    pub port: Option<u16>,
    pub follow_port_folder: bool,
    pub timeout: Option<Duration>,
    pub poll_interval: Duration,
}
impl Options {
    pub fn from_view(
        view: &StartOptions,
        language: Language,
        path: Option<OsString>,
        timeout: Option<Duration>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            language,
            path,
            ports: view.ports,
            port: view.port,
            follow_port_folder: view.follow_port_folder,
            timeout,
            poll_interval,
        }
    }
}

pub fn run(options: Options) -> Result<(), CliError> {
    let target = if options.ports && options.follow_port_folder {
        None
    } else {
        Some(oflh_core::Target::new(
            options.path.clone().unwrap_or_else(|| OsString::from(".")),
        )?)
    };
    let mut backend = oflh_platform::native()?;
    let cancel = oflh_core::Cancellation::default();
    let interrupt = cancel.clone();
    ctrlc::set_handler(move || interrupt.cancel())
        .map_err(|error| CliError::Wait(format!("install wait cancellation handler: {error}")))?;
    oflh_platform::wait_until_free(
        backend.as_mut(),
        oflh_platform::WaitTarget {
            path: target.as_ref(),
            include_ports: options.ports || options.port.is_some(),
            port: options.port,
            scope_ports_to_path: false,
        },
        oflh_core::wait::Options {
            poll_interval: options.poll_interval,
            timeout: options.timeout,
        },
        std::time::Instant::now(),
        &cancel,
    )?;
    println!(
        "{}",
        crate::messages::choose(
            options.language,
            "FREE: the target has no reported holders.",
            "FREI: Für das Ziel wurden keine Nutzer gefunden.",
            "空闲：未发现占用该目标的进程。",
        )
    );
    Ok(())
}

pub fn error_exit_code(error: &CliError) -> u8 {
    match error {
        CliError::Usage(_) => check::EXIT_USAGE,
        CliError::Core(oflh_core::Error::Cancelled) => EXIT_CANCELLED,
        CliError::Core(oflh_core::Error::WaitTimedOut) => EXIT_TIMEOUT,
        _ => check::EXIT_INSPECTION_FAILED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_and_interrupt_have_stable_distinct_wait_codes() {
        assert_eq!(
            error_exit_code(&CliError::Core(oflh_core::Error::WaitTimedOut)),
            4
        );
        assert_eq!(
            error_exit_code(&CliError::Core(oflh_core::Error::Cancelled)),
            130
        );
        assert_eq!(error_exit_code(&CliError::Usage("bad".into())), 2);
    }
}
