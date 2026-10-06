//! Real OS contracts. Helpers exist only in this test executable, never in oflh.
use oflh_core::*;
use oflh_platform::Backend;
#[cfg(not(windows))]
use oflh_platform::native;
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct ChildGuard(Child);
#[cfg(windows)]
fn native() -> Result<Box<dyn Backend>> {
    oflh_platform::native_with_inspection_helper(
        oflh_platform::inspection_helper::InspectionHelperCommand::new(
            std::env::current_exe().unwrap(),
            ["--exact", "native_inspection_helper", "--nocapture"]
                .map(std::ffi::OsString::from)
                .to_vec(),
        ),
    )
}
#[cfg(windows)]
#[test]
fn native_inspection_helper() {
    if std::env::var_os("OFLH_NATIVE_HANDLE_HELPER").is_none() {
        return;
    }
    let result = oflh_platform::inspection_helper::run_stdio();
    // Leave no test-harness trailer in the private binary stream. Native RAII
    // resources and stdout have already been dropped by run_stdio.
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn ready(child: &mut Child) -> u32 {
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(id) = line.strip_prefix("OFLH READY ") {
                let _ = tx.send(id.parse::<u32>().unwrap());
            }
            if line == "LOCK FIXTURE READY" {
                let _ = tx.send(0);
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(15))
        .expect("helper readiness timeout")
}
fn start(path: &Path, mode: &str) -> ChildGuard {
    let mut c = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_helper", "--nocapture"])
            .env("OFLH_FIXTURE", path)
            .env("OFLH_MODE", mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    ready(&mut c.0);
    c
}
fn process(backend: &mut dyn Backend, path: &Path, pid: u32) -> Process {
    let target = Target::new(path).unwrap();
    let result = backend.scan(&target, &Cancellation::default()).unwrap();
    result
        .processes
        .into_iter()
        .find(|p| p.identity.pid == pid)
        .unwrap_or_else(|| panic!("PID {pid} missing for {}", path.display()))
}
#[test]
fn fixture_helper() {
    let Some(path) = std::env::var_os("OFLH_FIXTURE") else {
        return;
    };
    let path = PathBuf::from(path);
    let mode = std::env::var("OFLH_MODE").unwrap();
    #[cfg(windows)]
    if matches!(mode.as_str(), "mapped-closed" | "directory" | "deleted") {
        windows_reference_fixture(&path, &mode);
        return;
    }
    if mode == "parent" {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_helper", "--nocapture"])
            .env("OFLH_MODE", "write")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        ready(&mut child);
        println!("OFLH READY {}", child.id());
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        drop(child.stdin.take());
        let _ = child.wait();
        std::process::exit(0);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::*;
        let share = match mode.as_str() {
            "read" => FILE_SHARE_READ | FILE_SHARE_DELETE,
            "write" => FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            "range" => FILE_SHARE_READ | FILE_SHARE_WRITE,
            _ => FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        };
        options.share_mode(share);
    }
    let file = options.open(&path).unwrap();
    std::env::set_current_dir(path.parent().unwrap()).unwrap();
    #[cfg(unix)]
    let mapping = {
        use std::os::fd::AsRawFd;
        // SAFETY: test file is at least 4096 bytes and fd lives until unmap.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };
        assert_ne!(p, libc::MAP_FAILED);
        if mode != "open" {
            // SAFETY: zero is a valid initial POD flock; fields below specify a valid POSIX lock.
            let mut lock: libc::flock = unsafe { std::mem::zeroed() };
            lock.l_type = if mode == "read" {
                libc::F_RDLCK
            } else {
                libc::F_WRLCK
            } as _;
            lock.l_whence = libc::SEEK_SET as _;
            if mode == "range" {
                lock.l_start = 128;
                lock.l_len = 256;
            }
            assert_eq!(
                // SAFETY: valid file and writable initialized POSIX lock record.
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
                0
            );
        }
        p
    };
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let busy = running.clone();
    let worker = std::thread::spawn(move || {
        let mut n = 1u64;
        while busy.load(std::sync::atomic::Ordering::Relaxed) {
            n = n.wrapping_mul(1664525).wrapping_add(1013904223);
            std::hint::black_box(n);
        }
    });
    println!("OFLH READY {}", std::process::id());
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    let n = std::io::stdin().read_line(&mut line).unwrap();
    #[cfg(unix)]
    {
        // SAFETY: mapping is the live address and length returned above, unmapped exactly once.
        assert_eq!(unsafe { libc::munmap(mapping, 4096) }, 0);
    }
    drop(file);
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    worker.join().unwrap();
    if n != 0 {
        println!("RELEASED");
        std::io::stdout().flush().unwrap();
        let _ = std::io::stdin().read_line(&mut line);
    }
    std::process::exit(0);
}
#[test]
fn native_feature_contract() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file ü with spaces.bin");
    fs::write(&path, vec![7; 4096]).unwrap();
    let mut child = start(&path, "write");
    let mut backend = native().unwrap();
    let p = process(&mut *backend, &path, child.0.id());
    assert!(p.usages.iter().any(|u| u.lock.is_some()), "{:?}", p.usages);
    assert!(p.memory.is_some_and(|m| m > 0));
    assert!(
        p.ancestors
            .iter()
            .any(|a| a.identity.pid == std::process::id() && a.identity.started != 0)
    );
    std::thread::sleep(Duration::from_millis(180));
    let m = backend
        .sample(&[p.identity], &Cancellation::default())
        .unwrap();
    assert!(
        m.iter()
            .any(|(id, m)| *id == p.identity && m.cpu.is_some_and(|n| n > 0.0 && n <= 100.0)),
        "{m:?}"
    );
    let mut stale = p.identity;
    stale.started += 1;
    assert!(matches!(
        backend.terminate(stale, true, &Cancellation::default()),
        Err(Error::Changed)
    ));
    #[cfg(unix)]
    {
        let p = process(&mut *backend, dir.path(), child.0.id());
        assert!(p.usages.iter().any(|u| u.relation == Relation::Cwd));
        assert!(p.usages.iter().any(|u| u.relation == Relation::Mapped));
    }
    backend
        .terminate(p.identity, true, &Cancellation::default())
        .unwrap();
    // Termination requests are asynchronous on Windows. Observe process exit before
    // expecting its restrictive file-sharing handles to have been released.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "helper did not exit after termination"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fs::read(&path).unwrap(), vec![7; 4096]);
}
#[test]
fn native_lock_modes_and_release() {
    for mode in ["open", "read", "write", "range"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mode.bin");
        fs::write(&path, vec![0; 4096]).unwrap();
        let mut child = start(&path, mode);
        let mut backend = native().unwrap();
        let p = process(&mut *backend, &path, child.0.id());
        assert_eq!(
            p.usages.iter().any(|u| u.lock.is_some()),
            mode != "open",
            "mode {mode}: {:?}",
            p.usages
        );
        if mode != "open" {
            let expected = if cfg!(windows) {
                match mode {
                    "read" => "write denied",
                    "write" => "read denied",
                    _ => "delete denied",
                }
            } else {
                match mode {
                    "read" => "read",
                    "write" => "write",
                    _ => "128–383",
                }
            };
            assert!(
                p.usages
                    .iter()
                    .filter_map(|u| u.lock.as_ref())
                    .any(|l| l.to_string().to_lowercase().contains(expected))
            );
        }
        child.0.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
        let target = Target::new(&path).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let r = backend.scan(&target, &Cancellation::default()).unwrap();
            if !r.processes.iter().any(|p| {
                p.identity.pid == child.0.id() && p.usages.iter().any(|u| u.lock.is_some())
            }) {
                break;
            }
            assert!(
                std::time::Instant::now() < until,
                "lock persisted after close"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "release should keep helper alive"
        );
    }
}
#[test]
fn native_cancellation_and_protection() {
    let mut b = native().unwrap();
    let c = Cancellation::default();
    c.cancel();
    assert!(matches!(
        b.scan(&Target::new(".").unwrap(), &c),
        Err(Error::Cancelled)
    ));
    for pid in [0, 1, std::process::id()] {
        assert!(matches!(
            b.terminate(
                Identity {
                    pid,
                    started: 1,
                    started_sub: 0
                },
                true,
                &Cancellation::default()
            ),
            Err(Error::Protected)
        ));
    }
}
#[test]
fn native_graceful() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file");
    fs::write(&path, vec![0; 4096]).unwrap();
    let child = start(&path, "open");
    let mut b = native().unwrap();
    let p = process(&mut *b, &path, child.0.id());
    let result = b.terminate(p.identity, false, &Cancellation::default());
    #[cfg(windows)]
    assert!(result.is_err(), "console helper must not be force killed");
    #[cfg(unix)]
    {
        let mut child = child;
        result.unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while child.0.try_wait().unwrap().is_none() {
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
#[cfg(target_os = "linux")]
#[test]
fn deleted_replacement_and_hardlink() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deleted file");
    let alias = dir.path().join("alias");
    fs::write(&path, vec![0; 4096]).unwrap();
    fs::hard_link(&path, &alias).unwrap();
    let child = start(&path, "open");
    let mut b = native().unwrap();
    process(&mut *b, &alias, child.0.id());
    fs::remove_file(&path).unwrap();
    let p = process(&mut *b, &path, child.0.id());
    assert!(p.usages.iter().any(|u| u.deleted));
    fs::write(&path, vec![1; 4096]).unwrap();
    let result = b
        .scan(&Target::new(&path).unwrap(), &Cancellation::default())
        .unwrap();
    assert!(
        !result
            .processes
            .iter()
            .any(|p| p.identity.pid == child.0.id())
    );
    let p = process(&mut *b, dir.path(), child.0.id());
    assert!(p.usages.iter().any(|u| u.deleted));
}
#[test]
fn native_parent_termination() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parent.bin");
    fs::write(&path, vec![0; 4096]).unwrap();
    let mut parent = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "fixture_helper", "--nocapture"])
            .env("OFLH_FIXTURE", &path)
            .env("OFLH_MODE", "parent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let child_id = ready(&mut parent.0);
    let mut b = native().unwrap();
    let p = process(&mut *b, &path, child_id);
    let id = p
        .ancestors
        .iter()
        .find(|a| a.identity.pid == parent.0.id())
        .expect("parent missing")
        .identity;
    let mut stale = id;
    stale.started += 1;
    assert!(matches!(
        b.terminate(stale, true, &Cancellation::default()),
        Err(Error::Changed)
    ));
    b.terminate(id, true, &Cancellation::default()).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let r = b
            .scan(&Target::new(&path).unwrap(), &Cancellation::default())
            .unwrap();
        if !r
            .processes
            .iter()
            .any(|p| p.identity.pid == child_id && p.usages.iter().any(|u| u.lock.is_some()))
        {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "child did not release on parent pipe close"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn compile_c_fixture(directory: &Path) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lock-fixture.c");
    let exe = directory.join(if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    });
    let target = if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-unknown-linux-gnu"
        } else {
            "x86_64-unknown-linux-gnu"
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        }
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-pc-windows-msvc"
    } else {
        "x86_64-pc-windows-msvc"
    };
    let compiler = cc::Build::new()
        .target(target)
        .host(target)
        .opt_level(2)
        .cargo_metadata(false)
        .get_compiler();
    let mut command = compiler.to_command();
    command.current_dir(directory);
    if compiler.is_like_msvc() {
        command.arg(&source).arg(format!("/Fe:{}", exe.display()));
    } else {
        command.arg(&source).arg("-o").arg(&exe);
    }
    assert!(
        command
            .status()
            .expect("C compiler required for independent interoperability test")
            .success()
    );
    exe
}

