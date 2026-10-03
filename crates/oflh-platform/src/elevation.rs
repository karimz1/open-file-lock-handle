//! Explicit native authorization for a single captured termination request.
//!
//! The executable runs a headless helper before initializing its WebView. Arguments
//! contain only numeric identity fields and an explicit mode; no PID-only retry is allowed.
use oflh_core::{Cancellation, Error, Identity, Result, io};
use std::path::Path;
#[cfg(unix)]
use std::process::Command;

/// Whether this process already has administrator/root privileges.
pub fn is_elevated() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid takes no pointers and has no preconditions.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(windows)]
    {
        // SAFETY: IsUserAnAdmin takes no arguments and checks enabled token membership.
        unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

/// Encode a helper outcome without losing a native OS error code.
/// Unix transports this number through stdout; Windows uses the process exit code.
pub fn helper_outcome(result: Result<()>) -> u32 {
    match result {
        Ok(()) => 0,
        Err(Error::Changed) => 1,
        Err(Error::Protected) => 2,
        Err(Error::Cancelled) => 3,
        Err(Error::Io { source, .. }) => source
            .raw_os_error()
            .and_then(|code| u32::try_from(code).ok())
            .and_then(|code| code.checked_add(256))
            .unwrap_or(4),
        Err(_) => 4,
    }
}
fn decode_outcome(code: u32) -> Result<()> {
    match code {
        0 => Ok(()),
        1 => Err(Error::Changed),
        2 => Err(Error::Protected),
        3 => Err(Error::Cancelled),
        256..=0x7fff_ffff => Err(io(
            "administrator termination",
            std::io::Error::from_raw_os_error((code - 256) as i32),
        )),
        _ => Err(Error::Unavailable(
            "Administrator helper could not complete the request".into(),
        )),
    }
}

/// Request OS authorization and run only the captured termination operation.
/// The helper rechecks identity using the normal backend and owns its native handle.
/// Authorization is requested separately for each target; cancellation stops the batch.
pub fn terminate(
    executable: &Path,
    identity: Identity,
    force: bool,
    cancel: &Cancellation,
) -> Result<()> {
    identity.validate()?;
    cancel.check()?;
    let arguments = [
        "--admin-terminate".to_string(),
        std::process::id().to_string(),
        identity.pid.to_string(),
        identity.started.to_string(),
        identity.started_sub.to_string(),
        u8::from(force).to_string(),
    ];
    launch(executable, &arguments).and_then(decode_outcome)
}

#[cfg(unix)]
fn output_code(output: std::process::Output) -> Result<u32> {
    if output.status.code() == Some(126)
        || (!output.status.success() && String::from_utf8_lossy(&output.stderr).contains("(-128)"))
    {
        return Err(Error::Cancelled);
    }
    if !output.status.success() {
        // Authentication refusal and unavailable agents must never cause another retry.
        return Err(Error::Unavailable(
            "Administrator authorization was cancelled or unavailable; no retry was sent".into(),
        ));
    }
    std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| Error::Unavailable("Invalid administrator helper response".into()))
}
#[cfg(target_os = "linux")]
fn launch(executable: &Path, arguments: &[String]) -> Result<u32> {
    output_code(
        Command::new("/usr/bin/pkexec")
            .arg(executable)
            .args(arguments)
            .output()
            .map_err(|error| io("request polkit authorization", error))?,
    )
}
#[cfg(target_os = "macos")]
fn launch(executable: &Path, arguments: &[String]) -> Result<u32> {
    let script = administrator_script(executable, arguments)?;
    output_code(
        Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| io("request administrator authorization", error))?,
    )
}
#[cfg(any(target_os = "macos", test))]
fn administrator_script(executable: &Path, arguments: &[String]) -> Result<String> {
    // AppleScript quotes the entire shell command; shell quoting separately preserves
    // spaces, quotes, dollar signs and backticks in the executable path.
    let path = executable.to_str().ok_or_else(|| {
        Error::Unavailable("Administrator authorization requires a UTF-8 executable path".into())
    })?;
    let command = format!("'{}' {}", path.replace('\'', "'\\''"), arguments.join(" "));
    Ok(format!(
        "do shell script \"{}\" with administrator privileges",
        command.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}
#[cfg(windows)]
fn launch(executable: &Path, arguments: &[String]) -> Result<u32> {
    super::windows::launch_elevated(executable, &arguments.join(" "))
}
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn launch(_: &Path, _: &[String]) -> Result<u32> {
    Err(Error::Unavailable(
        "Administrator authorization unavailable".into(),
    ))
}

/// Parse the headless helper's numeric-only arguments and perform one native action.
/// The requester PID remains protected even though the helper has a different PID.
pub fn run_helper(arguments: &[String]) -> Result<()> {
    if arguments.len() != 5 {
        return Err(Error::Unavailable(
            "Invalid administrator helper arguments".into(),
        ));
    }
    let invalid = || Error::Unavailable("Invalid administrator helper identity or mode".into());
    let requester: u32 = arguments[0].parse().map_err(|_| invalid())?;
    let identity = Identity {
        pid: arguments[1].parse().map_err(|_| invalid())?,
        started: arguments[2].parse().map_err(|_| invalid())?,
        started_sub: arguments[3].parse().map_err(|_| invalid())?,
    };
    let force = match arguments[4].as_str() {
        "0" => false,
        "1" => true,
        _ => return Err(invalid()),
    };
    identity.validate()?;
    if identity.pid == requester {
        return Err(Error::Protected);
    }
    super::native()?.terminate(identity, force, &Cancellation::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_retains_native_errors_and_safety_categories() {
        for code in [1, 5, 13, 87, 1223] {
            let encoded = helper_outcome(Err(io("test", std::io::Error::from_raw_os_error(code))));
            let Err(Error::Io { source, .. }) = decode_outcome(encoded) else {
                panic!("native error lost")
            };
            assert_eq!(source.raw_os_error(), Some(code));
        }
        assert!(matches!(
            decode_outcome(helper_outcome(Err(Error::Changed))),
            Err(Error::Changed)
        ));
        assert!(decode_outcome(0).is_ok());
        assert!(decode_outcome(5).is_err());
    }
    #[test]
    fn administrator_script_quotes_shell_and_applescript_separately() {
        let script = administrator_script(
            Path::new("/Applications/Fixture's \"$HOME`id`\\ app"),
            &["--admin-terminate".into(), "42".into()],
        )
        .unwrap();
        assert!(script.starts_with("do shell script \"'"));
        assert!(script.ends_with(" --admin-terminate 42\" with administrator privileges"));
        assert!(script.contains("Fixture'\\\\''s"));
        assert!(script.contains("\\\"$HOME`id`\\\\ app'"));
        #[cfg(target_os = "macos")]
        {
            let directory = tempfile::tempdir().unwrap();
            let output = Command::new("/usr/bin/osacompile")
                .arg("-o")
                .arg(directory.path().join("fixture.scpt"))
                .args(["-e", &script])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "generated AppleScript must compile without executing authorization"
            );
        }
    }
    #[test]
    fn helper_rejects_malformed_protected_and_requester_targets() {
        assert!(run_helper(&[]).is_err());
        for args in [
            ["42", "42", "1", "0", "1"],
            ["42", "1", "1", "0", "1"],
            ["42", "43", "0", "0", "1"],
        ] {
            assert!(matches!(
                run_helper(&args.map(String::from)),
                Err(Error::Protected)
            ));
        }
        assert!(run_helper(&["42", "43", "1", "0", "2"].map(String::from)).is_err());
    }
}
