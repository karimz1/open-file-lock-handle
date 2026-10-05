use super::*;
use std::{
    ffi::OsString,
    mem::size_of,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    Storage::FileSystem::*,
    System::{Diagnostics::ToolHelp::*, ProcessStatus::*, RestartManager::*, Threading::*},
    UI::WindowsAndMessaging::*,
};
struct Handle(HANDLE);
impl Handle {
    fn new(handle: HANDLE, operation: &'static str) -> Result<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            Err(io(operation, std::io::Error::last_os_error()))
        } else {
            Ok(Self(handle))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Handle owns exactly one valid handle and never exposes ownership.
        unsafe { CloseHandle(self.0) };
    }
}
fn open(pid: u32, access: u32) -> Result<Handle> {
    // SAFETY: OpenProcess accepts numeric access and PID and returns a new owned handle.
    Handle::new(unsafe { OpenProcess(access, 0, pid) }, "open process")
}
fn wide(path: &Path) -> Result<Vec<u16>> {
    let mut encoded: Vec<_> = path.as_os_str().encode_wide().collect();
    if encoded.contains(&0) {
        return Err(Error::Unavailable("path contains NUL".into()));
    }
    encoded.push(0);
    Ok(encoded)
}
fn path(encoded: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(
        &encoded[..encoded
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(encoded.len())],
    ))
}
fn ticks(file_time: FILETIME) -> u64 {
    (u64::from(file_time.dwHighDateTime) << 32) | u64::from(file_time.dwLowDateTime)
}
fn times(handle: &Handle, pid: u32) -> Result<(Identity, u64)> {
    let mut created = FILETIME::default();
    let mut exit = created;
    let mut kernel = created;
    let mut user = created;
    // SAFETY: process handle is live and all outputs are writable FILETIME values.
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(io("read process identity", std::io::Error::last_os_error()));
    }
    Ok((
        Identity {
            pid,
            started: ticks(created),
            started_sub: 0,
        },
        ticks(kernel)
            .saturating_add(ticks(user))
            .saturating_mul(100),
    ))
}
fn read_identity(pid: u32) -> Result<Identity> {
    times(&open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?, pid).map(|v| v.0)
}
fn read_process(pid: u32) -> Result<Process> {
    let handle = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    let (identity, _) = times(&handle, pid)?;
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    // SAFETY: buffer capacity matches len; the owned handle remains open.
    let executable =
        if unsafe { QueryFullProcessImageNameW(handle.0, 0, buf.as_mut_ptr(), &mut len) } != 0 {
            path(&buf[..len as usize])
        } else {
            PathBuf::new()
        };
    let name = executable
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Process {
        identity,
        name,
        executable,
        user: "unknown".into(),
        ..Process::default()
    })
}
fn username(pid: u32) -> String {
    let Ok(handle) = open(pid, PROCESS_QUERY_LIMITED_INFORMATION) else {
        return "unknown".into();
    };
    let mut token = null_mut();
    // SAFETY: output is a writable handle slot and process handle is valid.
    if unsafe { OpenProcessToken(handle.0, TOKEN_QUERY, &mut token) } == 0 {
        return "unknown".into();
    }
    let Ok(token) = Handle::new(token, "open process token") else {
        return "unknown".into();
    };
    let mut needed = 0;
    // SAFETY: null buffer with zero length queries required size.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed) };
    if needed == 0 || needed > 1024 * 1024 {
        return "unknown".into();
    }
    let mut data = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    // SAFETY: word-aligned allocation holds at least needed writable bytes.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            data.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return "unknown".into();
    }
    // SAFETY: successful TokenUser query initialized a TOKEN_USER at this aligned address.
    let sid = unsafe { (*(data.as_ptr().cast::<TOKEN_USER>())).User.Sid };
    let mut name = vec![0u16; 256];
    let mut domain = vec![0u16; 256];
    let mut name_length = name.len() as u32;
    let mut domain_length = domain.len() as u32;
    let mut kind = 0;
    // SAFETY: SID belongs to live data buffer; names have lengths specified by name_length/domain_length.
    if unsafe {
        LookupAccountSidW(
            null(),
            sid,
            name.as_mut_ptr(),
            &mut name_length,
            domain.as_mut_ptr(),
            &mut domain_length,
            &mut kind,
        )
    } == 0
    {
        return "unknown".into();
    }
    let name = String::from_utf16_lossy(&name[..name_length as usize]);
    let domain = String::from_utf16_lossy(&domain[..domain_length as usize]);
    if domain.is_empty() {
        name
    } else {
        format!("{domain}\\{name}")
    }
}
fn file_id(path: &Path) -> Option<(u32, u64)> {
    let name = wide(path).ok()?;
    // SAFETY: terminated UTF-16 path; metadata-only open, maximal sharing; no mutation.
    let handle = Handle::new(
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
        "inspect file identity",
    )
    .ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: live file handle and writable correctly sized output.
    if unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0 {
        return None;
    }
    Some((
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}
/// Cache metadata probes within one scan; lifetime validation remains fresh.
struct FileMatcher<'a> {
    target: &'a Target,
    target_id: Option<(u32, u64)>,
    identities: HashMap<PathBuf, Option<(u32, u64)>>,
}
impl<'a> FileMatcher<'a> {
    fn new(target: &'a Target, cancel: &Cancellation) -> Self {
        let target_id = if target.directory {
            None
        } else {
            cancel.record(InspectionCounter::FileIdentityQueries, 1);
            file_id(&target.path)
        };
        Self {
            target,
            target_id,
            identities: HashMap::new(),
        }
    }
    fn matches(&mut self, path: &Path, cancel: &Cancellation) -> bool {
        if self.target.directory {
            return self.target.contains(path);
        }
        let Some(target_id) = self.target_id else {
            return self.target.matches(path, None);
        };
        let candidate = *self.identities.entry(path.to_owned()).or_insert_with(|| {
            cancel.record(InspectionCounter::FileIdentityQueries, 1);
            file_id(path)
        });
        candidate.map_or_else(
            || self.target.matches(path, None),
            |identity| target_id == identity,
        )
    }
}
struct Session(u32);
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: session was acquired by RmStartSession and is ended exactly once.
        unsafe { RmEndSession(self.0) };
    }
}
fn rm_users(paths: &[PathBuf], cancel: &Cancellation) -> Result<Vec<RM_PROCESS_INFO>> {
    cancel.check()?;
    cancel.record(InspectionCounter::ResourceQueries, 1);
    let _timer = cancel.measure(InspectionCounter::ResourceQueryMicros);
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut session = 0;
    let mut key = [0u16; 33];
    // SAFETY: outputs match documented Restart Manager session and key sizes.
    let code = unsafe { RmStartSession(&mut session, 0, key.as_mut_ptr()) };
    if code != 0 {
        return Err(io(
            "start Restart Manager",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    let session = Session(session);
    let names: Vec<_> = paths
        .iter()
        .map(|process| wide(process))
        .collect::<Result<_>>()?;
    let pointers: Vec<_> = names
        .iter()
        .map(|name_length| name_length.as_ptr())
        .collect();
    // SAFETY: path pointers refer to terminated strings kept alive through this synchronous call.
    let code = unsafe {
        RmRegisterResources(
            session.0,
            pointers.len() as u32,
            pointers.as_ptr(),
            0,
            null(),
            0,
            null(),
        )
    };
    if code != 0 {
        return Err(io(
            "register resources",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    cancel.check()?;
    let mut records = Vec::<RM_PROCESS_INFO>::new();
    for _ in 0..5 {
        cancel.check()?;
        let mut needed = 0;
        let mut count = records.len() as u32;
        let mut reboot = 0;
        // SAFETY: empty list uses null; otherwise buffer has count writable initialized records.
        let code = unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                if records.is_empty() {
                    null_mut()
                } else {
                    records.as_mut_ptr()
                },
                &mut reboot,
            )
        };
        if code == 0 {
            if count as usize > records.len() {
                return Err(Error::Unavailable(
                    "invalid Restart Manager result length".into(),
                ));
            }
            records.truncate(count as usize);
            return Ok(records);
        }
        if code != ERROR_MORE_DATA {
            return Err(io(
                "query resource users",
                std::io::Error::from_raw_os_error(code as i32),
            ));
        }
        if needed > 1_000_000 {
            return Err(Error::Unavailable(
                "Restart Manager result too large".into(),
            ));
        }
        records.resize_with(needed as usize + 16, RM_PROCESS_INFO::default);
    }
    Err(Error::Unavailable(
        "resources changed too quickly; refresh".into(),
    ))
}
fn sharing(path: &Path) -> Option<LockEvidence> {
    let name = wide(path).ok()?;
    for (access, kind) in [
        (GENERIC_READ, AccessKind::Read),
        (GENERIC_WRITE, AccessKind::Write),
        (DELETE, AccessKind::Delete),
    ] {
        // SAFETY: existing file only, maximal sharing; probes neither write data nor acquire byte-range locks.
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError has no preconditions and immediately follows failing call.
            if unsafe { GetLastError() } == ERROR_SHARING_VIOLATION {
                return Some(LockEvidence::SharingConflict(kind));
            }
        } else if !handle.is_null() {
            drop(Handle(handle))
        }
    }
    None
}
#[derive(Default)]
pub struct Native {
    sampler: Sampler,
}
fn process_snapshot() -> Result<(Handle, PROCESSENTRY32W)> {
    // SAFETY: valid snapshot flags; API returns an owned handle.
    let handle = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        "snapshot processes",
    )?;
    let entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    Ok((handle, entry))
}
impl Backend for Native {
    fn scan(&mut self, target: &Target, cancel: &Cancellation) -> Result<Snapshot> {
        cancel.check()?;
        cancel.set_phase(InspectionPhase::Processes);
        let mut snapshot = Snapshot {
            warnings: vec![
                "Windows: CWD, directory handles and deleted files are not visible. Sharing conflicts are per file; reported users are not proven lock owners. Byte-range locks are not enumerated.".into(),
            ],
            ..Snapshot::default()
        };
        let mut limited = 0;
        let mut parents = HashMap::new();
        let mut matcher = FileMatcher::new(target, cancel);
        let (handle, mut entry) = process_snapshot()?;
        // SAFETY: initialized size field and live snapshot handle.
        let mut ok = unsafe { Process32FirstW(handle.0, &mut entry) };
        while ok != 0 {
            cancel.check()?;
            let pid = entry.th32ProcessID;
            parents.insert(pid, entry.th32ParentProcessID);
            if pid > 0 && pid != std::process::id() {
                cancel.record(InspectionCounter::Processes, 1);
                if let Ok(mut process) = read_process(pid) {
                    if !process.executable.as_os_str().is_empty()
                        && matcher.matches(&process.executable, cancel)
                    {
                        process.usages.push(Usage {
                            path: process.executable.clone(),
                            relation: Relation::Executable,
                            access: Access::Execute,
                            ..Usage::default()
                        })
                    }
                    let mut modules = Err(Error::Unavailable("module snapshot unavailable".into()));
                    for _ in 0..3 {
                        cancel.check()?;
                        cancel.record(InspectionCounter::ModuleSnapshots, 1);
                        let timer = cancel.measure(InspectionCounter::ModuleSnapshotMicros);
                        // SAFETY: documented Toolhelp flags, observed positive PID.
                        modules = Handle::new(
                            unsafe {
                                CreateToolhelp32Snapshot(
                                    TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32,
                                    pid,
                                )
                            },
                            "snapshot modules",
                        );
                        drop(timer);
                        if !matches!(
                            &modules,
                            Err(Error::Io { source, .. })
                                if source.raw_os_error() == Some(ERROR_BAD_LENGTH as i32)
                        ) {
                            break;
                        }
                    }
                    if let Ok(modules) = modules {
                        let mut module = MODULEENTRY32W {
                            dwSize: size_of::<MODULEENTRY32W>() as u32,
                            ..Default::default()
                        };
                        // SAFETY: live snapshot and initialized output size.
                        let mut next = unsafe { Module32FirstW(modules.0, &mut module) };
                        while next != 0 {
                            cancel.check()?;
                            cancel.record(InspectionCounter::Resources, 1);
                            let path = path(&module.szExePath);
                            if matcher.matches(&path, cancel) {
                                process.usages.push(Usage {
                                    path,
                                    relation: Relation::Mapped,
                                    access: Access::Execute,
                                    ..Usage::default()
                                })
                            }
                            // SAFETY: same valid snapshot/output as above.
                            next = unsafe { Module32NextW(modules.0, &mut module) };
                        }
                    } else {
                        limited += 1
                    }
                    if !process.usages.is_empty()
                        && read_identity(pid).is_ok_and(|identity| identity == process.identity)
                    {
                        snapshot.processes.push(process)
                    }
                } else {
                    limited += 1
                }
            }
            // SAFETY: same live process snapshot and output.
            ok = unsafe { Process32NextW(handle.0, &mut entry) };
        }
        // SAFETY: capture failure code immediately after enumeration ends.
        let code = unsafe { GetLastError() };
        if code != ERROR_NO_MORE_FILES {
            return Err(io(
                "enumerate processes",
                std::io::Error::from_raw_os_error(code as i32),
            ));
        }
        collect_resource_users(target, &mut snapshot, &mut limited, cancel)?;
        snapshot.normalize();
        for process in &mut snapshot.processes {
            cancel.check()?;
            process.user = username(process.identity.pid);
            process.parent = parents.get(&process.identity.pid).copied().unwrap_or(0);
            let mut next = process.parent;
            while next > 0
                && next != process.identity.pid
                && process.ancestors.len() < 8
                && !process.ancestors.iter().any(|a| a.identity.pid == next)
            {
                cancel.check()?;
                match read_process(next) {
                    Ok(a) => {
                        process.ancestors.push(Ancestor {
                            identity: a.identity,
                            name: a.name,
                        });
                        next = parents.get(&next).copied().unwrap_or(0)
                    }
                    Err(_) => {
                        process.ancestors.push(Ancestor {
                            identity: Identity {
                                pid: next,
                                ..Identity::default()
                            },
                            name: "unavailable".into(),
                        });
                        break;
                    }
                }
            }
        }
        if limited > 0 {
            snapshot.warnings.push(format!(
                "{limited} process/resource inspections were unavailable (permissions or changes)."
            ))
        }
        let ids = snapshot
            .processes
            .iter()
            .map(|process| process.identity)
            .collect::<Vec<_>>();
        apply_metrics(&mut snapshot, self.sample(&ids, cancel)?);
        Ok(snapshot)
    }
    fn sample(
        &mut self,
        ids: &[Identity],
        cancel: &Cancellation,
    ) -> Result<Vec<(Identity, Metrics)>> {
        let mut raw = Vec::with_capacity(ids.len());
        for &identity in ids {
            cancel.check()?;
            let Ok(handle) = open(identity.pid, PROCESS_QUERY_INFORMATION | PROCESS_VM_READ) else {
                continue;
            };
            let Ok((current, cpu)) = times(&handle, identity.pid) else {
                continue;
            };
            if current != identity {
                continue;
            }
            let mut memory_info = PROCESS_MEMORY_COUNTERS {
                cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                ..Default::default()
            };
            // SAFETY: live process handle, writable PROCESS_MEMORY_COUNTERS with its exact size.
            let memory = if unsafe {
                GetProcessMemoryInfo(
                    handle.0,
                    &mut memory_info,
                    size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                )
            } != 0
            {
                Some(memory_info.WorkingSetSize as u64)
            } else {
                None
            };
            raw.push((identity, cpu, memory));
        }
        let total = self.sampler.clock().saturating_mul(
            std::thread::available_parallelism().map_or(1, |count| count.get()) as u64,
        );
        Ok(self.sampler.sample(raw, total))
    }
    fn is_running(&mut self, identity: Identity) -> Result<bool> {
        let handle = match open(identity.pid, PROCESS_QUERY_LIMITED_INFORMATION | 0x00100000) {
            Ok(handle) => handle,
            Err(Error::Io { source, .. })
                if source.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) =>
            {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        if times(&handle, identity.pid)?.0 != identity {
            return Ok(false);
        }
        // SAFETY: the owned handle pins the process and grants SYNCHRONIZE access; zero timeout never blocks.
        match unsafe { WaitForSingleObject(handle.0, 0) } {
            WAIT_OBJECT_0 => Ok(false),
            WAIT_TIMEOUT => Ok(true),
            _ => Err(io("verify process exit", std::io::Error::last_os_error())),
        }
    }
    fn terminate(&mut self, identity: Identity, force: bool, cancel: &Cancellation) -> Result<()> {
        identity.validate()?;
        cancel.check()?;
        let handle = open(
            identity.pid,
            PROCESS_QUERY_LIMITED_INFORMATION
                | 0x00100000
                | if force { PROCESS_TERMINATE } else { 0 },
        )?;
        if times(&handle, identity.pid)?.0 != identity {
            return Err(Error::Changed);
        }
        if force {
            // SAFETY: owned process handle pins the exact validated lifetime and has terminate access.
            if unsafe { TerminateProcess(handle.0, 1) } == 0 {
                return Err(io("terminate process", std::io::Error::last_os_error()));
            }
            return Ok(());
        }
        let mut state = CloseState {
            pid: identity.pid,
            sent: 0,
            handle: handle.0,
        };
        // SAFETY: callback receives a pointer to state, valid for synchronous EnumWindows. the owned handle remains open.
        if unsafe { EnumWindows(Some(close_window), (&mut state as *mut CloseState) as isize) } == 0
        {
            return Err(io("enumerate windows", std::io::Error::last_os_error()));
        }
        if state.sent == 0 {
            return Err(Error::Unavailable(
                "no window accepted a graceful close request; use force kill explicitly if needed"
                    .into(),
            ));
        }
        Ok(())
    }
}
struct CloseState {
    pid: u32,
    sent: usize,
    handle: HANDLE,
}
unsafe extern "system" fn close_window(hwnd: HWND, param: isize) -> i32 {
    // SAFETY: only called synchronously by our EnumWindows with the live CloseState address.
    let state = unsafe { &mut *(param as *mut CloseState) };
    let mut pid = 0;
    // SAFETY: OS supplies hwnd and writable PID slot. A still-running pinned process cannot have its PID reused.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == state.pid
            && WaitForSingleObject(state.handle, 0) == WAIT_TIMEOUT
            && PostMessageW(hwnd, WM_CLOSE, 0, 0) != 0
        {
            state.sent += 1
        }
    }
    1
}
// Empty areas need only one expensive registration per batch. Occupied batches
// still split to singleton files: a batch user does not prove use of every file.
const RESOURCE_BATCH_SIZE: usize = 128;
const MAX_RESOURCE_BATCH_SIZE: usize = 1024;
const DIRECTORY_FILE_LIMIT: usize = 10_000;
fn collect_resource_users(
    target: &Target,
    snapshot: &mut Snapshot,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    cancel.set_phase(InspectionPhase::Files);
    let mut cache = HashMap::new();
    let mut batch = Vec::with_capacity(MAX_RESOURCE_BATCH_SIZE);
    let mut batch_size = RESOURCE_BATCH_SIZE;
    let mut count = 0;
    let mut stack = vec![target.path.clone()];
    while let Some(path) = stack.pop() {
        cancel.check()?;
        if target.directory {
            // Carry small tails across folders. Flushing every tiny directory
            // turns a root scan into thousands of expensive registrations.
            if batch.len() >= RESOURCE_BATCH_SIZE {
                flush_resource_batch(
                    &mut batch,
                    &mut batch_size,
                    snapshot,
                    &mut cache,
                    limited,
                    cancel,
                )?;
            }
            batch_size = RESOURCE_BATCH_SIZE;
            cancel.record(InspectionCounter::Directories, 1);
            let entries = match std::fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(_) => {
                    *limited += 1;
                    continue;
                }
            };
            for entry in entries {
                cancel.check()?;
                let Ok(entry) = entry else {
                    *limited += 1;
                    continue;
                };
                let Ok(kind) = entry.file_type() else {
                    *limited += 1;
                    continue;
                };
                if kind.is_dir() {
                    stack.push(entry.path())
                } else if kind.is_file() {
                    batch.push(entry.path());
                    count += 1;
                    if batch.len() == batch_size {
                        flush_resource_batch(
                            &mut batch,
                            &mut batch_size,
                            snapshot,
                            &mut cache,
                            limited,
                            cancel,
                        )?;
                    }
                    if count >= DIRECTORY_FILE_LIMIT {
                        break;
                    }
                }
            }
        } else {
            batch.push(path);
            count += 1
        }
        if count >= DIRECTORY_FILE_LIMIT {
            snapshot.warnings.push(
                "Directory scan limited to 10,000 files. Narrow the target for complete coverage."
                    .into(),
            );
            break;
        }
    }
    if !batch.is_empty() {
        correlate(&batch, snapshot, &mut cache, limited, cancel)?;
    }
    Ok(())
}

fn flush_resource_batch(
    batch: &mut Vec<PathBuf>,
    batch_size: &mut usize,
    snapshot: &mut Snapshot,
    cache: &mut HashMap<Identity, Process>,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    let occupied = correlate(batch, snapshot, cache, limited, cancel)?;
    *batch_size = if occupied {
        RESOURCE_BATCH_SIZE
    } else {
        (*batch_size * 2).min(MAX_RESOURCE_BATCH_SIZE)
    };
    batch.clear();
    Ok(())
}

fn correlate(
    paths: &[PathBuf],
    snapshot: &mut Snapshot,
    cache: &mut HashMap<Identity, Process>,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<bool> {
    cancel.check()?;
    let mut apps = match rm_users(paths, cancel) {
        Ok(a) => a,
        Err(Error::Cancelled) => return Err(Error::Cancelled),
        Err(error) => {
            *limited += 1;
            cancel.record(InspectionCounter::Files, paths.len() as u64);
            if !snapshot
                .warnings
                .iter()
                .any(|warning| warning.starts_with("Restart Manager query failed:"))
            {
                snapshot
                    .warnings
                    .push(format!("Restart Manager query failed: {error}"));
            }
            return Ok(true);
        }
    };
    // Self users are never emitted, so self-only batches need no subdivision.
    apps.retain(|app| app.Process.dwProcessId != std::process::id());
    if apps.is_empty() {
        cancel.record(InspectionCounter::Files, paths.len() as u64);
        return Ok(false);
    }
    if paths.len() > 1 {
        let mid = paths.len() / 2;
        correlate(&paths[..mid], snapshot, cache, limited, cancel)?;
        correlate(&paths[mid..], snapshot, cache, limited, cancel)?;
        return Ok(true);
    }
    cancel.record(InspectionCounter::Files, 1);
    let lock = sharing(&paths[0]);
    for app in apps {
        let identity = Identity {
            pid: app.Process.dwProcessId,
            started: ticks(app.Process.ProcessStartTime),
            started_sub: 0,
        };
        if identity.pid == std::process::id() {
            continue;
        }
        let mut process = if let Some(process) = cache.get(&identity) {
            if !read_identity(identity.pid).is_ok_and(|i| i == identity) {
                continue;
            }
            process.clone()
        } else {
            match read_process(identity.pid) {
                Ok(process) if process.identity == identity => {
                    cache.insert(identity, process.clone());
                    process
                }
                Ok(_) => continue,
                Err(_) => {
                    *limited += 1;
                    Process {
                        identity,
                        name: path(&app.strAppName).to_string_lossy().into_owned(),
                        user: "unknown".into(),
                        ..Process::default()
                    }
                }
            }
        };
        process.usages.push(Usage {
            path: paths[0].clone(),
            relation: Relation::RestartManager,
            ..Usage::default()
        });
        if let Some(lock) = &lock {
            process.usages.push(Usage {
                path: paths[0].clone(),
                relation: Relation::Locked,
                lock: Some(lock.clone()),
                ..Usage::default()
            })
        }
        snapshot.processes.push(process);
    }
    Ok(true)
}

pub(super) fn port_identity(pid: u32) -> Result<Identity> {
    read_identity(pid)
}

pub(super) fn port_processes(cancel: &Cancellation) -> Result<Vec<Process>> {
    let (handle, mut entry) = process_snapshot()?;
    let mut processes = Vec::new();
    // SAFETY: initialized structure size and live owned snapshot handle.
    let mut next = unsafe { Process32FirstW(handle.0, &mut entry) };
    while next != 0 {
        cancel.check()?;
        if let Ok(mut process) = read_process(entry.th32ProcessID) {
            process.parent = entry.th32ParentProcessID;
            processes.push(process);
        }
        // SAFETY: same live snapshot and correctly sized output record.
        next = unsafe { Process32NextW(handle.0, &mut entry) };
    }
    // SAFETY: capture the enumeration result immediately after the final native call.
    let code = unsafe { GetLastError() };
    if code != ERROR_NO_MORE_FILES {
        return Err(io(
            "enumerate port owners",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    Ok(processes)
}

pub(super) fn launch_elevated(executable: &Path, arguments: &str) -> Result<u32> {
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    let executable = wide(executable)?;
    let parameters: Vec<u16> = arguments.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
    // SAFETY: zero is a valid initial state for this Win32 structure; cbSize is set below.
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = executable.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    // SAFETY: structure size and all NUL-terminated buffers remain valid through the synchronous call.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = std::io::Error::last_os_error();
        return Err(io("request UAC authorization", error));
    }
    let handle = Handle::new(info.hProcess, "open administrator helper")?;
    // SAFETY: handle owns the helper process, and waiting runs off the UI thread.
    if unsafe { WaitForSingleObject(handle.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err(io(
            "wait for administrator helper",
            std::io::Error::last_os_error(),
        ));
    }
    let mut code = 0;
    // SAFETY: valid owned process handle and writable DWORD output.
    if unsafe { GetExitCodeProcess(handle.0, &mut code) } == 0 {
        return Err(io(
            "read administrator helper result",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(code)
}

#[cfg(test)]
mod inspection_tests {
    use super::*;
    #[test]
    fn small_directories_share_batches_instead_of_registering_each_file() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..256 {
            let folder = directory.path().join(format!("small-{index:04}"));
            std::fs::create_dir(&folder).unwrap();
            std::fs::write(folder.join("unused.bin"), [0; 64]).unwrap();
        }
        let cancel = Cancellation::default();
        let mut snapshot = Snapshot::default();
        let mut limited = 0;
        collect_resource_users(
            &Target::new(directory.path()).unwrap(),
            &mut snapshot,
            &mut limited,
            &cancel,
        )
        .unwrap();
        assert_eq!(limited, 0);
        assert_eq!(cancel.progress().directories, 257);
        assert_eq!(cancel.progress().files, 256);
        assert_eq!(cancel.progress().resource_queries, 2);
    }
    #[test]
    fn self_only_resources_do_not_subdivide_or_publish_users() {
        let directory = tempfile::tempdir().unwrap();
        let mut held = Vec::new();
        for index in 0..MAX_RESOURCE_BATCH_SIZE {
            let path = directory.path().join(format!("self-{index:04}.bin"));
            std::fs::write(&path, [0; 64]).unwrap();
            held.push(
                std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .unwrap(),
            );
        }
        let cancel = Cancellation::default();
        let mut snapshot = Snapshot::default();
        let mut limited = 0;
        collect_resource_users(
            &Target::new(directory.path()).unwrap(),
            &mut snapshot,
            &mut limited,
            &cancel,
        )
        .unwrap();
        assert_eq!(limited, 0);
        assert!(snapshot.processes.is_empty());
        assert_eq!(cancel.progress().resource_queries, 4);
        assert_eq!(cancel.progress().files, MAX_RESOURCE_BATCH_SIZE as u64);
        assert_eq!(held.len(), MAX_RESOURCE_BATCH_SIZE);
    }
    #[test]
    fn identity_cache_preserves_alias_matching_and_is_scoped_to_one_scan() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("original.bin");
        let alias = directory.path().join("alias.bin");
        std::fs::write(&original, [0; 64]).unwrap();
        std::fs::hard_link(&original, &alias).unwrap();
        let target = Target::new(&original).unwrap();
        let cancel = Cancellation::default();
        let mut matcher = FileMatcher::new(&target, &cancel);
        for _ in 0..10 {
            assert!(matcher.matches(&alias, &cancel));
        }
        assert_eq!(cancel.progress().file_identity_queries, 2);
        // A new scan observes a replaced alias rather than reusing cached metadata.
        std::fs::remove_file(&alias).unwrap();
        std::fs::write(&alias, [1; 64]).unwrap();
        let mut matcher = FileMatcher::new(&target, &cancel);
        assert!(!matcher.matches(&alias, &cancel));
    }
}

#[cfg(test)]
mod coverage_limit_tests {
    use super::*;
    #[test]
    fn directory_file_cap_remains_visible_and_bounded() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..=DIRECTORY_FILE_LIMIT {
            std::fs::write(directory.path().join(format!("unused-{index:05}.bin")), []).unwrap();
        }
        let cancel = Cancellation::default();
        let mut snapshot = Snapshot::default();
        collect_resource_users(
            &Target::new(directory.path()).unwrap(),
            &mut snapshot,
            &mut 0,
            &cancel,
        )
        .unwrap();
        assert_eq!(cancel.progress().files, DIRECTORY_FILE_LIMIT as u64);
        assert!(cancel.progress().resource_queries <= 16);
        assert!(
            snapshot
                .warnings
                .iter()
                .any(|warning| warning.contains("limited to 10,000 files"))
        );
    }
}
