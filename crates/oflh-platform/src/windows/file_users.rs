//! Compatibility recovery for Restart Manager's invalid-handle result.
//! Class 47 is reserved; unsupported filesystems/OSes remain explicit failures.
use super::{Handle, process_snapshot, read_identity, wide};
use oflh_core::{Cancellation, Error, Identity, InspectionCounter, Result, io};
use std::{
    collections::HashMap,
    mem::{offset_of, size_of},
    path::Path,
};
use windows_sys::{
    Wdk::{
        Storage::FileSystem::{FileProcessIdsUsingFileInformation, NtQueryInformationFile},
        System::SystemServices::FILE_PROCESS_IDS_USING_FILE_INFORMATION,
    },
    Win32::{
        Foundation::*,
        Storage::FileSystem::*,
        System::{Diagnostics::ToolHelp::*, IO::IO_STATUS_BLOCK},
    },
};

pub(super) fn capture_identities(cancel: &Cancellation) -> Result<HashMap<u32, Identity>> {
    cancel.check()?;
    let (snapshot, mut entry) = process_snapshot()?;
    let mut identities = HashMap::new();
    // SAFETY: owned process snapshot and correctly sized initialized output entry.
    let mut next = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while next != 0 {
        cancel.check()?;
        if let Ok(identity) = read_identity(entry.th32ProcessID) {
            identities.insert(identity.pid, identity);
        }
        // SAFETY: same owned snapshot and writable correctly sized record.
        next = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    // SAFETY: capture the enumeration result immediately after the last call.
    let code = unsafe { GetLastError() };
    if code != ERROR_NO_MORE_FILES {
        return Err(io(
            "capture native file-user process births",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    Ok(identities)
}

pub(super) fn query(path: &Path, cancel: &Cancellation) -> Result<Vec<u32>> {
    cancel.check()?;
    let name = wide(path)?;
    // SAFETY: terminated path, metadata-only access and compatible sharing flags.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    let handle = Handle::new(handle, "open native file-user metadata")?;
    let mut buffer = vec![0usize; 32];
    loop {
        cancel.check()?;
        cancel.record(InspectionCounter::NativeFileUserQueries, 1);
        let mut status_block = IO_STATUS_BLOCK::default();
        // SAFETY: owned handle; SDK-defined layout, pointer-aligned writable
        // buffer with an exact bounded byte size; initialized writable status.
        let status = unsafe {
            NtQueryInformationFile(
                handle.0,
                &mut status_block,
                buffer.as_mut_ptr().cast(),
                size_of_val(buffer.as_slice()) as u32,
                FileProcessIdsUsingFileInformation,
            )
        };
        cancel.check()?;
        if matches!(
            status,
            STATUS_BUFFER_OVERFLOW | STATUS_BUFFER_TOO_SMALL | STATUS_INFO_LENGTH_MISMATCH
        ) {
            if size_of_val(buffer.as_slice()) >= 16 * 1024 * 1024 {
                return Err(Error::Unavailable(
                    "native file-user result exceeds buffer budget; results unavailable".into(),
                ));
            }
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != STATUS_SUCCESS {
            return Err(io(
                "query native file users (NTSTATUS)",
                std::io::Error::from_raw_os_error(status),
            ));
        }
        let mut users = decode_users(&buffer, status_block.Information)?;
        users.sort_unstable();
        users.dedup();
        return Ok(users);
    }
}

fn decode_users(buffer: &[usize], returned: usize) -> Result<Vec<u32>> {
    let offset = offset_of!(FILE_PROCESS_IDS_USING_FILE_INFORMATION, ProcessIdList);
    if returned < offset || returned > size_of_val(buffer) {
        return Err(Error::Unavailable(
            "invalid native file-user result length".into(),
        ));
    }
    // SAFETY: returned covers the SDK header; the allocation is pointer-aligned.
    let count = unsafe { buffer.as_ptr().cast::<u32>().read() } as usize;
    if count > (returned - offset) / size_of::<usize>() {
        return Err(Error::Unavailable(
            "invalid native file-user result count".into(),
        ));
    }
    let start = offset / size_of::<usize>();
    buffer[start..start + count]
        .iter()
        .map(|pid| {
            u32::try_from(*pid)
                .map_err(|_| Error::Unavailable("native file-user PID exceeds u32".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_list_retains_every_pid_and_rejects_invalid_extents() {
        assert_eq!(
            offset_of!(FILE_PROCESS_IDS_USING_FILE_INFORMATION, ProcessIdList),
            8
        );
        let mut buffer = vec![0usize; 161];
        buffer[0] = 160;
        for (index, pid) in buffer[1..].iter_mut().enumerate() {
            *pid = index + 1;
        }
        assert_eq!(
            decode_users(&buffer, size_of_val(buffer.as_slice()))
                .unwrap()
                .len(),
            160
        );
        assert!(decode_users(&buffer, 8).is_err());
        assert!(decode_users(&buffer, size_of_val(buffer.as_slice()) + 1).is_err());
        assert!(decode_users(&[], 0).is_err());
        buffer[0] = usize::MAX;
        assert!(decode_users(&buffer, size_of_val(buffer.as_slice())).is_err());
        assert_eq!(decode_users(&[0], 8).unwrap(), Vec::<u32>::new());
        assert!(decode_users(&[1, usize::MAX], 16).is_err());
    }
}
