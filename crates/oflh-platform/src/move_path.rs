//! Atomic moves and renames that never replace an existing destination.
use oflh_core::{Result, io};
use std::path::Path;

/// Move a file, symlink, or directory without replacing any destination entry.
/// Cross-volume moves and unsupported filesystem operations remain explicit errors.
pub fn move_without_replace(source: &Path, destination: &Path) -> Result<()> {
    native_move(source, destination)
        .map_err(|error| io("move or rename without replacing destination", error))
}

#[cfg(windows)]
fn native_move(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    fn wide(path: &Path) -> std::io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        value.push(0);
        Ok(value)
    }
    let source = wide(source)?;
    let destination = wide(destination)?;
    // SAFETY: both lossless UTF-16 buffers are NUL-terminated and remain alive for
    // the call. Zero flags omit MOVEFILE_REPLACE_EXISTING and MOVEFILE_COPY_ALLOWED.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            0,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn native_move(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    #[cfg(target_os = "linux")]
    // SAFETY: both C strings retain the native path bytes and remain alive for
    // the call. RENAME_NOREPLACE atomically refuses every existing destination.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    // SAFETY: both native C strings are NUL-terminated and alive for the call.
    // RENAME_EXCL performs the existence check atomically with the rename.
    let result =
        unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn native_move(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_unicode_files_and_directories_and_never_overwrites_existing_entries() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Bestätigung.pdf");
        let destination = root.path().join("renamed.pdf");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&destination, b"keep").unwrap();
        assert!(move_without_replace(&source, &destination).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
        assert_eq!(std::fs::read(&destination).unwrap(), b"keep");
        let fresh = root.path().join("new.pdf");
        move_without_replace(&source, &fresh).unwrap();
        assert_eq!(std::fs::read(fresh).unwrap(), b"source");
        assert!(!source.exists());
        let folder = root.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        let moved = root.path().join("moved-folder");
        move_without_replace(&folder, &moved).unwrap();
        assert!(moved.is_dir());
    }
    #[cfg(unix)]
    #[test]
    fn an_existing_dangling_symlink_is_not_replaced() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let destination = root.path().join("link");
        std::fs::write(&source, b"source").unwrap();
        std::os::unix::fs::symlink(root.path().join("missing"), &destination).unwrap();
        assert!(move_without_replace(&source, &destination).is_err());
        assert!(
            std::fs::symlink_metadata(destination)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
}
