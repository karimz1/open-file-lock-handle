//! Validated names and filesystem results for explicit waiting follow-ups.
#[cfg(any(feature = "desktop", test))]
use crate::contract::WaitFollowUpResult;
use crate::{contract::Failure, service::WaitFollowUp};
use oflh_core::{Cancellation, Target};
use std::path::{Path, PathBuf};

/// Finish an already successful action in reveal-then-notify order, retaining partial errors.
#[cfg(any(feature = "desktop", test))]
pub(crate) fn finish(
    action: String,
    path: Option<PathBuf>,
    reveal: impl FnOnce(&Path) -> Result<(), Failure>,
    notify: impl FnOnce(Option<&str>) -> Result<(), Failure>,
) -> Result<WaitFollowUpResult, Failure> {
    let reveal_error = path.as_deref().and_then(|path| reveal(path).err());
    if action == "reveal"
        && let Some(error) = &reveal_error
    {
        return Err(error.clone());
    }
    let path = path.as_deref().map(crate::dataset::display);
    let notification_error = notify(path.as_deref()).err();
    Ok(WaitFollowUpResult {
        action,
        path,
        reveal_error,
        notification_error,
    })
}

/// Accept a portable filename component, never a path or reserved device name.
pub(crate) fn validate_name(name: &str) -> Result<(), Failure> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        });
    if name.trim().is_empty()
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || device
    {
        return Err(Failure::invalid(
            "Enter a filename without a path, control characters, reserved characters, or a reserved device name",
        ));
    }
    Ok(())
}

/// Folders whose deletion would wreck the system or the user's data. The list
/// holds the folders themselves; their ancestors are refused as well.
fn protected_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    let home = ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from);
    for home in home {
        for child in [
            "",
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            "Movies",
        ] {
            folders.push(home.join(child));
        }
    }
    #[cfg(windows)]
    for variable in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
        "APPDATA",
        "LOCALAPPDATA",
        "PUBLIC",
    ] {
        folders.extend(std::env::var_os(variable).map(PathBuf::from));
    }
    #[cfg(unix)]
    folders.extend(
        [
            "/bin",
            "/boot",
            "/dev",
            "/etc",
            "/lib",
            "/lib32",
            "/lib64",
            "/opt",
            "/proc",
            "/root",
            "/run",
            "/sbin",
            "/srv",
            "/sys",
            "/tmp",
            "/usr",
            "/var",
            "/home",
            "/Applications",
            "/Library",
            "/System",
            "/Users",
            "/Volumes",
            "/private",
        ]
        .map(PathBuf::from),
    );
    let canonical: Vec<PathBuf> = folders
        .iter()
        .filter_map(|folder| std::fs::canonicalize(folder).ok())
        .collect();
    folders.extend(canonical);
    folders
}

/// Refuse to delete a filesystem root, a protected folder, or any ancestor of one.
pub(crate) fn refuse_protected_delete(path: &Path) -> Result<(), Failure> {
    if is_protected(path, &protected_folders()) {
        return Err(Failure::kind(
            "protected_delete_target",
            "OFLH does not delete drive roots, home folders, or system folders",
        ));
    }
    Ok(())
}

fn is_protected(path: &Path, protected: &[PathBuf]) -> bool {
    let canonical = std::fs::canonicalize(path).ok();
    [Some(path), canonical.as_deref()]
        .into_iter()
        .flatten()
        .any(|path| {
            path.parent().is_none()
                || protected
                    .iter()
                    .any(|folder| same_or_ancestor(path, folder))
        })
}

/// `path` equals `folder` or contains it; Windows paths compare case-insensitively.
fn same_or_ancestor(path: &Path, folder: &Path) -> bool {
    #[cfg(windows)]
    {
        let lower = |path: &Path| PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase());
        lower(folder).starts_with(lower(path))
    }
    #[cfg(not(windows))]
    folder.starts_with(path)
}

/// Translate move errors that the user can act on into stable categories.
fn move_failure(error: oflh_core::Error, source: &Path, destination: &Path) -> Failure {
    let oflh_core::Error::Io { source: io, .. } = &error else {
        return Failure::from(error);
    };
    #[cfg(windows)]
    let cross_device = io.raw_os_error() == Some(17);
    #[cfg(not(windows))]
    let cross_device = io.kind() == std::io::ErrorKind::CrossesDevices;
    // RENAME_NOREPLACE/RENAME_EXCL are refused by filesystems that cannot honour them.
    #[cfg(target_os = "linux")]
    let unsupported =
        io.kind() == std::io::ErrorKind::InvalidInput && !destination.starts_with(source);
    #[cfg(not(target_os = "linux"))]
    let unsupported =
        io.kind() == std::io::ErrorKind::Unsupported && !destination.starts_with(source);
    let kind = if cross_device {
        "move_cross_device"
    } else if unsupported {
        "move_unsupported"
    } else {
        return Failure::from(error);
    };
    let mut failure = Failure::from(error);
    failure.kind = kind.into();
    failure
}

