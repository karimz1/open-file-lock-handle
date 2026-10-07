//! Handle-first discovery experiment, isolated because native path queries can block.
use super::{FileHandle, Fixture, Result, open_metadata};
use oflh_core::{Error, Identity, Target, io};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    mem::{align_of, size_of},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use windows_sys::{
    Wdk::System::SystemInformation::NtQuerySystemInformation,
    Win32::{Foundation::*, Storage::FileSystem::*, System::Threading::*},
};

/// Native class 64 layout from phnt/ntexapi.h; verify before reading any records.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct HandleEntry {
    object: usize,
    process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_trace: u16,
    object_type: u16,
    attributes: u32,
    reserved: u32,
}
const HEADER_BYTES: usize = 2 * size_of::<usize>();

fn decode_entries(buffer: &[usize], returned: usize) -> oflh_core::Result<Vec<HandleEntry>> {
    if size_of::<HandleEntry>() != 40
        || size_of::<usize>() != 8
        || align_of::<HandleEntry>() > align_of::<usize>()
    {
        return Err(Error::Unavailable("unsupported native handle ABI".into()));
    }
    if returned < HEADER_BYTES || returned > size_of_val(buffer) {
        return Err(Error::Unavailable(
            "invalid native handle snapshot size".into(),
        ));
    }
    let count = buffer[0];
    if count > (returned - HEADER_BYTES) / size_of::<HandleEntry>() {
        return Err(Error::Unavailable(
            "invalid native handle snapshot count".into(),
        ));
    }
    // SAFETY: layout/alignment checked above; the returned byte extent covers all
    // initialized entries. The struct contains only integer types (all bits valid).
    let records =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().add(2).cast::<HandleEntry>(), count) };
    Ok(records.to_vec())
}

fn system_handles() -> oflh_core::Result<Vec<HandleEntry>> {
    let mut buffer = vec![0usize; 128 * 1024 / size_of::<usize>()];
    loop {
        let bytes = size_of_val(buffer.as_slice());
        let mut returned = 0;
        // SAFETY: pointer-aligned writable allocation, exact u32 byte length,
        // writable return length. Class 64 is confined to this opt-in experiment.
        let status = unsafe {
            NtQuerySystemInformation(64, buffer.as_mut_ptr().cast(), bytes as u32, &mut returned)
        };
        if matches!(
            status,
            STATUS_INFO_LENGTH_MISMATCH | STATUS_BUFFER_TOO_SMALL
        ) {
            let next = (bytes * 2).max(returned as usize + 64 * 1024);
            if next > 128 * 1024 * 1024 {
                return Err(Error::Unavailable(
                    "native handle snapshot exceeds probe budget".into(),
                ));
            }
            buffer.resize(next.div_ceil(size_of::<usize>()), 0);
            continue;
        }
        if status != STATUS_SUCCESS {
            return Err(io(
                "snapshot native handles (NTSTATUS)",
                std::io::Error::from_raw_os_error(status),
            ));
        }
        return decode_entries(&buffer, returned as usize);
    }
}