#[test]
fn independent_c_fixture() {
    let dir = tempfile::tempdir().unwrap();
    let exe = compile_c_fixture(dir.path());
    for mode in ["open", "read", "write", "range"] {
        let path = dir.path().join("external.dat");
        fs::write(&path, vec![0; 4096]).unwrap();
        let mut c = Command::new(&exe);
        c.arg(mode).arg(&path);
        if mode == "range" {
            c.args(["128", "256"]);
        }
        let mut child = ChildGuard(
            c.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        ready(&mut child.0);
        let mut b = native().unwrap();
        let p = process(&mut *b, &path, child.0.id());
        assert_eq!(
            p.usages.iter().any(|u| u.lock.is_some()),
            mode != "open",
            "external {mode}: {:?}",
            p.usages
        );
    }
}

#[test]
fn directory_batches_keep_distinct_native_file_users_and_progress() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    for index in 0..1100 {
        fs::write(first.join(format!("unused-{index:04}.bin")), [0; 64]).unwrap();
    }
    let first_file = first.join("held ü.bin");
    let second_file = second.join("held with spaces.bin");
    fs::write(&first_file, [0; 4096]).unwrap();
    fs::write(&second_file, [0; 4096]).unwrap();
    let first_child = start(&first_file, "open");
    let second_child = start(&second_file, "open");
    let mut backend = native().unwrap();
    let first_observation = process(&mut *backend, &first_file, first_child.0.id());
    let second_observation = process(&mut *backend, &second_file, second_child.0.id());
    let cancel = Cancellation::default();
    let snapshot = backend
        .scan(&Target::new(directory.path()).unwrap(), &cancel)
        .unwrap();
    for expected in [&first_observation, &second_observation] {
        let observed = snapshot
            .processes
            .iter()
            .find(|process| process.identity == expected.identity)
            .unwrap();
        for usage in expected
            .usages
            .iter()
            .filter(|usage| usage.relation != Relation::Cwd)
        {
            #[cfg(windows)]
            let matches = observed.usages.iter().any(|candidate| {
                let mut expected = usage.clone();
                if matches!(
                    expected.relation,
                    Relation::RestartManager | Relation::NativeFileUser
                ) {
                    expected.relation = Relation::Open;
                }
                *candidate == expected
            });
            #[cfg(not(windows))]
            let matches = observed.usages.contains(usage);
            assert!(
                matches,
                "directory lost a single-file observation: {usage:?}"
            );
        }
    }
    let progress = cancel.progress();
    assert!(progress.processes > 0);
    assert!(progress.resources > 0);
    #[cfg(windows)]
    {
        assert_eq!(progress.files, 2);
        assert_eq!(progress.directories, 0);
        assert_eq!(progress.resource_queries, 0);
        assert_eq!(progress.native_handle_snapshots, 1);
        assert!(progress.native_handle_names > 0 && progress.memory_regions > 0);
        assert_eq!(progress.file_identity_queries, 0);
    }
    #[cfg(unix)]
    {
        // Unix enumerates process references; it does not walk unused disk files.
        assert_eq!(progress.files, 0);
        assert_eq!(progress.directories, 0);
    }
}