/// Return the actual new native path only after the requested operation succeeds.
pub(crate) fn apply(
    target: &Target,
    action: WaitFollowUp,
    cancel: &Cancellation,
) -> Result<Option<PathBuf>, Failure> {
    cancel.check().map_err(Failure::from)?;
    match action {
        WaitFollowUp::None => Ok(None),
        WaitFollowUp::Reveal => Ok(Some(target.path.clone())),
        WaitFollowUp::Delete => {
            refuse_protected_delete(&target.path)?;
            let result = if target.directory {
                std::fs::remove_dir_all(&target.path)
            } else {
                std::fs::remove_file(&target.path)
            };
            result.map_err(|error| {
                Failure::from(oflh_core::io("delete target after becoming free", error))
            })?;
            Ok(None)
        }
        WaitFollowUp::Move(folder) => {
            let folder = std::fs::canonicalize(folder)
                .map_err(|error| Failure::from(oflh_core::io("resolve move destination", error)))?;
            if !std::fs::metadata(&folder)
                .map_err(|error| Failure::from(oflh_core::io("inspect move destination", error)))?
                .is_dir()
            {
                return Err(Failure::invalid("Choose a destination folder"));
            }
            let name = target
                .path
                .file_name()
                .ok_or_else(|| Failure::invalid("Choose a named file or folder to move"))?;
            let destination = folder.join(name);
            if destination.starts_with(&target.path) {
                return Err(Failure::kind(
                    "move_into_itself",
                    "A folder cannot be moved into itself",
                ));
            }
            cancel.check().map_err(Failure::from)?;
            oflh_platform::move_without_replace(&target.path, &destination)
                .map_err(|error| move_failure(error, &target.path, &destination))?;
            Ok(Some(destination))
        }
        WaitFollowUp::Rename(name) => {
            validate_name(&name)?;
            let parent = target
                .path
                .parent()
                .ok_or_else(|| Failure::invalid("Choose a named file or folder to rename"))?;
            let destination = parent.join(name);
            if destination == target.path {
                return Err(Failure::invalid("Choose a different filename"));
            }
            oflh_platform::move_without_replace(&target.path, &destination)
                .map_err(|error| move_failure(error, &target.path, &destination))?;
            Ok(Some(destination))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_actions_reveal_the_result_before_notifying_and_keep_auxiliary_errors_separate() {
        let events = std::cell::RefCell::new(Vec::new());
        let path = PathBuf::from("/build/destination/Bestätigung.pdf");
        let result = finish(
            "move".into(),
            Some(path.clone()),
            |actual| {
                assert_eq!(actual, path);
                events.borrow_mut().push("reveal");
                Err(Failure::invalid("file manager unavailable"))
            },
            |actual| {
                assert_eq!(actual, Some(crate::dataset::display(&path).as_str()));
                events.borrow_mut().push("notify");
                Err(Failure::invalid("notification unavailable"))
            },
        )
        .unwrap();
        assert_eq!(*events.borrow(), ["reveal", "notify"]);
        assert_eq!(result.action, "move");
        assert_eq!(result.path, Some(crate::dataset::display(&path)));
        assert!(result.reveal_error.is_some());
        assert!(result.notification_error.is_some());
        let deleted = finish(
            "delete".into(),
            None,
            |_| panic!("a deleted path must not be revealed"),
            |path| {
                assert!(path.is_none());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(deleted.path, None);
    }
    #[test]
    fn a_failed_reveal_only_action_does_not_announce_success() {
        let error = finish(
            "reveal".into(),
            Some(PathBuf::from("/build/report.pdf")),
            |_| Err(Failure::invalid("file manager unavailable")),
            |_| panic!("a failed reveal must not send a success notification"),
        )
        .unwrap_err();
        assert_eq!(error.message, "file manager unavailable");
    }
    #[test]
    fn roots_protected_folders_and_their_ancestors_are_never_deleted() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home").join("karim");
        let documents = home.join("Documents");
        std::fs::create_dir_all(&documents).unwrap();
        let report = documents.join("report.pdf");
        std::fs::write(&report, b"x").unwrap();
        let protected = [home.clone(), documents.clone()];
        for refused in [home.as_path(), documents.as_path(), home.parent().unwrap()] {
            assert!(is_protected(refused, &protected), "{refused:?}");
        }
        let filesystem_root = root.path().ancestors().last().unwrap();
        assert!(is_protected(filesystem_root, &[]));
        assert!(!is_protected(&report, &protected));
        assert!(!is_protected(&documents.join("old"), &protected));
        let error = refuse_protected_delete(filesystem_root).unwrap_err();
        assert_eq!(error.kind, "protected_delete_target");
    }
    #[test]
    fn a_folder_is_not_moved_into_itself_and_stays_in_place() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("build");
        std::fs::create_dir(&folder).unwrap();
        let target = Target::new(&folder).unwrap();
        let error = apply(
            &target,
            WaitFollowUp::Move(folder.clone()),
            &Cancellation::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind, "move_into_itself");
        assert!(folder.is_dir());
    }
    #[test]
    fn a_protected_delete_leaves_the_target_untouched() {
        let home = std::fs::canonicalize(
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap(),
        )
        .unwrap();
        let target = Target::new(&home).unwrap();
        let error = apply(&target, WaitFollowUp::Delete, &Cancellation::default()).unwrap_err();
        assert_eq!(error.kind, "protected_delete_target");
        assert!(home.is_dir());
    }
    #[test]
    fn names_are_components_and_keep_unicode_and_extensions() {
        for valid in [
            "Bestätigung neu.pdf",
            ".hidden",
            "archive.tar.gz",
            "文件.pdf",
        ] {
            assert!(validate_name(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            " ",
            ".",
            "..",
            "../escape.pdf",
            "folder/file",
            "folder\\file",
            "C:\\file.pdf",
            "bad\0.pdf",
            "bad\u{0085}.pdf",
            "CON",
            "nul.pdf",
            "com1.txt",
            "LPT9",
            "bad?.pdf",
            "bad.",
            "bad ",
        ] {
            assert!(validate_name(invalid).is_err(), "{invalid:?}");
        }
    }
}
