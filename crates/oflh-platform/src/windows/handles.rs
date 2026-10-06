//! Process references, not disk traversal. Blocking native work lives in an owned helper.
use super::*;
use crate::inspection_protocol::{Failure, Message, Observation, Progress};
use std::{
    collections::{BTreeMap, HashSet},
    io::{BufReader, BufWriter, Write},
    sync::{
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};
mod mappings;
mod names;
mod snapshot;

#[derive(Default)]
struct Stats([AtomicU64; 8]);
impl Stats {
    fn add(&self, index: usize, amount: u64) {
        self.0[index].fetch_add(amount, Ordering::Relaxed);
    }
    fn progress(&self) -> Progress {
        let values = self.0.each_ref().map(|value| value.load(Ordering::Relaxed));
        Progress {
            processes: values[0],
            handles: values[1],
            names: values[2],
            regions: values[3],
            mapped_names: values[4],
            snapshots: values[5],
            snapshot_micros: values[6],
            workers: values[7],
        }
    }
}

#[derive(Default)]
struct Failures(BTreeMap<(Failure, Option<i32>), u64>);
impl Failures {
    fn note(&mut self, operation: Failure, error: &Error) {
        let code = match error {
            Error::Io { source, .. } => source.raw_os_error(),
            _ => None,
        };
        *self.0.entry((operation, code)).or_default() += 1;
    }
    fn send(self, sender: &SyncSender<Message>) -> Result<()> {
        for ((operation, code), count) in self.0 {
            send(
                sender,
                Message::Warning {
                    operation,
                    code,
                    count,
                },
            )?;
        }
        Ok(())
    }
}
fn send(sender: &SyncSender<Message>, message: Message) -> Result<()> {
    sender
        .send(message)
        .map_err(|_| Error::Unavailable("inspection helper consumer disconnected".into()))
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum NativeOperation {
    Process,
    Duplicate,
    Metadata,
    Path,
    Alias,
    Mapping,
    Sharing,
}
struct ActivityGuard<'a> {
    current: &'a AtomicU8,
    previous: u8,
}
impl Drop for ActivityGuard<'_> {
    fn drop(&mut self) {
        self.current.store(self.previous, Ordering::Relaxed);
    }
}
struct Context<'a> {
    target: &'a Target,
    devices: &'a names::DevicePaths,
    sharing: &'a names::Sharing,
    stats: &'a Stats,
    activity: &'a AtomicU8,
    sender: &'a SyncSender<Message>,
    cancel: &'a Cancellation,
}
impl Context<'_> {
    fn operation(&self, operation: NativeOperation) -> ActivityGuard<'_> {
        ActivityGuard {
            current: self.activity,
            previous: self.activity.swap(1 << operation as u8, Ordering::Relaxed),
        }
    }
    fn publish(
        &self,
        identity: Identity,
        process: &Handle,
        path: &Path,
        directory: bool,
        mapped: bool,
        deleted: bool,
    ) -> Result<()> {
        self.cancel.check()?;
        let sharing = if directory || deleted {
            None
        } else {
            let _activity = self.operation(NativeOperation::Sharing);
            self.sharing.inspect(path)
        };
        // The owned process handle pins this birth identity. A terminated process
        // is rejected before emission; the caller verifies the birth again.
        // SAFETY: owned process grants SYNCHRONIZE, timeout zero never blocks.
        if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
            return Ok(());
        }
        send(
            self.sender,
            Message::Observation(Observation {
                identity,
                path: path.as_os_str().encode_wide().collect(),
                directory,
                mapped,
                deleted,
                sharing,
            }),
        )
    }
}

fn duplicate(process: &Handle, value: usize) -> Result<Handle> {
    let mut handle = null_mut();
    // SAFETY: pinned source process; the numeric remote handle is not dereferenced.
    // SAME_ACCESS returns a new local handle without modifying/closing the source.
    if unsafe {
        DuplicateHandle(
            process.0,
            value as HANDLE,
            GetCurrentProcess(),
            &mut handle,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io("duplicate file handle", std::io::Error::last_os_error()));
    }
    Handle::new(handle, "own duplicated file handle")
}

