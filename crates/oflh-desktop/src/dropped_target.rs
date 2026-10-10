//! One pending native drop, accepted only when the frontend permits file inspection.
use crate::contract::Failure;
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

#[derive(Default)]
struct PendingDrop {
    next_request: u32,
    target: Option<(u32, Result<PathBuf, Failure>)>,
}

/// Retain one lossless native target while its opaque request is offered to the UI.
#[derive(Default)]
pub(crate) struct DroppedTarget {
    pending: Mutex<PendingDrop>,
}

impl DroppedTarget {
    fn lock(&self) -> MutexGuard<'_, PendingDrop> {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// Replace the pending drop without inspecting or serializing native paths.
    pub(crate) fn capture(&self, paths: &[PathBuf]) -> Result<u32, Failure> {
        let mut pending = self.lock();
        let request = pending
            .next_request
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to drop another target"))?;
        let target = match paths {
            [path] => Ok(path.clone()),
            _ => Err(Failure::invalid("Drop one file or folder at a time")),
        };
        pending.next_request = request;
        pending.target = Some((request, target));
        Ok(request)
    }

    /// Consume an accepted drop once; stale requests cannot consume a newer target.
    pub(crate) fn take(&self, request: u32) -> Result<PathBuf, Failure> {
        let mut pending = self.lock();
        if pending.target.as_ref().map(|(current, _)| *current) != Some(request) {
            return Err(Failure::invalid("Dropped target expired; drop it again"));
        }
        match pending.target.take() {
            Some((_, target)) => target,
            None => Err(Failure::invalid("Dropped target expired; drop it again")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_files_and_folders_without_accepting_them_until_requested() {
        let dropped = DroppedTarget::default();
        for path in ["/workspace/report.txt", "/workspace/project"] {
            let path = PathBuf::from(path);
            let request = dropped.capture(std::slice::from_ref(&path)).unwrap();
            assert_eq!(dropped.take(request).unwrap(), path);
            assert!(dropped.take(request).is_err());
        }
    }

    #[test]
    fn ignored_or_stale_requests_do_not_consume_a_newer_drop() {
        let dropped = DroppedTarget::default();
        let ignored = dropped
            .capture(&[PathBuf::from("/workspace/ignored")])
            .unwrap();
        let path = PathBuf::from("/workspace/project");
        let accepted = dropped.capture(std::slice::from_ref(&path)).unwrap();
        assert!(dropped.take(ignored).is_err());
        assert_eq!(dropped.take(accepted).unwrap(), path);
    }

    #[test]
    fn empty_or_multiple_targets_fail_only_when_the_ui_accepts_the_drop() {
        let dropped = DroppedTarget::default();
        for paths in [
            vec![],
            vec![
                PathBuf::from("/workspace/project"),
                PathBuf::from("/tmp/other"),
            ],
        ] {
            let request = dropped.capture(&paths).unwrap();
            assert!(dropped.take(request).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_paths_do_not_round_trip_through_display_text() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(
            b"/workspace/file-\xff".to_vec(),
        ));
        let dropped = DroppedTarget::default();
        let request = dropped.capture(std::slice::from_ref(&path)).unwrap();
        assert_eq!(dropped.take(request).unwrap(), path);
    }
}
