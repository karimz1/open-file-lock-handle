//! Portable Windows layout: a marker file next to the executable keeps
//! settings and WebView data in a `data` folder in the same directory.
// The helpers are used from the desktop shell. Workspace lint without that
// feature still type-checks this module, so unused items are expected.
#![cfg_attr(not(feature = "desktop"), allow(dead_code))]
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Empty file that switches on portable mode when it sits next to the executable.
pub(crate) const MARKER_NAME: &str = "portable";

/// Relative app-directory root used while portable. A subfolder of the
/// executable directory, never `"./"` itself, so WebView scopes cannot rewrite
/// files next to the executable.
pub(crate) const DIRECTORIES_OVERRIDE: &str = "./data";

/// Whether `directory` contains the portable marker file.
pub(crate) fn marker_present(directory: &Path) -> bool {
    directory.join(MARKER_NAME).is_file()
}

/// App-directory override to apply when portable, or `None` when the marker is absent.
pub(crate) fn directories_override(directory: &Path) -> Option<&'static str> {
    marker_present(directory).then_some(DIRECTORIES_OVERRIDE)
}

/// Directory that holds settings, the recent-target database, and WebView data.
pub(crate) fn data_dir(exe_dir: &Path) -> PathBuf {
    exe_dir.join("data")
}

/// Updater behaviour for a given OS and portable flag. Linux and portable
/// Windows only offer a download link, because there is no in-place installer
/// to apply.
pub(crate) fn update_mode_for(linux: bool, portable: bool) -> &'static str {
    if linux || portable {
        "download"
    } else {
        "install"
    }
}

/// Updater behaviour of this build.
pub(crate) fn update_mode(portable: bool) -> &'static str {
    update_mode_for(cfg!(target_os = "linux"), portable)
}

/// Bundle identifier used for the single-instance mutex. Portable copies
/// include the executable directory so they do not attach to an installed
/// instance, or to another portable folder.
pub(crate) fn single_instance_identifier(base: &str, exe_dir: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    exe_dir.hash(&mut hasher);
    format!("{base}-p{:x}", hasher.finish())
}

/// Whether the running executable is in portable mode.
pub(crate) fn is_active() -> bool {
    exe_directory().is_some_and(|directory| marker_present(&directory))
}

pub(crate) fn exe_directory() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oflh-portable-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn marker_file_enables_the_data_subdirectory_override() {
        let dir = unique_dir("marker");
        assert!(!marker_present(&dir));
        assert_eq!(directories_override(&dir), None);
        fs::write(dir.join(MARKER_NAME), b"").unwrap();
        assert!(marker_present(&dir));
        assert_eq!(directories_override(&dir), Some("./data"));
        assert_ne!(directories_override(&dir), Some("./"));
        assert_eq!(directories_override(&dir), Some(DIRECTORIES_OVERRIDE));
        assert_eq!(data_dir(&dir), dir.join("data"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_data_folder_alone_does_not_enable_portable_mode() {
        let dir = unique_dir("data-only");
        fs::create_dir(dir.join("data")).unwrap();
        assert!(!marker_present(&dir));
        assert_eq!(directories_override(&dir), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn portable_windows_builds_only_offer_a_download_link() {
        assert_eq!(update_mode_for(false, true), "download");
        assert_eq!(update_mode_for(false, false), "install");
        assert_eq!(update_mode_for(true, false), "download");
        assert_eq!(update_mode_for(true, true), "download");
        assert_eq!(update_mode(true), "download");
        if cfg!(target_os = "linux") {
            assert_eq!(update_mode(false), "download");
        } else {
            assert_eq!(update_mode(false), "install");
        }
    }

    #[test]
    fn single_instance_identifier_isolates_portable_folders() {
        let first = unique_dir("instance-a");
        let second = unique_dir("instance-b");
        let base = "io.github.karimz1.oflh";
        let a = single_instance_identifier(base, &first);
        let b = single_instance_identifier(base, &second);
        let again = single_instance_identifier(base, &first);
        assert_eq!(a, again);
        assert_ne!(a, b);
        assert!(a.starts_with(base));
        assert!(b.starts_with(base));
        assert!(
            a.chars().all(|character| character.is_ascii_alphanumeric()
                || character == '.'
                || character == '-'),
            "{a}"
        );
        fs::remove_dir_all(first).unwrap();
        fs::remove_dir_all(second).unwrap();
    }
}
