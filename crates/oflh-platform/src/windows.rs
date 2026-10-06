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
mod file_users;
pub(crate) mod handles;
pub(crate) mod helper_job;
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
    helper: Option<crate::inspection_helper::InspectionHelperCommand>,
}
impl Native {
    pub(crate) fn with_helper(helper: crate::inspection_helper::InspectionHelperCommand) -> Self {
        Self {
            helper: Some(helper),
            ..Self::default()
        }
    }
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
                "Windows: CWD classification and byte-range locks are not enumerated. Protected processes, unresolved native paths and mapping aliases can limit coverage. Sharing conflicts are per file; reported users are not proven lock owners.".into(),
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
        if target.directory {
            collect_handle_users(
                self.helper.as_ref(),
                target,
                &mut snapshot,
                &mut limited,
                cancel,
            )?;
        } else {
            collect_resource_users(target, &mut snapshot, &mut limited, cancel)?;
        }
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
fn record_handle_progress(
    previous: crate::inspection_protocol::Progress,
    current: crate::inspection_protocol::Progress,
    cancel: &Cancellation,
) {
    // The transport rejects backwards counters before delivering a frame.
    for (counter, amount) in [
        (
            InspectionCounter::Resources,
            current.handles - previous.handles + current.mapped_names - previous.mapped_names,
        ),
        (
            InspectionCounter::NativeHandleNames,
            current.names - previous.names,
        ),
        (
            InspectionCounter::MemoryRegions,
            current.regions - previous.regions,
        ),
        (
            InspectionCounter::MappedNames,
            current.mapped_names - previous.mapped_names,
        ),
        (
            InspectionCounter::NativeHandleSnapshots,
            current.snapshots - previous.snapshots,
        ),
        (
            InspectionCounter::NativeHandleSnapshotMicros,
            current.snapshot_micros - previous.snapshot_micros,
        ),
    ] {
        cancel.record(counter, amount);
    }
}

fn collect_handle_users(
    configured: Option<&crate::inspection_helper::InspectionHelperCommand>,
    target: &Target,
    snapshot: &mut Snapshot,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    use crate::{inspection_protocol::*, inspection_transport::Outcome};
    cancel.set_phase(InspectionPhase::Files);
    let configuration = configured.cloned().map_or_else(
        crate::inspection_helper::InspectionHelperCommand::current,
        Ok,
    )?;
    let request = Request {
        path: target.path.as_os_str().encode_wide().collect(),
        owner: read_identity(std::process::id())?,
    };
    let mut previous = Progress::default();
    let mut observed = HashMap::<Identity, Vec<Usage>>::new();
    let mut files = std::collections::HashSet::new();
    let mut warnings = std::collections::BTreeMap::<(Failure, Option<i32>), u64>::new();
    cancel.record(
        InspectionCounter::ResourceWorkers,
        handles::worker_count() as u64,
    );
    let outcome =
        crate::inspection_transport::inspect(&configuration, request, cancel, |message| {
            cancel.check()?;
            match message {
                Message::Observation(observation) => {
                    let path = path(&observation.path);
                    // Treat helper data as observations, never as unchecked action targets.
                    if !target.contains(&path) {
                        return Err(Error::Unavailable(
                            "inspection helper returned a path outside the target".into(),
                        ));
                    }
                    if !observation.directory && files.insert(path.clone()) {
                        cancel.record(InspectionCounter::Files, 1);
                    }
                    let usages = observed.entry(observation.identity).or_default();
                    let usage = Usage {
                        path,
                        relation: if observation.mapped {
                            Relation::Mapped
                        } else {
                            Relation::Open
                        },
                        access: if observation.directory {
                            Access::Directory
                        } else if observation.mapped {
                            Access::Mapped
                        } else {
                            Access::Unknown
                        },
                        deleted: observation.deleted,
                        lock: None,
                    };
                    if let Some(kind) = observation.sharing {
                        usages.push(Usage {
                            relation: Relation::Locked,
                            access: Access::Unknown,
                            lock: Some(LockEvidence::SharingConflict(kind)),
                            ..usage.clone()
                        });
                    }
                    usages.push(usage);
                }
                Message::Progress(progress) => {
                    record_handle_progress(previous, progress, cancel);
                    previous = progress;
                }
                Message::Warning {
                    operation,
                    code,
                    count,
                } => {
                    *warnings.entry((operation, code)).or_default() += count;
                }
                _ => {
                    return Err(Error::Unavailable(
                        "invalid delivered helper message".into(),
                    ));
                }
            }
            Ok(())
        })?;
    for (identity, usages) in observed {
        cancel.check()?;
        match read_process(identity.pid) {
            Ok(mut process)
                if process.identity == identity
                    && read_identity(identity.pid).is_ok_and(|current| current == identity) =>
            {
                process.usages = usages;
                snapshot.processes.push(process);
            }
            _ => *limited += 1,
        }
    }
    for ((operation, code), count) in warnings {
        snapshot.warnings.push(format!(
            "Windows partial inspection: {count} {} attempts unavailable{}.",
            operation.label(),
            code.map_or_else(String::new, |code| format!(" (OS code {code})"))
        ));
    }
    match outcome {
        Outcome::Complete => Ok(()),
        Outcome::Partial(error) => {
            snapshot
                .warnings
                .push(format!("Windows handle inspection incomplete: {error}"));
            Ok(())
        }
        Outcome::Unavailable(error) => {
            snapshot.warnings.push(format!("Windows handle inspection unavailable: {error}; using limited Restart Manager fallback."));
            collect_resource_users(target, snapshot, limited, cancel)
        }
    }
}

fn collect_resource_users(
    target: &Target,
    snapshot: &mut Snapshot,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    if target.directory {
        collect_resources_parallel(
            target,
            snapshot,
            limited,
            cancel,
            resource_worker_count(),
            &inspect_resource_batch,
        )
    } else {
        collect_resources_serial(target, snapshot, limited, cancel)
    }
}

fn resource_worker_count() -> usize {
    // Registration spends time in the OS. Two workers per logical CPU overlap
    // those waits, with headroom below Restart Manager's per-user session limit.
    std::thread::available_parallelism()
        .map_or(2, |count| count.get())
        .saturating_mul(2)
        .clamp(2, 8)
}

enum ResourceEntry {
    Directory,
    File(PathBuf),
}

struct ResourceWalk {
    limited: usize,
    capped: bool,
}

fn walk_resource_files(
    target: &Target,
    cancel: &Cancellation,
    mut visit: impl FnMut(ResourceEntry) -> Result<()>,
) -> Result<ResourceWalk> {
    let mut limited = 0;
    let mut count = 0;
    let mut stack = vec![target.path.clone()];
    while let Some(path) = stack.pop() {
        cancel.check()?;
        if !target.directory {
            visit(ResourceEntry::File(path))?;
            count += 1;
            continue;
        }
        cancel.record(InspectionCounter::Directories, 1);
        visit(ResourceEntry::Directory)?;
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => {
                limited += 1;
                continue;
            }
        };
        for entry in entries {
            cancel.check()?;
            let Ok(entry) = entry else {
                limited += 1;
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                limited += 1;
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                visit(ResourceEntry::File(entry.path()))?;
                count += 1;
                if count >= DIRECTORY_FILE_LIMIT {
                    return Ok(ResourceWalk {
                        limited,
                        capped: true,
                    });
                }
            }
        }
    }
    Ok(ResourceWalk {
        limited,
        capped: false,
    })
}