fn inspect_handles(
    identity: Identity,
    pinned: &Handle,
    values: &[usize],
    context: &Context<'_>,
    failures: &mut Failures,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    let source = match open(
        identity.pid,
        PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
    ) {
        Ok(source) => source,
        Err(error) => {
            failures.note(Failure::Handle, &error);
            return Ok(());
        }
    };
    if times(&source, identity.pid)?.0 != identity {
        return Err(Error::Changed);
    }
    let mut seen = HashSet::new();
    for &value in values {
        context.cancel.check()?;
        context.stats.add(1, 1);
        let _activity = context.operation(NativeOperation::Duplicate);
        let handle = match duplicate(&source, value) {
            Ok(handle) => handle,
            Err(error) => {
                failures.note(Failure::Handle, &error);
                continue;
            }
        };
        // Handle slots can change after the snapshot. Inspect the actual duplicate,
        // never trust the captured object pointer/type/access fields as evidence.
        // SAFETY: worker owns the duplicated handle; filter pipes before path queries.
        if unsafe { GetFileType(handle.0) } != FILE_TYPE_DISK {
            continue;
        }
        let metadata = {
            let _activity = context.operation(NativeOperation::Metadata);
            names::standard(&handle)
        };
        let info = match metadata {
            Ok(info) => info,
            Err(error) => {
                failures.note(Failure::Path, &error);
                continue;
            }
        };
        let deleted = info.DeletePending || info.NumberOfLinks == 0;
        context.stats.add(2, 1);
        let resolved = {
            let _activity = context.operation(NativeOperation::Path);
            names::final_path(&handle, deleted, context.target)
        };
        let observed = match resolved {
            Ok(path) => path,
            Err(error) => {
                failures.note(
                    if deleted {
                        Failure::DeletedName
                    } else {
                        Failure::Path
                    },
                    &error,
                );
                continue;
            }
        };
        if !seen.insert((observed.clone(), info.Directory, deleted)) {
            continue;
        }
        if context.target.contains(&observed) {
            context.publish(identity, pinned, &observed, info.Directory, false, deleted)?;
        } else if deleted {
            // POSIX unlink can move a live handle's native name into an NTFS
            // tombstone directory. Its old parent cannot be inferred safely.
            failures.note(
                Failure::DeletedName,
                &Error::Unavailable("original deleted-file folder is unavailable".into()),
            );
        } else if !info.Directory && info.NumberOfLinks > 1 && !deleted {
            let aliases = {
                let _activity = context.operation(NativeOperation::Alias);
                names::aliases(&handle, &observed, context.target)
            };
            match aliases {
                Ok(aliases) => {
                    for alias in aliases {
                        context.publish(identity, pinned, &alias, false, false, false)?;
                    }
                }
                Err(error) => failures.note(Failure::Alias, &error),
            }
        }
    }
    Ok(())
}

fn inspect_process(
    pid: u32,
    values: &[usize],
    context: &Context<'_>,
    failures: &mut Failures,
) -> Result<()> {
    let _activity = context.operation(NativeOperation::Process);
    context.stats.add(0, 1);
    let pinned = match open(pid, PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE) {
        Ok(process) => process,
        Err(error) => {
            failures.note(Failure::Process, &error);
            return Ok(());
        }
    };
    let identity = times(&pinned, pid)?.0;
    if let Err(error) = inspect_handles(identity, &pinned, values, context, failures) {
        failures.note(Failure::Handle, &error);
    }
    if let Err(error) = mappings::inspect(identity, &pinned, context, failures) {
        failures.note(Failure::Mapping, &error);
    }
    Ok(())
}

fn groups(owner: Identity, stats: &Stats) -> Result<Vec<(u32, Vec<usize>)>> {
    let executable =
        std::env::current_exe().map_err(|error| io("locate file type probe", error))?;
    let known = names::metadata(&executable)?;
    let started = Instant::now();
    let entries = snapshot::collect()?;
    stats.add(5, 1);
    stats.add(
        6,
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
    );
    let file_type = entries
        .iter()
        .find(|entry| {
            entry.process_id == std::process::id() as usize
                && entry.handle_value == known.0 as usize
        })
        .ok_or_else(|| Error::Unavailable("native snapshot omitted the file type probe".into()))?
        .object_type;
    let mut processes = BTreeMap::<u32, Vec<usize>>::new();
    for entry in entries {
        let pid = u32::try_from(entry.process_id)
            .map_err(|_| Error::Unavailable("invalid native snapshot PID".into()))?;
        if entry.object_type == file_type
            && pid > 0
            && pid != owner.pid
            && pid != std::process::id()
        {
            processes.entry(pid).or_default().push(entry.handle_value);
        }
    }
    // A data mapping can outlive every file handle. Include all process lifetimes,
    // even ones absent from the file-handle snapshot, and inspect their regions.
    let (handle, mut entry) = process_snapshot()?;
    // SAFETY: live process snapshot and initialized output size.
    let mut found = unsafe { Process32FirstW(handle.0, &mut entry) };
    while found != 0 {
        let pid = entry.th32ProcessID;
        if pid > 0 && pid != owner.pid && pid != std::process::id() {
            processes.entry(pid).or_default();
        }
        // SAFETY: same live snapshot and correctly sized output.
        found = unsafe { Process32NextW(handle.0, &mut entry) };
    }
    // SAFETY: capture the enumeration failure immediately.
    let code = unsafe { GetLastError() };
    if code != ERROR_NO_MORE_FILES {
        return Err(io(
            "enumerate handle source processes",
            std::io::Error::from_raw_os_error(code as i32),
        ));
    }
    Ok(processes.into_iter().collect())
}

