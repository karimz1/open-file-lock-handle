//! Launch requests from the command line, such as the Windows Explorer
//! **Inspect file** and **Inspect folder** context menu entries.
use std::{ffi::OsString, path::PathBuf};

/// Flag that precedes the target path. The NSIS installer registers
/// `"<exe>" --inspect "%1"` (and `"%V"` for a folder background) with Explorer.
pub const INSPECT_FLAG: &str = "--inspect";

/// Return the target that `--inspect <path>` asks to open at startup.
///
/// `arguments` includes the program name, as from [`std::env::args_os`]. Without
/// the flag, or without a non-empty path after it, the app starts normally. The
/// path stays an [`OsString`] so non-Unicode names survive.
pub fn inspect_target(arguments: impl IntoIterator<Item = OsString>) -> Option<PathBuf> {
    inspect_target_for(arguments, cfg!(windows))
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
}
