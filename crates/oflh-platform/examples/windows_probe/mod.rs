use oflh_core::{Cancellation, Error, Target, io};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    mem::{offset_of, size_of},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use windows_sys::{
    Wdk::{
        Storage::FileSystem::{FileProcessIdsUsingFileInformation, NtQueryInformationFile},
        System::SystemServices::FILE_PROCESS_IDS_USING_FILE_INFORMATION,
    },
    Win32::{Foundation::*, Storage::FileSystem::*, System::IO::IO_STATUS_BLOCK},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
mod handles;

struct FileHandle(HANDLE);
impl Drop for FileHandle {
    fn drop(&mut self) {
        // SAFETY: this guard owns the valid handle acquired by CreateFileW.
        unsafe { CloseHandle(self.0) };
    }
}

fn open_metadata(path: &Path) -> oflh_core::Result<FileHandle> {
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    if name.contains(&0) {
        return Err(Error::Unavailable("native path contains NUL".into()));
    }
    name.push(0);
    // SAFETY: terminated path lives through the call; no file contents are read.
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
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(io(
            "open native probe metadata",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(FileHandle(handle))
}

/// Class 47 is reserved by Microsoft: this is deliberately an opt-in experiment.
/// The SDK's PID array is pointer-aligned; use a pointer-sized backing allocation.
fn file_users(path: &Path) -> oflh_core::Result<Vec<u32>> {
    let handle = open_metadata(path)?;
    let mut buffer = vec![0usize; 32];
    loop {
        let bytes = buffer.len() * size_of::<usize>();
        let mut status_block = IO_STATUS_BLOCK::default();
        // SAFETY: live owned handle, initialized output, pointer-aligned writable
        // allocation of exactly bytes; the requested layout is provided by the SDK.
        let status = unsafe {
            NtQueryInformationFile(
                handle.0,
                &mut status_block,
                buffer.as_mut_ptr().cast(),
                bytes as u32,
                FileProcessIdsUsingFileInformation,
            )
        };
        if matches!(
            status,
            STATUS_BUFFER_OVERFLOW | STATUS_BUFFER_TOO_SMALL | STATUS_INFO_LENGTH_MISMATCH
        ) {
            if bytes >= 16 * 1024 * 1024 {
                return Err(Error::Unavailable(
                    "native file user list exceeds probe budget".into(),
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
        return decode_file_users(&buffer, status_block.Information);
    }
}

fn decode_file_users(buffer: &[usize], returned: usize) -> oflh_core::Result<Vec<u32>> {
    let offset = offset_of!(FILE_PROCESS_IDS_USING_FILE_INFORMATION, ProcessIdList);
    if returned < offset || returned > std::mem::size_of_val(buffer) {
        return Err(Error::Unavailable(
            "invalid native file user result size".into(),
        ));
    }
    // SAFETY: returned covers the header; buffer has SDK-required alignment.
    let count = unsafe { buffer.as_ptr().cast::<u32>().read() } as usize;
    if count > (returned - offset) / size_of::<usize>() {
        return Err(Error::Unavailable("invalid native file user count".into()));
    }
    buffer[offset / size_of::<usize>()..offset / size_of::<usize>() + count]
        .iter()
        .map(|pid| {
            u32::try_from(*pid).map_err(|_| Error::Unavailable("native PID exceeds u32".into()))
        })
        .collect()
}

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn file_path(root: &Path, index: usize) -> PathBuf {
    root.join(format!("file-{index:05}.bin"))
}
fn hold_files(root: &Path, count: usize) -> Result<()> {
    let mut files = Vec::new();
    for index in 0..count {
        files.push(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(file_path(root, index))?,
        );
    }
    println!("READY");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    std::hint::black_box(files);
    Ok(())
}
fn start_fixture(root: &Path, count: usize) -> Result<Fixture> {
    let mut child = Fixture(
        Command::new(std::env::current_exe()?)
            .arg("--hold")
            .arg(root)
            .arg(count.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let output = child.0.stdout.take().ok_or("missing fixture output")?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let ready = BufReader::new(output).read_line(&mut line).is_ok() && line.trim() == "READY";
        let _ = sender.send(ready);
    });
    if !receiver.recv_timeout(Duration::from_secs(15))? {
        return Err("native probe fixture failed to become ready".into());
    }
    Ok(child)
}
fn fixture_coverage(root: &Path, fixtures: &[Fixture]) -> Result<Value> {
    let target = Target::new(root)?;
    let cancel = Cancellation::default();
    let snapshot = oflh_platform::native()?.scan(&target, &cancel)?;
    let matching: BTreeSet<_> = snapshot
        .processes
        .iter()
        .filter(|process| {
            process
                .usages
                .iter()
                .any(|usage| target.contains(&usage.path))
        })
        .map(|process| process.identity.pid)
        .collect();
    Ok(
        json!({"found_fixture_users":fixtures.iter().filter(|child| matching.contains(&child.0.id())).count(),
        "expected_fixture_users":fixtures.len(),"warnings":snapshot.warnings.len()}),
    )
}
fn profile_discovery() -> Result<Value> {
    let root = tempfile::tempdir()?;
    for index in 0..2048 {
        fs::write(file_path(root.path(), index), [0u8; 64])?;
    }
    let child = start_fixture(root.path(), 128)?;
    let mut samples = Vec::new();
    let mut observed = 0;
    for sample in 0..6 {
        let started = Instant::now();
        observed = 0;
        for index in 0..2048 {
            if file_users(&file_path(root.path(), index))?.contains(&child.0.id()) {
                observed += 1;
            }
        }
        if observed != 128 {
            return Err("native discovery missed held fixture files".into());
        }
        if sample > 0 {
            samples.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    samples.sort_by(f64::total_cmp);
    Ok(
        json!({"files":2048,"held_files":128,"found_held_files":observed,"samples":samples.len(),
        "discovery_median_ms":samples[samples.len()/2],
        "handle_discovery":handles::isolated(root.path(), &[child.0.id()])?,
        "scope":"file user discovery only; excludes metadata, modules, locks and process identity validation"}),
    )
}
fn profile_many_users() -> Result<Value> {
    let root = tempfile::tempdir()?;
    fs::write(file_path(root.path(), 0), [0u8; 64])?;
    let mut children = Vec::new();
    for _ in 0..160 {
        children.push(start_fixture(root.path(), 1)?);
    }
    let started = Instant::now();
    let users = file_users(&file_path(root.path(), 0))?;
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    let found = children
        .iter()
        .filter(|child| users.contains(&child.0.id()))
        .count();
    let backend = fixture_coverage(root.path(), &children)?;
    Ok(
        json!({"expected_fixture_users":160,"native_query_found":found,
        "native_query_ms":elapsed,"backend":backend,
        "handle_discovery":handles::isolated(root.path(), &children.iter().map(|child| child.0.id()).collect::<Vec<_>>())?}),
    )
}

pub(super) fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--handles-worker") {
        let root = Path::new(args.get(1).ok_or("missing handle probe root")?);
        let pids = args
            .get(2)
            .ok_or("missing fixture identities")?
            .to_str()
            .ok_or("invalid fixture identities")?
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<Vec<u32>, _>>()?;
        println!("{}", handles::inspect(root, &pids)?);
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--hold") {
        return hold_files(
            Path::new(args.get(1).ok_or("missing fixture root")?),
            args.get(2)
                .ok_or("missing fixture count")?
                .to_str()
                .ok_or("invalid count")?
                .parse()?,
        );
    }
    let dense = profile_discovery().unwrap_or_else(|error| json!({"error":error.to_string()}));
    let many = profile_many_users().unwrap_or_else(|error| json!({"error":error.to_string()}));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"os":"windows","arch":std::env::consts::ARCH,
        "experimental":true,"shipping_backend":false,"direct_file_users":dense,"many_users":many}))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_layout_and_variable_user_list_are_checked() {
        assert_eq!(size_of::<usize>(), 8, "probe targets native x64 and ARM64");
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
            decode_file_users(&buffer, size_of_val(buffer.as_slice()))
                .unwrap()
                .len(),
            160
        );
        assert!(decode_file_users(&buffer, 8).is_err());
        assert!(decode_file_users(&buffer, size_of_val(buffer.as_slice()) + 1).is_err());
        assert!(decode_file_users(&buffer, 0).is_err());
        buffer[0] = usize::MAX;
        assert!(decode_file_users(&buffer, size_of_val(buffer.as_slice())).is_err());
        assert_eq!(decode_file_users(&[0], 8).unwrap(), Vec::<u32>::new());
        assert!(decode_file_users(&[1, usize::MAX], 16).is_err());
    }
}
