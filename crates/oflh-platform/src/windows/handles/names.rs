//! Native paths stay UTF-16. Alias enumeration verifies the held file's identity.
use super::super::*;
use std::{
    collections::HashMap,
    path::{Component, Prefix},
};

pub(super) fn metadata(path: &Path) -> Result<Handle> {
    let name = wide(path)?;
    // SAFETY: terminated path, metadata-only existing open, maximal sharing.
    Handle::new(
        unsafe {
            CreateFileW(
                name.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                null_mut(),
            )
        },
        "open file metadata",
    )
}

pub(super) fn final_path(handle: &Handle, deleted: bool) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 512];
    loop {
        // SAFETY: valid owned disk handle, writable buffer of stated length.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle.0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                (if deleted {
                    FILE_NAME_OPENED
                } else {
                    FILE_NAME_NORMALIZED
                }) | VOLUME_NAME_DOS,
            )
        } as usize;
        if length == 0 {
            return Err(io(
                "resolve file handle path",
                std::io::Error::last_os_error(),
            ));
        }
        if length < buffer.len() {
            return Ok(path(&buffer[..length]));
        }
        if length >= 32_768 {
            return Err(Error::Unavailable(
                "file path exceeds native helper extent".into(),
            ));
        }
        buffer.resize(length + 1, 0);
    }
}

pub(super) fn standard(handle: &Handle) -> Result<FILE_STANDARD_INFO> {
    let mut info = FILE_STANDARD_INFO::default();
    // SAFETY: SDK class and exact writable FILE_STANDARD_INFO size, owned handle.
    if unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileStandardInfo,
            (&mut info as *mut FILE_STANDARD_INFO).cast(),
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    } == 0
    {
        return Err(io(
            "read held file metadata",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(info)
}
fn identity(handle: &Handle) -> Result<(u64, [u8; 16])> {
    let mut info = FILE_ID_INFO::default();
    // SAFETY: SDK class and exact output size; full 128-bit file identity is retained.
    if unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io(
            "read held file identity",
            std::io::Error::last_os_error(),
        ));
    }
    Ok((info.VolumeSerialNumber, info.FileId.Identifier))
}

struct Links(HANDLE);
impl Drop for Links {
    fn drop(&mut self) {
        // SAFETY: FindFirstFileNameW acquired this search handle, closed exactly once.
        unsafe { FindClose(self.0) };
    }
}
fn volume_root(path: &Path) -> Result<PathBuf> {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err(Error::Unavailable(
            "file alias volume is unavailable".into(),
        ));
    };
    if !matches!(
        prefix.kind(),
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _)
    ) {
        return Err(Error::Unavailable(
            "file alias volume is unsupported".into(),
        ));
    }
    let mut root = PathBuf::from(prefix.as_os_str());
    root.push("\\");
    Ok(root)
}

pub(super) fn aliases(handle: &Handle, observed: &Path, target: &Target) -> Result<Vec<PathBuf>> {
    let held_id = identity(handle)?;
    let named = metadata(observed)?;
    if identity(&named)? != held_id {
        return Err(Error::Changed);
    }
    let root = volume_root(observed)?;
    let name = wide(observed)?;
    let mut buffer = vec![0u16; 512];
    let search = loop {
        let mut length = buffer.len() as u32;
        // SAFETY: terminated native path, flags zero, exact writable buffer/length.
        let search =
            unsafe { FindFirstFileNameW(name.as_ptr(), 0, &mut length, buffer.as_mut_ptr()) };
        if search != INVALID_HANDLE_VALUE && !search.is_null() {
            break Links(search);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_MORE_DATA as i32)
            || length as usize > 32_768
            || length as usize <= buffer.len()
        {
            return Err(io("enumerate file aliases", error));
        }
        buffer.resize(length as usize, 0);
    };
    let mut matches = Vec::new();
    loop {
        let relative = path(&buffer);
        let alias = root.join(
            relative
                .strip_prefix("\\")
                .map_err(|_| Error::Unavailable("invalid volume-relative file alias".into()))?,
        );
        if target.contains(&alias) && identity(&metadata(&alias)?)? == held_id {
            matches.push(alias);
        }
        loop {
            buffer.fill(0);
            let mut length = buffer.len() as u32;
            // SAFETY: owned live search handle and exact writable buffer extent.
            if unsafe { FindNextFileNameW(search.0, &mut length, buffer.as_mut_ptr()) } != 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_HANDLE_EOF as i32) {
                // The named path could have been replaced during enumeration.
                if identity(&metadata(observed)?)? != held_id {
                    return Err(Error::Changed);
                }
                return Ok(matches);
            }
            if error.raw_os_error() != Some(ERROR_MORE_DATA as i32)
                || length as usize > 32_768
                || length as usize <= buffer.len()
            {
                return Err(io("enumerate next file alias", error));
            }
            buffer.resize(length as usize, 0);
        }
    }
}