#[test]
fn directory_inspection_retains_more_than_150_distinct_native_users() {
    let directory = tempfile::tempdir().unwrap();
    let executable = compile_c_fixture(directory.path());
    let data = directory.path().join("data");
    fs::create_dir(&data).unwrap();
    let path = data.join("shared.bin");
    fs::write(&path, [0; 64]).unwrap();
    let expected_path = Target::new(&path).unwrap().path;
    let mut children = Vec::new();
    for _ in 0..160 {
        let mut child = ChildGuard(
            Command::new(&executable)
                .arg("open")
                .arg(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        ready(&mut child.0);
        children.push(child);
    }
    let snapshot = native()
        .unwrap()
        .scan(&Target::new(&data).unwrap(), &Cancellation::default())
        .unwrap();
    let observed: std::collections::BTreeSet<_> = snapshot
        .processes
        .iter()
        .filter(|process| {
            process
                .usages
                .iter()
                .any(|usage| usage.path == expected_path)
        })
        .map(|process| process.identity.pid)
        .collect();
    let missing = children
        .iter()
        .filter(|child| !observed.contains(&child.0.id()))
        .count();
    let observed_identities = children
        .iter()
        .filter(|child| {
            snapshot
                .processes
                .iter()
                .any(|process| process.identity.pid == child.0.id())
        })
        .count();
    assert_eq!(
        missing,
        0,
        "native directory inspection lost fixture users: matched={}, identities={}, warnings={}, resource_errors={:?}",
        observed.len(),
        observed_identities,
        snapshot.warnings.len(),
        snapshot
            .warnings
            .iter()
            .filter(|warning| warning.starts_with("Restart Manager query failed:"))
            .collect::<Vec<_>>()
    );
    assert!(observed.len() >= 160);
}

#[cfg(windows)]
fn windows_reference_fixture(path: &Path, mode: &str) {
    use std::os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    };
    use windows_sys::Win32::{Foundation::*, Storage::FileSystem::*, System::Memory::*};
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(if mode == "directory" {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        })
        .open(path)
        .unwrap();
    struct View(MEMORY_MAPPED_VIEW_ADDRESS);
    impl Drop for View {
        fn drop(&mut self) {
            // SAFETY: exactly the live view returned by MapViewOfFile, unmapped once.
            unsafe { UnmapViewOfFile(self.0) };
        }
    }
    let view = if mode == "mapped-closed" {
        // SAFETY: owned file with 4096 bytes, read-only unnamed mapping, valid inputs.
        let section = unsafe {
            CreateFileMappingW(
                file.as_raw_handle(),
                std::ptr::null(),
                PAGE_READONLY,
                0,
                0,
                std::ptr::null(),
            )
        };
        assert!(!section.is_null() && section != INVALID_HANDLE_VALUE);
        // SAFETY: a newly returned section handle transfers to exactly one RAII owner.
        let section = unsafe { OwnedHandle::from_raw_handle(section) };
        // SAFETY: live mapping section, read-only bounded view of the fixture file.
        let view = unsafe { MapViewOfFile(section.as_raw_handle(), FILE_MAP_READ, 0, 0, 4096) };
        assert!(!view.Value.is_null());
        drop(section);
        Some(View(view))
    } else {
        None
    };
    if mode == "deleted" {
        let original = Target::new(path).unwrap();
        fs::remove_file(path).unwrap();
        let mut info = FILE_STANDARD_INFO::default();
        // SAFETY: live fixture handle and exact writable SDK record, diagnostic only.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileStandardInfo,
                (&mut info as *mut FILE_STANDARD_INFO).cast(),
                std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
            )
        };
        let mut matches = Vec::new();
        for flags in [FILE_NAME_NORMALIZED, FILE_NAME_OPENED] {
            let mut name = vec![0u16; 32768];
            // SAFETY: owned fixture handle and stated writable UTF-16 extent.
            let length = unsafe {
                GetFinalPathNameByHandleW(
                    file.as_raw_handle(),
                    name.as_mut_ptr(),
                    name.len() as u32,
                    flags | VOLUME_NAME_DOS,
                )
            } as usize;
            use std::os::windows::ffi::OsStringExt;
            let observed = PathBuf::from(std::ffi::OsString::from_wide(
                &name[..length.min(name.len())],
            ));
            matches.push(length > 0 && length < name.len() && original.matches(&observed, None));
        }
        // Aggregate diagnostics reveal no native paths or process identities.
        eprintln!(
            "deleted fixture: metadata_ok={ok}, pending={}, links={}, normalized_matches={}, opened_matches={}",
            info.DeletePending, info.NumberOfLinks, matches[0], matches[1]
        );
    }
    let held_file = if mode == "mapped-closed" {
        drop(file);
        None
    } else {
        Some(file)
    };
    println!("OFLH READY {}", std::process::id());
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    drop(view);
    drop(held_file);
}

