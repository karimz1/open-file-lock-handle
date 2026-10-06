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

pub(super) fn final_path(handle: &Handle, deleted: bool, target: &Target) -> Result<PathBuf> {
    if deleted {
        // GetFinalPathNameByHandle can reject a delete-pending file even though
        // FileNameInfo still exposes its name. Bind volume-relative names to a
        // checked volume identity; never attach them to an arbitrary drive.
        if let Ok(path) = deleted_path(handle, target) {
            return Ok(path);
        }
    }
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

fn decode_name(buffer: &[u32]) -> Result<PathBuf> {
    let Some(&length) = buffer.first() else {
        return Err(Error::Unavailable("file name header is missing".into()));
    };
    let length = length as usize;
    if std::mem::offset_of!(FILE_NAME_INFO, FileName) != size_of::<u32>()
        || length == 0
        || !length.is_multiple_of(2)
        || length / 2 > 32_768
        || length > size_of_val(buffer).saturating_sub(size_of::<u32>())
    {
        return Err(Error::Unavailable("invalid native file name extent".into()));
    }
    // SAFETY: aligned u32 allocation, initialized success extent checked above;
    // every UTF-16 bit pattern is retained without Unicode replacement.
    let name =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(1).cast::<u16>(), length / 2) };
    if name.contains(&0) {
        return Err(Error::Unavailable("native file name contains NUL".into()));
    }
    Ok(path(name))
}
fn relative_name(handle: &Handle) -> Result<PathBuf> {
    let mut buffer = vec![0u32; 256];
    loop {
        let bytes = size_of_val(buffer.as_slice());
        // SAFETY: owned handle, aligned initialized allocation, exact writable
        // extent for SDK FileNameInfo's variable-length UTF-16 record.
        if unsafe {
            GetFileInformationByHandleEx(
                handle.0,
                FileNameInfo,
                buffer.as_mut_ptr().cast(),
                bytes as u32,
            )
        } != 0
        {
            return decode_name(&buffer);
        }
        let error = std::io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(code) if code == ERROR_MORE_DATA as i32 || code == ERROR_INSUFFICIENT_BUFFER as i32)
        {
            return Err(io("read deleted file name", error));
        }
        let maximum = 32_768 * 2 + size_of::<u32>();
        let required = buffer[0] as usize + size_of::<u32>();
        let next = (bytes * 2).max(required).min(maximum);
        if required > maximum || next <= bytes {
            return Err(Error::Unavailable(
                "native file name exceeds helper extent".into(),
            ));
        }
        buffer.resize(next.div_ceil(size_of::<u32>()), 0);
    }
}
fn deleted_path(handle: &Handle, target: &Target) -> Result<PathBuf> {
    let root = volume_root(&target.path)?;
    if identity(handle)?.0 != identity(&metadata(&root)?)?.0 {
        return Err(Error::Unavailable(
            "deleted file volume cannot be resolved through target drive".into(),
        ));
    }
    let relative = relative_name(handle)?;
    Ok(root.join(
        relative
            .strip_prefix("\\")
            .map_err(|_| Error::Unavailable("invalid deleted file volume-relative name".into()))?,
    ))
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

/// Successful and unavailable probes are shared by path for this scan only.
#[derive(Clone, Copy)]
struct SharingProbe {
    evidence: Option<AccessKind>,
    unavailable: Option<i32>,
}
impl SharingProbe {
    fn result(self) -> Result<Option<AccessKind>> {
        if self.evidence.is_some() {
            return Ok(self.evidence);
        }
        match self.unavailable {
            Some(code) => Err(io(
                "probe native file sharing",
                std::io::Error::from_raw_os_error(code),
            )),
            None => Ok(None),
        }
    }
}
fn native_name(path: &Path) -> Result<Vec<u16>> {
    if !path.is_absolute() {
        return Err(Error::Unavailable(
            "sharing probe requires an absolute path".into(),
        ));
    }
    let name: Vec<u16> = path.as_os_str().encode_wide().collect();
    let verbatim: Vec<_> = "\\\\?\\".encode_utf16().collect();
    let network: Vec<_> = "\\\\".encode_utf16().collect();
    let mut native: Vec<_> = "\\??\\".encode_utf16().collect();
    if let Some(suffix) = name.strip_prefix(verbatim.as_slice()) {
        native.extend(suffix);
    } else if let Some(suffix) = name.strip_prefix(network.as_slice()) {
        native.extend("UNC\\".encode_utf16());
        native.extend(suffix);
    } else if matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
    {
        native.extend(name);
    } else {
        return Err(Error::Unavailable(
            "sharing probe requires an absolute DOS or UNC path".into(),
        ));
    }
    if native.contains(&0) || native.len() > u16::MAX as usize / 2 {
        return Err(Error::Unavailable(
            "sharing probe native name exceeds extent".into(),
        ));
    }
    Ok(native)
}
fn probe_sharing(path: &Path) -> Result<SharingProbe> {
    use windows_sys::{
        Wdk::{Foundation::OBJECT_ATTRIBUTES, Storage::FileSystem::*},
        Win32::System::IO::IO_STATUS_BLOCK,
    };
    const _: () = {
        assert!(size_of::<UNICODE_STRING>() == 16);
        assert!(size_of::<OBJECT_ATTRIBUTES>() == 48);
        assert!(size_of::<IO_STATUS_BLOCK>() == 16);
    };
    let mut name = native_name(path)?;
    let name_bytes = (name.len() * size_of::<u16>()) as u16;
    let unicode = UNICODE_STRING {
        Length: name_bytes,
        MaximumLength: name_bytes,
        Buffer: name.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        ..OBJECT_ATTRIBUTES::default()
    };
    let mut unavailable = None;
    for (access, kind) in [
        (FILE_GENERIC_READ, AccessKind::Read),
        (FILE_GENERIC_WRITE, AccessKind::Write),
        (DELETE | SYNCHRONIZE, AccessKind::Delete),
    ] {
        let mut status_block = IO_STATUS_BLOCK::default();
        let mut raw = null_mut();
        // SAFETY: checked length-delimited UTF-16 and exact SDK POD layouts remain
        // live through the synchronous call. FILE_OPEN never creates or changes
        // content; maximal sharing and NO_RECALL avoid recalling offline data.
        // COMPLETE_IF_OPLOCKED returns after initiating an oplock break rather
        // than waiting for the inspected application's acknowledgement.
        let status = unsafe {
            NtCreateFile(
                &mut raw,
                access,
                &attributes,
                &mut status_block,
                null(),
                FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_OPEN,
                FILE_SYNCHRONOUS_IO_NONALERT
                    | FILE_NON_DIRECTORY_FILE
                    | FILE_COMPLETE_IF_OPLOCKED
                    | FILE_OPEN_NO_RECALL,
                null(),
                0,
            )
        };
        if status == STATUS_SHARING_VIOLATION {
            return Ok(SharingProbe {
                evidence: Some(kind),
                unavailable: None,
            });
        }
        if status < 0 {
            unavailable.get_or_insert(status);
            continue;
        }
        // Synchronous opens complete before the borrowed I/O block can be dropped.
        // Alternate success STATUS_OPLOCK_BREAK_IN_PROGRESS still owns a handle.
        drop(Handle::new(raw, "own native sharing probe")?);
    }
    Ok(SharingProbe {
        evidence: None,
        unavailable,
    })
}
#[derive(Default)]
pub(super) struct Sharing(std::sync::Mutex<HashMap<PathBuf, SharingProbe>>);
impl Sharing {
    pub(super) fn inspect(&self, path: &Path) -> Result<Option<AccessKind>> {
        let cached = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(path)
            .copied();
        let probe = match cached {
            Some(probe) => probe,
            None => {
                // Native calls run outside the cache lock and inside the helper.
                let probe = probe_sharing(path)?;
                self.0
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .insert(path.to_owned(), probe);
                probe
            }
        };
        probe.result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_probe_names_preserve_dos_unc_and_unpaired_utf16() {
        for (input, expected) in [
            ("C:\\folder\\file", "\\??\\C:\\folder\\file"),
            ("\\\\?\\C:\\file", "\\??\\C:\\file"),
            ("\\\\server\\share\\file", "\\??\\UNC\\server\\share\\file"),
            (
                "\\\\?\\UNC\\server\\share\\file",
                "\\??\\UNC\\server\\share\\file",
            ),
        ] {
            assert_eq!(
                native_name(Path::new(input)).unwrap(),
                expected.encode_utf16().collect::<Vec<_>>()
            );
        }
        let mut name: Vec<_> = "C:\\file".encode_utf16().collect();
        name.push(0xd800);
        assert_eq!(native_name(&path(&name)).unwrap().last(), Some(&0xd800));
        assert!(native_name(Path::new("relative.bin")).is_err());
        // The native C-string decoder intentionally trims NUL; construct an
        // actual invalid OsString so this regression reaches the probe boundary.
        assert!(
            native_name(&PathBuf::from(OsString::from_wide(&[
                67, 58, 92, 65, 0, 66
            ])))
            .is_err()
        );
        assert!(native_name(Path::new("C:relative.bin")).is_err());
        let mut oversized: Vec<_> = "C:\\".encode_utf16().collect();
        oversized.extend(std::iter::repeat_n(65, 32768));
        assert!(native_name(&PathBuf::from(OsString::from_wide(&oversized))).is_err());
        let probe = SharingProbe {
            evidence: None,
            unavailable: Some(STATUS_ACCESS_DENIED),
        };
        assert!(
            matches!(probe.result(), Err(Error::Io { source, .. }) if source.raw_os_error() == Some(STATUS_ACCESS_DENIED))
        );
    }
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
    #[test]
    fn file_name_extents_are_checked_before_reading_utf16() {
        let mut buffer = vec![0u32; 4];
        buffer[0] = 4;
        buffer[1] = u32::from(b'\\') | (0xd800 << 16);
        let path = decode_name(&buffer).unwrap();
        assert_eq!(
            path.as_os_str().encode_wide().collect::<Vec<_>>(),
            [92, 0xd800]
        );
        for length in [0, 1, 13, 14, u32::MAX] {
            buffer[0] = length;
            assert!(decode_name(&buffer).is_err());
        }
        buffer[0] = 4;
        buffer[1] = 0;
        assert!(decode_name(&buffer).is_err());
    }
}