pub(crate) fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map_or(2, |count| count.get().saturating_mul(2))
        .clamp(2, 8)
}

pub(crate) fn run_stdio() -> Result<()> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    crate::inspection_protocol::write_magic(&mut output)?;
    crate::inspection_protocol::write_message(&mut output, &Message::Hello)?;
    output
        .flush()
        .map_err(|error| io("flush helper greeting", error))?;
    let Message::Request(request) =
        crate::inspection_protocol::read_message(&mut BufReader::new(std::io::stdin().lock()))?
    else {
        return Err(Error::Unavailable(
            "inspection helper request is missing".into(),
        ));
    };
    let result = execute(request, &mut output);
    if let Err(error) = &result {
        let code = match error {
            Error::Io { source, .. } => source.raw_os_error(),
            _ => None,
        };
        crate::inspection_protocol::write_message(
            &mut output,
            &Message::Failed {
                code,
                reason: error.to_string(),
            },
        )?;
        output
            .flush()
            .map_err(|error| io("flush native helper failure", error))?;
    }
    result
}
fn execute(request: crate::inspection_protocol::Request, output: &mut impl Write) -> Result<()> {
    if read_identity(request.owner.pid)? != request.owner {
        return Err(Error::Changed);
    }
    let target = Target::new(path(&request.path))?;
    if !target.directory {
        return Err(Error::Unavailable(
            "handle helper requires a directory target".into(),
        ));
    }
    let stats = Stats::default();
    let groups = groups(request.owner, &stats)?;
    let devices = names::DevicePaths::new();
    let sharing = names::Sharing::default();
    let cancel = Cancellation::default();
    let next = AtomicUsize::new(0);
    let activities: Vec<_> = (0..worker_count()).map(|_| AtomicU8::new(0)).collect();
    std::thread::scope(|scope| -> Result<()> {
        let (sender, receiver) = mpsc::sync_channel(worker_count() * 2);
        let mut threads = Vec::new();
        for (index, activity) in activities.iter().enumerate() {
            let sender = sender.clone();
            let groups = &groups;
            let next = &next;
            let target = &target;
            let devices = &devices;
            let sharing = &sharing;
            let stats = &stats;
            let cancel = &cancel;
            let thread = std::thread::Builder::new()
                .name(format!("oflh-native-{index}"))
                .spawn_scoped(scope, move || {
                    let context = Context {
                        target,
                        devices,
                        sharing,
                        stats,
                        activity,
                        sender: &sender,
                        cancel,
                    };
                    let mut failures = Failures::default();
                    while let Some((pid, values)) = groups.get(next.fetch_add(1, Ordering::Relaxed))
                    {
                        context.cancel.check()?;
                        if let Err(error) = inspect_process(*pid, values, &context, &mut failures) {
                            failures.note(Failure::Process, &error);
                        }
                    }
                    failures.send(&sender)
                });
            match thread {
                Ok(thread) => {
                    stats.add(7, 1);
                    threads.push(thread);
                }
                Err(error) => {
                    cancel.cancel();
                    drop(receiver);
                    return Err(io("start native inspection worker", error));
                }
            }
        }
        drop(sender);
        let mut result = Ok(());
        let mut last_flush = Instant::now();
        loop {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(message) => {
                    if let Err(error) = crate::inspection_protocol::write_message(output, &message)
                    {
                        result = Err(error);
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if last_flush.elapsed() >= Duration::from_millis(100) {
                if let Err(error) = crate::inspection_protocol::write_message(
                    output,
                    &Message::Progress(stats.progress()),
                )
                .and_then(|()| {
                    crate::inspection_protocol::write_message(
                        output,
                        &Message::Activity(
                            activities
                                .iter()
                                .fold(0, |mask, activity| mask | activity.load(Ordering::Relaxed)),
                        ),
                    )
                })
                .and_then(|()| {
                    output
                        .flush()
                        .map_err(|error| io("flush helper observations", error))
                }) {
                    result = Err(error);
                    break;
                }
                last_flush = Instant::now();
            }
        }
        if result.is_err() {
            cancel.cancel();
        }
        drop(receiver);
        // Join every successfully spawned worker. A blocked native query is
        // stopped by the parent killing this owned helper, never TerminateThread.
        for thread in threads {
            match thread.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if result.is_ok() {
                        result = Err(error);
                    }
                }
                Err(_) => {
                    if result.is_ok() {
                        result = Err(Error::Unavailable(
                            "native inspection worker panicked".into(),
                        ));
                    }
                }
            }
        }
        result?;
        crate::inspection_protocol::write_message(output, &Message::Progress(stats.progress()))?;
        crate::inspection_protocol::write_message(output, &Message::Done)?;
        output
            .flush()
            .map_err(|error| io("flush helper completion", error))
    })
}