#[cfg(windows)]
#[test]
fn handle_scan_finds_late_files_without_traversal_or_ten_thousand_file_cap() {
    let directory = tempfile::tempdir().unwrap();
    for index in 0..10_050 {
        fs::write(
            directory.path().join(format!("file-{index:05}.bin")),
            [0; 4096],
        )
        .unwrap();
    }
    let held = directory.path().join("file-10049.bin");
    let child = start(&held, "open");
    let cancel = Cancellation::default();
    let snapshot = native()
        .unwrap()
        .scan(&Target::new(directory.path()).unwrap(), &cancel)
        .unwrap();
    let expected = Target::new(&held).unwrap().path;
    assert!(snapshot.processes.iter().any(|process| {
        process.identity.pid == child.0.id()
            && process
                .usages
                .iter()
                .any(|usage| usage.path == expected && usage.relation == Relation::Open)
    }));
    let progress = cancel.progress();
    assert_eq!(progress.files, 1);
    assert_eq!(progress.directories, 0);
    assert_eq!(progress.resource_queries, 0);
    assert_eq!(progress.native_handle_snapshots, 1);
    assert!(
        !snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("10,000") || warning.contains("helper"))
    );
}

#[cfg(windows)]
#[test]
fn directory_handle_and_closed_file_mapping_remain_visible() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("mapping ü.bin");
    fs::write(&path, [0; 4096]).unwrap();
    let mapped = start(&path, "mapped-closed");
    let held_directory = start(directory.path(), "directory");
    let snapshot = native()
        .unwrap()
        .scan(
            &Target::new(directory.path()).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
    let expected = Target::new(&path).unwrap().path;
    let process = snapshot
        .processes
        .iter()
        .find(|process| process.identity.pid == mapped.0.id())
        .unwrap();
    assert!(process.usages.iter().any(|usage| usage.path == expected
        && usage.relation == Relation::Mapped
        && usage.access == Access::Mapped));
    assert!(
        !process
            .usages
            .iter()
            .any(|usage| usage.path == expected && usage.relation == Relation::Open)
    );
    let expected = Target::new(directory.path()).unwrap().path;
    assert!(
        snapshot
            .processes
            .iter()
            .any(|process| process.identity.pid == held_directory.0.id()
                && process.usages.iter().any(|usage| usage.path == expected
                    && usage.relation == Relation::Open
                    && usage.access == Access::Directory))
    );
}