fn apply_resource_walk(walk: ResourceWalk, snapshot: &mut Snapshot, limited: &mut usize) {
    *limited += walk.limited;
    if walk.capped {
        snapshot.warnings.push(
            "Directory scan limited to 10,000 files. Narrow the target for complete coverage."
                .into(),
        );
    }
}

fn collect_resources_serial(
    target: &Target,
    snapshot: &mut Snapshot,
    limited: &mut usize,
    cancel: &Cancellation,
) -> Result<()> {
    cancel.set_phase(InspectionPhase::Files);
    let mut cache = HashMap::new();
    let mut batch = Vec::with_capacity(MAX_RESOURCE_BATCH_SIZE);
    let mut batch_size = RESOURCE_BATCH_SIZE;
    let walk = walk_resource_files(target, cancel, |entry| {
        match entry {
            ResourceEntry::Directory => {
                // Carry small tails across folders rather than registering each
                // tiny directory. Growth stays local to one directory.
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
            }
            ResourceEntry::File(path) => {
                batch.push(path);
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
            }
        }
        Ok(())
    })?;
    apply_resource_walk(walk, snapshot, limited);
    if !batch.is_empty() {
        correlate(&batch, snapshot, &mut cache, limited, cancel)?;
    }
    Ok(())
}

#[derive(Default)]
struct ResourceWorker {
    snapshot: Snapshot,
    processes: HashMap<Identity, Process>,
    limited: usize,
}

