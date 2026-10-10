//! Launch an independent desktop process without serializing native target paths.
use crate::contract::Failure;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
};

const INSPECT_TARGET: &str = "--inspect-target";

/// Parse the internal startup handoff, retaining the target as an OS string.
pub(crate) fn startup_target(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Option<PathBuf>, Failure> {
    let mut arguments = arguments.into_iter();
    let Some(flag) = arguments.next() else {
        return Ok(None);
    };
    let target = arguments.next();
    if flag != OsStr::new(INSPECT_TARGET)
        || target.as_ref().is_none_or(|path| path.is_empty())
        || arguments.next().is_some()
    {
        return Err(Failure::invalid(
            "Expected --inspect-target followed by one file or folder",
        ));
    }
    Ok(target.map(PathBuf::from))
}

fn command(executable: &Path, target: Option<&Path>) -> Command {
    let mut command = Command::new(executable);
    command.arg(crate::launch::NEW_WINDOW_FLAG);
    if let Some(target) = target {
        command.arg(INSPECT_TARGET).arg(target);
    }
    command
}

/// Start another app instance and reap it off the UI thread after it exits.
#[cfg(feature = "desktop")]
pub(crate) fn launch(target: Option<PathBuf>) -> Result<(), Failure> {
    let executable = std::env::current_exe()
        .map_err(|error| oflh_core::io("locate the desktop executable", error))?;
    spawn_and_reap(command(&executable, target.as_deref()))
}

#[cfg(any(feature = "desktop", test))]
fn spawn_and_reap(mut command: Command) -> Result<(), Failure> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    // Create the reaper first so a thread-creation failure cannot leave a child behind.
    std::thread::Builder::new()
        .name("desktop-window".into())
        .spawn(move || match command.spawn() {
            Ok(mut child) => {
                let _ = sender.send(Ok(()));
                let _ = child.wait();
            }
            Err(error) => {
                let _ = sender.send(Err(Failure::from(oflh_core::io(
                    "open a new window",
                    error,
                ))));
            }
        })
        .map_err(|error| oflh_core::io("start the window launcher", error))?;
    receiver
        .recv()
        .map_err(|_| Failure::invalid("The window launcher stopped before opening a window"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_windows_request_an_independent_instance_without_a_target() {
        let command = command(Path::new("oflh-desktop"), None);
        assert_eq!(command.get_program(), OsStr::new("oflh-desktop"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [OsStr::new(crate::launch::NEW_WINDOW_FLAG)]
        );
        assert!(crate::launch::new_window_requested(
            std::iter::once(command.get_program())
                .chain(command.get_args())
                .map(OsStr::to_os_string)
        ));
        assert_eq!(startup_target([]).unwrap(), None);
    }

    #[test]
    fn target_arguments_are_lossless_and_do_not_interpret_paths_as_options() {
        for target in [
            "/workspace/project with spaces",
            "--version",
            "--new-window",
            "--inspect",
            "/workspace/$report.txt",
        ] {
            let path = Path::new(target);
            let command = command(Path::new("oflh-desktop"), Some(path));
            assert!(crate::launch::new_window_requested(
                std::iter::once(command.get_program())
                    .chain(command.get_args())
                    .map(OsStr::to_os_string)
            ));
            let arguments: Vec<_> = command
                .get_args()
                .skip(1)
                .map(OsStr::to_os_string)
                .collect();
            assert_eq!(arguments, [OsString::from(INSPECT_TARGET), path.into()]);
            assert_eq!(startup_target(arguments).unwrap(), Some(path.into()));
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_drops_reach_the_new_instance_unchanged() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(OsString::from_vec(b"/workspace/file-\xff".to_vec()));
        let command = command(Path::new("oflh-desktop"), Some(&path));
        assert_eq!(
            startup_target(command.get_args().skip(1).map(OsStr::to_os_string)).unwrap(),
            Some(path)
        );
    }

    #[test]
    fn malformed_startup_requests_are_rejected() {
        for arguments in [
            vec!["--unknown"],
            vec![INSPECT_TARGET],
            vec![INSPECT_TARGET, ""],
            vec![INSPECT_TARGET, "/workspace/project", "extra"],
        ] {
            assert!(startup_target(arguments.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn launch_failure_keeps_operation_context_and_os_error() {
        let directory = tempfile::tempdir().unwrap();
        let error =
            spawn_and_reap(Command::new(directory.path().join("missing-executable"))).unwrap_err();
        assert_eq!(error.kind, "not_found");
        assert!(error.os_code.is_some());
        assert!(error.message.contains("open a new window"));
    }

    #[test]
    fn launches_an_independent_process_without_waiting_for_it_to_exit() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("child-pid");
        let release = directory.path().join("release");
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "instance::tests::window_launch_fixture"])
            .env("OFLH_TEST_WINDOW_PID", &marker)
            .env("OFLH_TEST_WINDOW_RELEASE", &release)
            .stdout(std::process::Stdio::null());
        spawn_and_reap(child).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let pid = std::fs::read_to_string(marker).unwrap();
        // Release the child even if the identity assertion fails.
        std::fs::write(release, b"done").unwrap();
        assert_ne!(pid, std::process::id().to_string());
    }

    #[test]
    fn window_launch_fixture() {
        let Some(marker) = std::env::var_os("OFLH_TEST_WINDOW_PID") else {
            return;
        };
        let release = PathBuf::from(std::env::var_os("OFLH_TEST_WINDOW_RELEASE").unwrap());
        std::fs::write(marker, std::process::id().to_string()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !release.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(release.exists());
    }
}
