//! Launch requests from the command line, such as the Windows Explorer
//! **Inspect file** and **Inspect folder** context menu entries.
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Flag that precedes the target path. The NSIS installer registers
/// `"<exe>" --inspect "%1"` (and `"%V"` for a folder background) with Explorer.
pub const INSPECT_FLAG: &str = "--inspect";

/// Internal flag for an independent window that bypasses Windows launch forwarding.
pub const NEW_WINDOW_FLAG: &str = "--new-window";

/// Whether the first startup argument explicitly requests an independent window.
///
/// `arguments` includes the program name. Only the first option is considered so
/// an Explorer target named `--new-window` cannot bypass launch forwarding.
pub fn new_window_requested(arguments: impl IntoIterator<Item = OsString>) -> bool {
    arguments.into_iter().nth(1).as_deref() == Some(std::ffi::OsStr::new(NEW_WINDOW_FLAG))
}

/// Return the target that `--inspect <path>` asks to open at startup.
///
/// `arguments` includes the program name, as from [`std::env::args_os`]. Without
/// the flag, or without a non-empty path after it, the app starts normally. The
/// path stays an [`OsString`] so non-Unicode names survive.
pub fn inspect_target(arguments: impl IntoIterator<Item = OsString>) -> Option<PathBuf> {
    inspect_target_for(arguments, cfg!(windows))
}

/// Return the target another launch forwarded to this running window.
///
/// On Windows the single-instance plugin hands over the second launch's arguments
/// (program name first) and working directory as text. A relative path is resolved
/// against that directory, because this process may run elsewhere.
pub fn forwarded_target(arguments: Vec<String>, working_directory: &str) -> Option<PathBuf> {
    forwarded_target_for(arguments, working_directory, cfg!(windows))
}

fn forwarded_target_for(
    arguments: Vec<String>,
    working_directory: &str,
    windows: bool,
) -> Option<PathBuf> {
    let path = inspect_target_for(arguments.into_iter().map(OsString::from), windows)?;
    if path.is_relative() && !working_directory.is_empty() {
        Some(Path::new(working_directory).join(path))
    } else {
        Some(path)
    }
}

fn inspect_target_for(
    arguments: impl IntoIterator<Item = OsString>,
    windows: bool,
) -> Option<PathBuf> {
    let mut arguments = arguments.into_iter().skip(1);
    arguments.find(|argument| argument == INSPECT_FLAG)?;
    let path = arguments.next().filter(|path| !path.is_empty())?;
    Some(PathBuf::from(if windows {
        repair_quoted_trailing_backslash(path)
    } else {
        path
    }))
}

/// Undo the Windows command-line rule that turns `\"` into a literal quote.
///
/// Explorer substitutes a drive root as `C:\`, so the registered `"%V"` becomes
/// `"C:\"`, which Windows argument parsing reads as `C:"`. Windows file names
/// cannot contain `"`, so a trailing quote can only be this lost backslash.
fn repair_quoted_trailing_backslash(path: OsString) -> OsString {
    match path.to_str() {
        Some(text) if text.ends_with('"') => {
            let mut repaired = text.trim_end_matches('"').to_owned();
            repaired.push('\\');
            OsString::from(repaired)
        }
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_explicit_new_windows_bypass_launch_forwarding() {
        assert!(new_window_requested(arguments(&[
            "oflh-desktop",
            NEW_WINDOW_FLAG
        ])));
        assert!(new_window_requested(arguments(&[
            "oflh-desktop",
            NEW_WINDOW_FLAG,
            "--inspect-target",
            "/build/out"
        ])));
        for values in [
            &["oflh-desktop"][..],
            &["oflh-desktop", INSPECT_FLAG, "/build/out"],
            &["oflh-desktop", INSPECT_FLAG, NEW_WINDOW_FLAG],
        ] {
            assert!(!new_window_requested(arguments(values)));
        }
    }

    #[test]
    fn explorer_flag_selects_the_following_path() {
        assert_eq!(
            inspect_target_for(
                arguments(&[
                    "oflh-desktop.exe",
                    "--inspect",
                    r"C:\Users\alice\My Files\report.docx"
                ]),
                true
            ),
            Some(PathBuf::from(r"C:\Users\alice\My Files\report.docx"))
        );
        assert_eq!(
            inspect_target_for(
                arguments(&["oflh-desktop", "--inspect", "/build/out"]),
                false
            ),
            Some(PathBuf::from("/build/out"))
        );
    }

    #[test]
    fn missing_flag_or_empty_path_starts_without_a_target() {
        for values in [
            &["oflh-desktop.exe"][..],
            &["oflh-desktop.exe", r"C:\build"],
            &["oflh-desktop.exe", "--inspect"],
            &["oflh-desktop.exe", "--inspect", ""],
            &["--inspect", r"C:\program-name-is-not-a-flag"],
        ] {
            assert_eq!(
                inspect_target_for(arguments(values), true),
                None,
                "{values:?}"
            );
        }
    }

    #[test]
    fn quoted_drive_root_regains_its_backslash_on_windows_only() {
        assert_eq!(
            inspect_target_for(arguments(&["oflh-desktop.exe", "--inspect", "C:\""]), true),
            Some(PathBuf::from(r"C:\"))
        );
        assert_eq!(
            inspect_target_for(
                arguments(&["oflh-desktop.exe", "--inspect", r#"\\server\share\folder""#]),
                true
            ),
            Some(PathBuf::from(r"\\server\share\folder\"))
        );
        assert_eq!(
            inspect_target_for(
                arguments(&["oflh-desktop", "--inspect", "/build/odd\""]),
                false
            ),
            Some(PathBuf::from("/build/odd\""))
        );
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn forwarded_launch_resolves_relative_paths_against_its_own_directory() {
        assert_eq!(
            forwarded_target_for(
                strings(&["oflh-desktop", "--inspect", "out/app.log"]),
                "/build",
                false
            ),
            Some(PathBuf::from("/build/out/app.log"))
        );
        assert_eq!(
            forwarded_target_for(
                strings(&["oflh-desktop", "--inspect", "/build/out"]),
                "/elsewhere",
                false
            ),
            Some(PathBuf::from("/build/out"))
        );
        assert_eq!(
            forwarded_target_for(
                strings(&["oflh-desktop", "--inspect", "relative"]),
                "",
                false
            ),
            Some(PathBuf::from("relative"))
        );
    }

    #[test]
    fn forwarded_launch_without_a_target_only_focuses_and_drive_roots_are_repaired() {
        assert_eq!(
            forwarded_target_for(strings(&["oflh-desktop.exe"]), r"C:\Users", true),
            None
        );
        assert_eq!(
            forwarded_target_for(
                strings(&["oflh-desktop.exe", "--inspect", "D:\""]),
                // Host path rules decide relativity; an empty directory skips joining.
                "",
                true
            )
            .map(|path| path.into_os_string()),
            Some(OsString::from(r"D:\"))
        );
    }
}