fn inspect_resource_batch(
    paths: &[PathBuf],
    worker: &mut ResourceWorker,
    cancel: &Cancellation,
) -> Result<()> {
    correlate(
        paths,
        &mut worker.snapshot,
        &mut worker.processes,
        &mut worker.limited,
        cancel,
    )?;
    Ok(())
}

fn run_resource_worker(
    receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<Vec<PathBuf>>>>,
    cancel: &Cancellation,
    inspect: &(impl Fn(&[PathBuf], &mut ResourceWorker, &Cancellation) -> Result<()> + Sync),
) -> Result<ResourceWorker> {
    let mut worker = ResourceWorker::default();
    loop {
        cancel.check()?;
        // Only receipt is serialized. No native call runs under this lock.
        let received = receiver
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .recv();
        let Ok(paths) = received else {
            break;
        };
        cancel.check()?;
        inspect(&paths, &mut worker, cancel)?;
    }
    Ok(worker)
}

fn collect_resources_parallel(
    target: &Target,
    snapshot: &mut Snapshot,
    limited: &mut usize,
    cancel: &Cancellation,
    workers: usize,
    inspect: &(impl Fn(&[PathBuf], &mut ResourceWorker, &Cancellation) -> Result<()> + Sync),
) -> Result<()> {
    cancel.set_phase(InspectionPhase::Files);
    let (sender, receiver) = std::sync::mpsc::sync_channel(workers * 2);
    let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        let mut failure = None;
        for index in 0..workers {
            let receiver = receiver.clone();
            let worker_cancel = cancel.clone();
            match std::thread::Builder::new()
                .name(format!("oflh-resources-{index}"))
                .spawn_scoped(scope, move || {
                    run_resource_worker(receiver, &worker_cancel, inspect)
                }) {
                Ok(handle) => {
                    cancel.record(InspectionCounter::ResourceWorkers, 1);
                    handles.push(handle);
                }
                Err(source) => {
                    failure = Some(io("start resource inspection worker", source));
                    break;
                }
            }
        }
        // Workers own all receiver references. If every worker stops, a blocked
        // send disconnects instead of waiting forever on a parent-owned receiver.
        drop(receiver);
        let walk = if failure.is_none() {
            let mut batch = Vec::with_capacity(RESOURCE_BATCH_SIZE);
            let result = walk_resource_files(target, cancel, |entry| {
                if let ResourceEntry::File(path) = entry {
                    batch.push(path);
                    if batch.len() == RESOURCE_BATCH_SIZE {
                        send_resource_batch(&sender, &mut batch, cancel)?;
                    }
                }
                Ok(())
            });
            result.and_then(|walk| {
                if !batch.is_empty() {
                    send_resource_batch(&sender, &mut batch, cancel)?;
                }
                Ok(walk)
            })
        } else {
            Err(Error::Unavailable(
                "resource workers could not start".into(),
            ))
        };
        drop(sender);
        let mut completed = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.join() {
                Ok(Ok(worker)) => completed.push(worker),
                Ok(Err(error)) => {
                    if failure.is_none() {
                        failure = Some(error);
                    }
                }
                Err(_) => {
                    if failure.is_none() {
                        failure = Some(Error::Unavailable(
                            "resource inspection worker panicked".into(),
                        ));
                    }
                }
            }
        }
        cancel.check()?;
        if let Some(error) = failure {
            return Err(error);
        }
        apply_resource_walk(walk?, snapshot, limited);
        for worker in completed {
            *limited += worker.limited;
            snapshot.processes.extend(worker.snapshot.processes);
            for warning in worker.snapshot.warnings {
                if !snapshot.warnings.contains(&warning) {
                    snapshot.warnings.push(warning);
                }
            }
        }
        Ok(())
    })
}

