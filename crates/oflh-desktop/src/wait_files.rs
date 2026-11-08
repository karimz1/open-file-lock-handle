//! Validated names and filesystem results for explicit waiting follow-ups.
#[cfg(any(feature = "desktop", test))]
use crate::contract::WaitFollowUpResult;
use crate::{contract::Failure, service::WaitFollowUp};
use oflh_core::{Cancellation, Target};
#[cfg(any(feature = "desktop", test))]
use std::path::Path;
use std::path::PathBuf;

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
            cancel.check().map_err(Failure::from)?;
            oflh_platform::move_without_replace(&target.path, &destination)
                .map_err(Failure::from)?;
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
                .map_err(Failure::from)?;
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