pub(super) struct DevicePaths(Vec<(Vec<u16>, Vec<u16>)>);
impl DevicePaths {
    pub(super) fn new() -> Self {
        let mut mappings = Vec::new();
        for letter in b'A'..=b'Z' {
            let drive = [u16::from(letter), u16::from(b':'), 0];
            let mut buffer = vec![0u16; 512];
            loop {
                // SAFETY: terminated drive name and exact writable output extent.
                let length = unsafe {
                    QueryDosDeviceW(drive.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32)
                };
                if length != 0 {
                    let current = buffer
                        .iter()
                        .position(|&character| character == 0)
                        .unwrap_or(buffer.len());
                    mappings.push((buffer[..current].to_vec(), drive[..2].to_vec()));
                    break;
                }
                // SAFETY: capture error immediately after the failed native call.
                if unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER || buffer.len() >= 32_768
                {
                    break;
                }
                buffer.resize(buffer.len() * 2, 0);
            }
        }
        mappings.sort_by_key(|(device, _)| std::cmp::Reverse(device.len()));
        Self(mappings)
    }
    pub(super) fn resolve(&self, native: &[u16]) -> Result<PathBuf> {
        for (device, drive) in &self.0 {
            if let Some(suffix) = native.strip_prefix(device.as_slice())
                && suffix.first() == Some(&u16::from(b'\\'))
            {
                // Match GetFinalPathNameByHandleW's lossless verbatim DOS form.
                let mut translated: Vec<_> = "\\\\?\\".encode_utf16().collect();
                translated.extend(drive);
                translated.extend(suffix);
                return Ok(path(&translated));
            }
        }
        let network: Vec<_> = "\\Device\\Mup\\".encode_utf16().collect();
        if let Some(suffix) = native.strip_prefix(network.as_slice()) {
            let mut translated: Vec<_> = "\\\\?\\UNC\\".encode_utf16().collect();
            translated.extend(suffix);
            return Ok(path(&translated));
        }
        Err(Error::Unavailable(
            "mapped-file device has no supported DOS/UNC path".into(),
        ))
    }
}

#[derive(Default)]
pub(super) struct Sharing(std::sync::Mutex<HashMap<PathBuf, Option<LockEvidence>>>);
impl Sharing {
    pub(super) fn inspect(&self, path: &Path) -> Option<AccessKind> {
        let cached = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(path)
            .cloned();
        let evidence = cached.unwrap_or_else(|| {
            // Potentially blocking probes run outside the cache lock and inside
            // the owned helper, never on the parent/UI thread.
            let evidence = sharing(path);
            self.0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(path.to_owned(), evidence.clone());
            evidence
        });
        match evidence {
            Some(LockEvidence::SharingConflict(kind)) => Some(kind),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_translation_respects_boundaries_and_preserves_surrogates() {
        let paths = DevicePaths(vec![(
            "\\Device\\Disk1".encode_utf16().collect(),
            "C:".encode_utf16().collect(),
        )]);
        let mut native: Vec<_> = "\\Device\\Disk1\\files\\".encode_utf16().collect();
        native.push(0xd800);
        let resolved = paths.resolve(&native).unwrap();
        assert_eq!(resolved.as_os_str().encode_wide().last(), Some(0xd800));
        assert!(
            paths
                .resolve(&"\\Device\\Disk10\\file".encode_utf16().collect::<Vec<_>>())
                .is_err()
        );
        assert_eq!(
            paths
                .resolve(
                    &"\\Device\\Mup\\server\\share\\file"
                        .encode_utf16()
                        .collect::<Vec<_>>()
                )
                .unwrap(),
            PathBuf::from("\\\\?\\UNC\\server\\share\\file")
        );
    }
}