fn process_identity(handle: &FileHandle, pid: u32) -> oflh_core::Result<Identity> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: owned process handle and correctly sized writable native outputs.
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(io(
            "identify handle source process",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(Identity {
        pid,
        started: (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
        started_sub: 0,
    })
}

fn duplicate(source: &FileHandle, value: usize) -> oflh_core::Result<FileHandle> {
    let mut handle = std::ptr::null_mut();
    // SAFETY: source owns the process handle; handle value is never dereferenced.
    // SAME_ACCESS creates a new local owned handle without changing the source.
    let succeeded = unsafe {
        DuplicateHandle(
            source.0,
            value as HANDLE,
            GetCurrentProcess(),
            &mut handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if succeeded == 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io(
            "duplicate observed file handle",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(FileHandle(handle))
}

fn final_path(handle: &FileHandle) -> oflh_core::Result<PathBuf> {
    let mut buffer = vec![0u16; 512];
    loop {
        // SAFETY: owned disk-file handle and writable UTF-16 buffer of stated size.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle.0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        } as usize;
        if length == 0 {
            return Err(io(
                "resolve observed file handle",
                std::io::Error::last_os_error(),
            ));
        }
        if length >= buffer.len() {
            if length > 1024 * 1024 {
                return Err(Error::Unavailable(
                    "native path exceeds probe budget".into(),
                ));
            }
            buffer.resize(length + 1, 0);
            continue;
        }
        return Ok(PathBuf::from(OsString::from_wide(&buffer[..length])));
    }
}

#[derive(Default)]
struct Work {
    processes: usize,
    unavailable_processes: usize,
    attempted_handles: usize,
    unavailable_handles: usize,
    path_queries: usize,
    users: BTreeSet<Identity>,
    fixture_users: BTreeSet<Identity>,
    fixture_files: BTreeSet<PathBuf>,
}

fn inspect_process(
    pid: u32,
    values: &[usize],
    target: &Target,
    fixtures: &[u32],
    work: &mut Work,
) -> oflh_core::Result<()> {
    work.processes += 1;
    // SAFETY: numeric access/PID create a process handle owned only by this worker.
    let process = unsafe {
        OpenProcess(
            PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE,
            0,
            pid,
        )
    };
    if process.is_null() || process == INVALID_HANDLE_VALUE {
        work.unavailable_processes += 1;
        return Ok(());
    }
    let process = FileHandle(process);
    let identity = match process_identity(&process, pid) {
        Ok(identity) => identity,
        Err(_) => {
            work.unavailable_processes += 1;
            return Ok(());
        }
    };
    let mut observed = BTreeSet::new();
    for &value in values {
        work.attempted_handles += 1;
        let Ok(handle) = duplicate(&process, value) else {
            work.unavailable_handles += 1;
            continue;
        };
        // SAFETY: this worker owns the duplicated handle. Skip pipes/devices before name queries.
        if unsafe { GetFileType(handle.0) } != FILE_TYPE_DISK {
            continue;
        }
        work.path_queries += 1;
        match final_path(&handle) {
            Ok(path) if target.contains(&path) => {
                observed.insert(path);
            }
            Ok(_) => {}
            Err(_) => work.unavailable_handles += 1,
        }
    }
    // A pinned process cannot be replaced by another PID lifetime. Still reject
    // observations after exit or a failed identity check before publishing them.
    // SAFETY: valid owned process handle with SYNCHRONIZE; zero timeout never blocks.
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT
        || !process_identity(&process, pid).is_ok_and(|current| current == identity)
    {
        return Ok(());
    }
    if !observed.is_empty() {
        work.users.insert(identity);
        if fixtures.contains(&pid) {
            work.fixture_users.insert(identity);
            work.fixture_files.extend(observed);
        }
    }
    Ok(())
}

pub(super) fn inspect(root: &Path, fixtures: &[u32]) -> Result<Value> {
    let target = Target::new(root)?;
    let known_file = open_metadata(&super::file_path(root, 0))?;
    let started = Instant::now();
    let entries = system_handles()?;
    let snapshot_ms = started.elapsed().as_secs_f64() * 1000.0;
    let file_type = entries
        .iter()
        .find(|entry| {
            entry.process_id == std::process::id() as usize
                && entry.handle_value == known_file.0 as usize
        })
        .ok_or("native snapshot omitted the known file-type probe")?
        .object_type;
    let mut processes = BTreeMap::<u32, Vec<usize>>::new();
    for entry in &entries {
        let Ok(pid) = u32::try_from(entry.process_id) else {
            return Err("invalid native process ID".into());
        };
        if entry.object_type == file_type && pid > 0 && pid != std::process::id() {
            processes.entry(pid).or_default().push(entry.handle_value);
        }
    }
    let groups: Vec<_> = processes.into_iter().collect();
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .clamp(1, 8);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| {
                let mut work = Work::default();
                while let Some((pid, values)) = groups.get(next.fetch_add(1, Ordering::Relaxed)) {
                    inspect_process(*pid, values, &target, fixtures, &mut work)?;
                }
                oflh_core::Result::Ok(work)
            }));
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "native probe worker panicked")
                    .and_then(|result| result.map_err(|_| "native probe worker failed"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()
    })?;
    let mut total = Work::default();
    for work in results {
        total.processes += work.processes;
        total.unavailable_processes += work.unavailable_processes;
        total.attempted_handles += work.attempted_handles;
        total.unavailable_handles += work.unavailable_handles;
        total.path_queries += work.path_queries;
        total.users.extend(work.users);
        total.fixture_users.extend(work.fixture_users);
        total.fixture_files.extend(work.fixture_files);
    }
    Ok(
        json!({"outcome":"completed","snapshot_handles":entries.len(),"snapshot_ms":snapshot_ms,
        "discovery_ms":started.elapsed().as_secs_f64()*1000.0,"workers":workers,
        "processes":total.processes,"unavailable_processes":total.unavailable_processes,
        "attempted_file_handles":total.attempted_handles,"path_queries":total.path_queries,
        "unavailable_handles":total.unavailable_handles,"matching_users":total.users.len(),
        "found_fixture_users":total.fixture_users.len(),"expected_fixture_users":fixtures.len(),
        "found_fixture_files":total.fixture_files.len(),
        "scope":"open disk handles only; excludes modules, closed-handle mappings, locks and full metadata"}),
    )
}

pub(super) fn isolated(root: &Path, pids: &[u32]) -> Result<Value> {
    let mut child = Fixture(
        Command::new(std::env::current_exe()?)
            .arg("--handles-worker")
            .arg(root)
            .arg(
                pids.iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.0.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            return Ok(json!({"outcome":"budget_exceeded","budget_seconds":30}));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child
        .0
        .stdout
        .take()
        .ok_or("missing isolated probe output")?;
    Ok(serde_json::from_reader(output).unwrap_or_else(|_| json!({"outcome":"worker_failed"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handle_snapshot_layout_and_extents_are_checked() {
        assert_eq!(size_of::<HandleEntry>(), 40);
        assert_eq!(std::mem::offset_of!(HandleEntry, object_type), 30);
        let mut buffer = vec![0usize; 7];
        buffer[0] = 1;
        assert_eq!(decode_entries(&buffer, 56).unwrap().len(), 1);
        assert!(decode_entries(&buffer, 55).is_err());
        assert!(decode_entries(&buffer, 57).is_err());
        assert!(decode_entries(&buffer, 8).is_err());
        buffer[0] = usize::MAX;
        assert!(decode_entries(&buffer, 56).is_err());
        assert!(decode_entries(&[], 0).is_err());
        assert!(decode_entries(&[0, 0], 16).unwrap().is_empty());
    }
}