#[cfg(windows)]
#[test]
fn directory_inspection_retains_an_outside_opened_hard_link_and_deleted_reference() {
    let fixture = tempfile::tempdir().unwrap();
    let directory = fixture.path().join("inspected");
    fs::create_dir(&directory).unwrap();
    let outside = fixture.path().join("outside.bin");
    let alias = directory.join("alias.bin");
    let deleted = directory.join("deleted.bin");
    fs::write(&outside, [0; 4096]).unwrap();
    fs::hard_link(&outside, &alias).unwrap();
    fs::write(&deleted, [0; 4096]).unwrap();
    let alias_child = start(&outside, "open");
    let deleted_child = start(&deleted, "deleted");
    let target = Target::new(&directory).unwrap();
    let expected_alias = Target::new(&alias).unwrap().path;
    let expected_deleted = target.path.join("deleted.bin");
    let snapshot = native()
        .unwrap()
        .scan(&target, &Cancellation::default())
        .unwrap();
    assert!(snapshot.processes.iter().any(|process| {
        process.identity.pid == alias_child.0.id()
            && process
                .usages
                .iter()
                .any(|usage| usage.path == expected_alias && usage.relation == Relation::Open)
    }));
    assert!(
        snapshot.processes.iter().any(|process| {
            process.identity.pid == deleted_child.0.id()
                && process.usages.iter().any(|usage| {
                    usage.path == expected_deleted
                        && usage.relation == Relation::Open
                        && usage.deleted
                })
        }),
        "deleted reference missing; warning_count={}",
        snapshot.warnings.len()
    );
}