fn send_resource_batch(
    sender: &std::sync::mpsc::SyncSender<Vec<PathBuf>>,
    batch: &mut Vec<PathBuf>,
    cancel: &Cancellation,
) -> Result<()> {
    cancel.check()?;
    let paths = std::mem::replace(batch, Vec::with_capacity(RESOURCE_BATCH_SIZE));
    sender.send(paths).map_err(|_| {
        cancel
            .check()
            .err()
            .unwrap_or_else(|| Error::Unavailable("resource worker queue disconnected".into()))
    })
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
        Err(error) if recoverable_resource_error(&error) => {
            recover_resource_group(paths, snapshot, cache, limited, cancel, &error)?;
            return Ok(true);
        }
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

fn recoverable_resource_error(error: &Error) -> bool {
    matches!(error, Error::Io { source, .. } if source.raw_os_error() == Some(ERROR_INVALID_HANDLE as i32))
}

fn recover_resource_group(
    paths: &[PathBuf],
    snapshot: &mut Snapshot,
    cache: &mut HashMap<Identity, Process>,
    limited: &mut usize,
    cancel: &Cancellation,
    original_error: &Error,
) -> Result<()> {
    // Restart Manager rejects some large user lists with error 6. Split
    // a failed group before recovery so no user is assigned to every file.
    if paths.len() > 1 {
        let middle = paths.len() / 2;
        correlate(&paths[..middle], snapshot, cache, limited, cancel)?;
        correlate(&paths[middle..], snapshot, cache, limited, cancel)?;
    } else {
        match recover_native_file_users(&paths[0], snapshot, cancel) {
            Ok(unavailable) => {
                *limited += unavailable;
                let warning = format!(
                    "Restart Manager query failed: {original_error}; native file-user fallback recovered this file (reserved Windows query; users are not proven lock owners)."
                );
                if !snapshot.warnings.contains(&warning) {
                    snapshot.warnings.push(warning);
                }
            }
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(recovery) => {
                *limited += 1;
                let warning = format!(
                    "Restart Manager query failed: {original_error}; native file-user fallback failed: {recovery}"
                );
                if !snapshot.warnings.contains(&warning) {
                    snapshot.warnings.push(warning);
                }
            }
        }
        cancel.record(InspectionCounter::Files, 1);
    }
    Ok(())
}

fn recover_native_file_users(
    path: &Path,
    snapshot: &mut Snapshot,
    cancel: &Cancellation,
) -> Result<usize> {
    // Capture births before the PID-only query. A later PID reuse must never bind
    // an old resource observation to a different lifetime or action target.
    let identities = file_users::capture_identities(cancel)?;
    let users = file_users::query(path, cancel)?;
    let lock = sharing(path);
    let mut unavailable = 0;
    for pid in users {
        cancel.check()?;
        if pid == std::process::id() || pid == 0 {
            continue;
        }
        let Some(identity) = identities.get(&pid) else {
            unavailable += 1;
            continue;
        };
        let mut process = match read_process(pid) {
            Ok(process) if process.identity == *identity => process,
            _ => {
                unavailable += 1;
                continue;
            }
        };
        process.usages.push(Usage {
            path: path.to_owned(),
            relation: Relation::NativeFileUser,
            ..Usage::default()
        });
        if let Some(lock) = &lock {
            process.usages.push(Usage {
                path: path.to_owned(),
                relation: Relation::Locked,
                lock: Some(lock.clone()),
                ..Usage::default()
            });
        }
        if read_identity(pid).is_ok_and(|current| current == *identity) {
            snapshot.processes.push(process);
        } else {
            unavailable += 1;
        }
    }
    Ok(unavailable)
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
        collect_resources_serial(
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
        collect_resources_serial(
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
        collect_resources_serial(
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

#[cfg(test)]
mod resource_worker_tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    fn fixture(files: usize) -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..files {
            std::fs::write(
                directory.path().join(format!("file-{index:05}.bin")),
                [0; 8],
            )
            .unwrap();
        }
        directory
    }

    #[test]
    fn bounded_workers_overlap_batches_and_keep_every_file() {
        let directory = fixture(512);
        let cancel = Cancellation::default();
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let barrier = Barrier::new(2);
        let mut snapshot = Snapshot::default();
        let mut limited = 0;
        collect_resources_parallel(
            &Target::new(directory.path()).unwrap(),
            &mut snapshot,
            &mut limited,
            &cancel,
            2,
            &|paths, _, cancel| {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(count, Ordering::SeqCst);
                barrier.wait();
                cancel.record(InspectionCounter::Files, paths.len() as u64);
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(cancel.progress().files, 512);
        assert_eq!(cancel.progress().resource_workers, 2);
        assert_eq!(limited, 0);
    }

    #[test]
    fn worker_cancellation_and_errors_disconnect_backpressured_dispatch() {
        let directory = fixture(4096);
        for cancelled in [false, true] {
            let cancel = Cancellation::default();
            let result = collect_resources_parallel(
                &Target::new(directory.path()).unwrap(),
                &mut Snapshot::default(),
                &mut 0,
                &cancel,
                2,
                &|_, _, cancel| {
                    if cancelled {
                        cancel.cancel();
                        Err(Error::Cancelled)
                    } else {
                        Err(Error::Io {
                            operation: "fixture worker",
                            source: std::io::Error::from_raw_os_error(5),
                        })
                    }
                },
            );
            match result.unwrap_err() {
                Error::Cancelled if cancelled => {}
                Error::Io { operation, source } if !cancelled => {
                    assert_eq!(operation, "fixture worker");
                    assert_eq!(source.raw_os_error(), Some(5));
                }
                error => panic!("worker failure lost its typed context: {error}"),
            }
        }
    }

    #[test]
    fn worker_panic_is_reported_after_all_threads_are_joined() {
        let directory = fixture(4096);
        let result = collect_resources_parallel(
            &Target::new(directory.path()).unwrap(),
            &mut Snapshot::default(),
            &mut 0,
            &Cancellation::default(),
            2,
            &|_, _, _| panic!("synthetic worker panic"),
        );
        assert!(
            matches!(result, Err(Error::Unavailable(message)) if message == "resource inspection worker panicked")
        );
    }
}

#[cfg(test)]
mod native_recovery_tests {
    use super::*;

    #[test]
    fn recovery_is_limited_to_invalid_handle_errors() {
        assert!(recoverable_resource_error(&io(
            "query resource users",
            std::io::Error::from_raw_os_error(6)
        )));
        assert!(!recoverable_resource_error(&io(
            "query resource users",
            std::io::Error::from_raw_os_error(5)
        )));
        assert!(!recoverable_resource_error(&Error::Cancelled));
        assert!(!recoverable_resource_error(&Error::Unavailable(
            "changing resources".into()
        )));
    }

    #[test]
    fn cancelled_recovery_stops_before_opening_native_resources() {
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            file_users::capture_identities(&cancel),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            file_users::query(Path::new("missing-fixture-file"), &cancel),
            Err(Error::Cancelled)
        ));
        assert_eq!(cancel.progress().native_file_user_queries, 0);
    }
}
