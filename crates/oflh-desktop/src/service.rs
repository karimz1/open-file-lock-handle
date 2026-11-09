//! Bounded scan scheduling, immutable publication and captured destructive actions.
use crate::{
    contract::*,
    dataset::{Dataset, display},
    recent::RecentTargets,
};
use oflh_core::{Ancestor, Cancellation, Error, Identity, Process, Snapshot, Target};
use oflh_platform::Backend;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard},
};

struct ScanJob {
    generation: u32,
    path: Option<PathBuf>,
    owner: Option<Identity>,
    cancel: Cancellation,
}
#[derive(Clone)]
struct PendingAction {
    ticket: String,
    identities: Vec<Identity>,
    force: bool,
    elevated: bool,
    targets: Vec<ActionTarget>,
}
/// An explicit one-shot action gated by a fresh free scan.
#[derive(Clone, Debug)]
pub enum WaitFollowUp {
    /// No resource or window change.
    None,
    /// Reveal the still-retained native path in the system file manager.
    Reveal,
    /// Delete the file or directory after the final scan.
    Delete,
    /// Move into this directory after checking the native target is free.
    Move(std::path::PathBuf),
    /// Rename within the original folder using one validated filename component.
    Rename(String),
}
#[derive(Clone)]
struct WaitJob {
    id: u64,
    process: Option<Identity>,
    description: WaitTargetView,
    path: Option<PathBuf>,
    port: Option<u16>,
    timeout: Option<std::time::Duration>,
    cancel: Cancellation,
    started: std::time::Instant,
}
impl WaitJob {
    /// A port wait on a path counts only processes that also use that path.
    fn check(&self) -> oflh_core::check::CheckTarget {
        oflh_core::check::CheckTarget::select(self.path.is_some(), false, self.port)
    }
}
struct State {
    generation: u32,
    dataset: Arc<Dataset>,
    pending: Option<ScanJob>,
    cancellation: Cancellation,
    scanning: bool,
    started: Option<std::time::Instant>,
    elapsed_ms: u64,
    last_scan_elapsed_ms: Option<u64>,
    shutdown: bool,
    target: Option<PathBuf>,
    recent: Vec<(u32, PathBuf)>,
    error: Option<Failure>,
    action: Option<PendingAction>,
    admin_recovery: Option<PendingAction>,
    next_ticket: u64,
    ancestry: Option<(String, Vec<Ancestor>)>,
    wait: Option<WaitJob>,
    wait_started: Option<std::time::Instant>,
    wait_outcome: Option<Result<(), Failure>>,
    wait_completed: Option<WaitJob>,
    next_wait: u64,
    wait_destination: Option<(String, PathBuf)>,
    next_wait_destination: u64,
    wait_follow_up_claimed: bool,
}
struct Shared {
    state: Mutex<State>,
    recent_targets: Mutex<RecentTargets>,
    ready: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn lock_recent_targets(&self) -> MutexGuard<'_, RecentTargets> {
        self.recent_targets
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}
fn cancel_wait_locked(state: &mut State) {
    if let Some(wait) = &state.wait {
        wait.cancel.cancel();
    }
    if let Some(wait) = &state.wait_completed {
        wait.cancel.cancel();
    }
    state.wait = None;
    state.wait_completed = None;
    state.wait_started = None;
    state.wait_outcome = None;
}
fn storage_failure(error: impl std::fmt::Display) -> Failure {
    Failure {
        kind: "desktop_storage".into(),
        message: oflh_core::safe(&format!("Could not update recent targets: {error}")),
        os_code: None,
        details: None,
    }
}
/// Long-lived desktop service. One worker and one pending scan bound native work.
pub struct Service {
    shared: Arc<Shared>,
    sessions: Arc<crate::sessions::Sessions>,
}
impl Service {
    /// Start a worker-owned native scanner and a completion callback.
    pub fn new(
        backend: Box<dyn Backend>,
        notify: impl Fn(Status) + Send + 'static,
    ) -> std::io::Result<Self> {
        let recent = RecentTargets::in_memory().map_err(std::io::Error::other)?;
        Self::start(backend, notify, recent)
    }

    /// Start the scanner and restore recent targets from a SQLite database.
    pub fn with_recent_database(
        backend: Box<dyn Backend>,
        notify: impl Fn(Status) + Send + 'static,
        database: impl AsRef<Path>,
    ) -> std::io::Result<Self> {
        let recent = RecentTargets::open(database).map_err(std::io::Error::other)?;
        Self::start(backend, notify, recent)
    }

    fn start(
        mut backend: Box<dyn Backend>,
        notify: impl Fn(Status) + Send + 'static,
        recent_targets: RecentTargets,
    ) -> std::io::Result<Self> {
        let recent_paths = recent_targets.load().map_err(std::io::Error::other)?;
        let generation = recent_paths.len() as u32;
        let recent = recent_paths
            .into_iter()
            .enumerate()
            .map(|(index, path)| (generation - index as u32, path))
            .collect();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation,
                dataset: Arc::new(Dataset::new(0, Snapshot::default())),
                pending: None,
                cancellation: Cancellation::default(),
                scanning: false,
                started: None,
                elapsed_ms: 0,
                last_scan_elapsed_ms: None,
                shutdown: false,
                target: None,
                recent,
                error: None,
                action: None,
                admin_recovery: None,
                next_ticket: 0,
                ancestry: None,
                wait: None,
                wait_started: None,
                wait_outcome: None,
                wait_completed: None,
                next_wait: 0,
                wait_destination: None,
                next_wait_destination: 0,
                wait_follow_up_claimed: false,
            }),
            recent_targets: Mutex::new(recent_targets),
            ready: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("oflh-desktop-scan".into())
            .spawn(move || {
                loop {
                    let job = {
                        let mut state = worker.lock();
                        while state.pending.is_none() && state.wait.is_none() && !state.shutdown {
                            let result = worker
                                .ready
                                .wait_timeout(state, std::time::Duration::from_millis(50));
                            state = result.unwrap_or_else(|error| error.into_inner()).0;
                        }
                        if state.shutdown {
                            return;
                        }
                        state.pending.take()
                    };
                    // Drop the temporary guard before native scanning; an `if let`
                    // scrutinee temporary otherwise lives for the entire block.
                    let wait = worker.lock().wait.clone();
                    if let Some(wait) = wait {
                        let options = oflh_core::wait::Options {
                            timeout: wait.timeout,
                            ..Default::default()
                        };
                        let path = wait
                            .path
                            .as_deref()
                            .map(Target::new)
                            .transpose()
                            .map_err(Failure::from);
                        let result = if let Some(identity) = wait.process {
                            crate::process_wait::wait_for_exit(
                                &mut *backend,
                                identity,
                                options,
                                wait.started,
                                &wait.cancel,
                                &oflh_core::wait::PollSleeper,
                            )
                            .map_err(Failure::from)
                        } else {
                            path.and_then(|path| {
                                oflh_platform::wait_until_free(
                                    &mut *backend,
                                    oflh_platform::WaitTarget {
                                        path: path.as_ref(),
                                        check: wait.check(),
                                    },
                                    options,
                                    wait.started,
                                    &wait.cancel,
                                )
                                .map_err(Failure::from)
                            })
                        };
                        let status = {
                            let mut state = worker.lock();
                            // Cancellation or a replacement wait invalidates this completion.
                            if state.wait.as_ref().is_none_or(|current| {
                                current.id != wait.id || current.cancel.check().is_err()
                            }) {
                                continue;
                            }
                            state.wait_outcome = Some(result);
                            state.wait_completed = state.wait.clone();
                            state.wait = None;
                            status(&state)
                        };
                        notify(status);
                        continue;
                    }
                    let Some(job) = job else {
                        continue;
                    };
                    let result = scan(&mut *backend, &job);
                    let status = {
                        let mut state = worker.lock();
                        if state.generation != job.generation || state.shutdown {
                            continue;
                        }
                        let succeeded = result.is_ok();
                        match result {
                            Ok((path, dataset)) => {
                                state.error = None;
                                if let Some(path) = path {
                                    state.recent.retain(|(_, previous)| *previous != path);
                                    state.recent.insert(0, (job.generation, path.clone()));
                                    state.recent.truncate(12);
                                    state.target = Some(path.clone());
                                    let storage_result = {
                                        let mut recent_targets = worker.lock_recent_targets();
                                        recent_targets.record(&path)
                                    };
                                    if let Err(error) = storage_result {
                                        state.error = Some(storage_failure(error));
                                    }
                                }
                                state.dataset = Arc::new(dataset);
                            }
                            Err(error) => state.error = Some(error),
                        }
                        state.elapsed_ms = elapsed_ms(&state);
                        state.scanning = false;
                        if succeeded {
                            state.last_scan_elapsed_ms = Some(state.elapsed_ms);
                        }
                        status(&state)
                    };
                    notify(status);
                }
            })?;
        Ok(Self {
            shared,
            sessions: Arc::new(crate::sessions::Sessions::native()),
        })
    }
    /// Replace the session source (tests, fixtures).
    pub fn with_sessions(mut self, sessions: crate::sessions::Sessions) -> Self {
        self.sessions = Arc::new(sessions);
        self
    }
    /// One table page with session columns filled for its rows only.
    pub fn page(&self, revision: u32, query: &TableQuery) -> Result<Page, Failure> {
        let mut page = self.dataset(revision)?.page(query)?;
        self.sessions.annotate(&mut page.rows);
        Ok(page)
    }
    fn owner(&self, process: &Process) -> OwnerView {
        self.sessions.owner(&process.user, process.identity.pid)
    }
    fn target(&self) -> Option<(PathBuf, bool)> {
        let path = self.shared.lock().target.clone()?;
        let directory = path.is_dir();
        Some((path, directory))
    }
    /// Capture the session of a scanned process for "Sign out user".
    pub fn prepare_sign_out(
        &self,
        revision: u32,
        key: &str,
    ) -> Result<SignOutConfirmation, Failure> {
        let dataset = self.dataset(revision)?;
        let (_, process) = dataset.process(key)?;
        self.dismiss();
        self.sessions.prepare_sign_out(
            process.identity.pid,
            dataset
                .snapshot
                .processes
                .iter()
                .map(|process| (process.identity.pid, process.name.clone())),
        )
    }
    /// Sign out the captured session after revalidating it.
    pub fn sign_out(&self, ticket: &str) -> Result<(), Failure> {
        self.sessions.sign_out(ticket)
    }
    /// Send a message to the session of a scanned process.
    pub fn send_message(&self, revision: u32, key: &str, text: &str) -> Result<(), Failure> {
        let (_, process) = self
            .dataset(revision)?
            .process(key)
            .map(|(i, p)| (i, p.identity.pid))?;
        self.sessions.send_message(process, text)
    }
    /// Files of the current target opened over SMB shares.
    pub fn share_files(&self) -> ShareFiles {
        let target = self.target();
        self.sessions.share_files(
            target
                .as_ref()
                .map(|(path, directory)| (path.as_path(), *directory)),
        )
    }
    /// Capture one share open of the current target for "Close open file".
    pub fn prepare_close_file(&self, id: u32) -> Result<CloseFileConfirmation, Failure> {
        let (path, directory) = self
            .target()
            .ok_or_else(|| Failure::invalid("Inspect a file or folder first"))?;
        self.dismiss();
        self.sessions.prepare_close_file(id, (&path, directory))
    }
    /// Close the captured share open after revalidating it.
    pub fn close_file(&self, ticket: &str) -> Result<(), Failure> {
        self.sessions.close_file(ticket)
    }
    /// Start an inspection. Requests during queued or active work preserve that request.
    pub fn inspect(&self, path: PathBuf) -> Result<Status, Failure> {
        if path.as_os_str().is_empty() {
            return Err(Failure::invalid("Choose a file or folder first"));
        }
        self.enqueue(Some(path), None)
    }
    /// Discover global local bindings without requiring a filesystem target.
    pub fn ports(&self) -> Result<Status, Failure> {
        let path = self.shared.lock().target.clone();
        self.enqueue(path, None)
    }
    /// Follow a port owner's captured folder, committing only a matching birth identity.
    pub fn follow_process(&self, revision: u32, key: &str) -> Result<Status, Failure> {
        let dataset = self.dataset(revision)?;
        let (_, process) = dataset.process(key)?;
        let path = process
            .inspection_folder()
            .ok_or_else(|| Failure::invalid("No folder is available for this owner"))?;
        self.enqueue(Some(path.to_owned()), Some(process.identity))
    }
    /// Start an inspection requested by another launch of the app, such as the
    /// Windows Explorer context menu while this window is open. Unlike [`Self::inspect`],
    /// a running or queued scan is cancelled and replaced: the user explicitly chose a
    /// new target outside this window. Both steps share one lock, so no other request
    /// can slip in between.
    pub fn take_over(&self, path: PathBuf) -> Result<Status, Failure> {
        if path.as_os_str().is_empty() {
            return Err(Failure::invalid("Choose a file or folder first"));
        }
        let mut state = self.shared.lock();
        if state.scanning {
            cancel_scan(&mut state);
        }
        if let Some(wait) = &state.wait {
            wait.cancel.cancel();
        }
        state.wait = None;
        self.enqueue_locked(&mut state, Some(path), None)
    }
    fn enqueue(&self, path: Option<PathBuf>, owner: Option<Identity>) -> Result<Status, Failure> {
        let mut state = self.shared.lock();
        self.enqueue_locked(&mut state, path, owner)
    }
    fn enqueue_locked(
        &self,
        state: &mut State,
        path: Option<PathBuf>,
        owner: Option<Identity>,
    ) -> Result<Status, Failure> {
        // Coalesce all entry points atomically: timer, keyboard, dialogs and drops
        // cannot cancel an inspection merely because its acknowledgment is delayed.
        if state.scanning || state.wait.is_some() {
            cancel_wait_locked(state);
            return Ok(status(state));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to start another scan"))?;
        state.cancellation.cancel();
        state.cancellation = Cancellation::default();
        state.pending = Some(ScanJob {
            generation: state.generation,
            path,
            owner,
            cancel: state.cancellation.clone(),
        });
        state.started = Some(std::time::Instant::now());
        state.elapsed_ms = 0;
        state.scanning = true;
        state.error = None;
        self.shared.ready.notify_one();
        Ok(status(state))
    }
    /// Wait for the current target. Only one scan is in flight and cancellation
    /// stops both the sleep and native scan; a deadline is never reported as free.
    pub fn wait_until_free(&self, request: WaitRequest) -> Status {
        let mut state = self.shared.lock();
        if state.scanning || state.pending.is_some() || state.wait.is_some() {
            return status(&state);
        }
        let description = WaitTargetView {
            kind: if request.path.is_some() {
                "file"
            } else {
                "port"
            },
            display: request.path.as_deref().map(display).unwrap_or_else(|| {
                request
                    .port
                    .map_or_else(String::new, |port| format!("Port {port}"))
            }),
        };
        self.begin_wait_locked(
            &mut state,
            WaitJob {
                id: 0,
                process: None,
                description,
                path: request.path,
                port: request.port,
                timeout: request.timeout,
                cancel: Cancellation::default(),
                started: std::time::Instant::now(),
            },
        )
    }
    /// Resolve a row selection in its owning snapshot before scheduling background work.
    pub fn wait_for_selection(&self, request: SelectionWaitRequest) -> Result<Status, Failure> {
        let mut state = self.shared.lock();
        if state.scanning || state.pending.is_some() || state.wait.is_some() {
            return Err(Failure::invalid(
                "Finish or cancel the current inspection or wait first",
            ));
        }
        if request.revision != state.dataset.revision {
            return Err(Failure::from(Error::Changed));
        }
        if request
            .timeout_seconds
            .is_some_and(|seconds| seconds == 0 || seconds > 86400)
        {
            return Err(Failure::invalid(
                "Wait timeout must be between 1 and 86400 seconds",
            ));
        }
        let (path, process, port, description) = match request.target {
            SelectionWaitTarget::File { reference } => {
                let path = state.dataset.path(&reference)?;
                if request.delete_after {
                    crate::wait_files::refuse_protected_delete(&path)?;
                }
                let description = WaitTargetView {
                    kind: "file",
                    display: display(&path),
                };
                (Some(path), None, None, description)
            }
            SelectionWaitTarget::Process { key } => {
                let (_, process) = state.dataset.process(&key)?;
                process.identity.validate().map_err(Failure::from)?;
                (
                    None,
                    Some(process.identity),
                    None,
                    WaitTargetView {
                        kind: "process",
                        display: format!(
                            "{} (PID {})",
                            oflh_core::safe(&process.name),
                            process.identity.pid
                        ),
                    },
                )
            }
            SelectionWaitTarget::Port { reference } => {
                let port = state.dataset.port(&reference)?.number;
                (
                    None,
                    None,
                    Some(port),
                    WaitTargetView {
                        kind: "port",
                        display: format!("Port {port}"),
                    },
                )
            }
        };
        Ok(self.begin_wait_locked(
            &mut state,
            WaitJob {
                id: 0,
                path,
                process,
                port,
                description,
                timeout: request.timeout_seconds.map(std::time::Duration::from_secs),
                cancel: Cancellation::default(),
                started: std::time::Instant::now(),
            },
        ))
    }
    fn begin_wait_locked(&self, state: &mut State, mut job: WaitJob) -> Status {
        if let Some(wait) = &state.wait_completed {
            wait.cancel.cancel();
        }
        state.wait_completed = None;
        state.next_wait = state.next_wait.wrapping_add(1);
        state.wait_follow_up_claimed = false;
        job.id = state.next_wait;
        state.wait_started = Some(std::time::Instant::now());
        state.wait_outcome = None;
        state.wait = Some(job);
        self.shared.ready.notify_one();
        status(state)
    }
    /// Cancel waiting without changing accepted inspection results.
    pub fn cancel_wait(&self) -> Status {
        let mut state = self.shared.lock();
        cancel_wait_locked(&mut state);
        state.wait_outcome = Some(Err(Failure::from(Error::Cancelled)));
        self.shared.ready.notify_one();
        status(&state)
    }
    fn wait_follow_up_with_backend(
        &self,
        id: u64,
        follow_up: WaitFollowUp,
        backend: &mut dyn Backend,
    ) -> Result<Option<PathBuf>, Failure> {
        let mut state = self.shared.lock();
        let job = state
            .wait_completed
            .clone()
            .filter(|job| job.id == id && state.wait_outcome.as_ref().is_some_and(Result::is_ok));
        let Some(job) = job else {
            return Err(Failure::invalid("No free target"));
        };
        if job.process.is_some() || job.path.is_none() {
            return Err(Failure::invalid("This wait has no filesystem follow-up"));
        }
        if state.wait_follow_up_claimed {
            return Err(Failure::invalid(
                "The follow-up for this wait has already been requested",
            ));
        }
        state.wait_follow_up_claimed = true;
        drop(state);
        let result = (|| {
            if let WaitFollowUp::Rename(name) = &follow_up {
                crate::wait_files::validate_name(name)?;
            }
            let target = job
                .path
                .as_deref()
                .map(Target::new)
                .transpose()
                .map_err(Failure::from)?;
            oflh_platform::confirm_free_before_action(
                backend,
                target.as_ref(),
                job.check(),
                &job.cancel,
            )
            .map_err(Failure::from)?;
            let target =
                target.ok_or_else(|| Failure::invalid("No filesystem target is waiting"))?;
            crate::wait_files::apply(&target, follow_up, &job.cancel)
        })();
        if let Err(error) = &result {
            let mut state = self.shared.lock();
            if state
                .wait_completed
                .as_ref()
                .is_some_and(|current| current.id == id)
            {
                state.wait_outcome = Some(Err(error.clone()));
            }
        }
        result
    }

    /// Retain a folder selected by the native picker without a display-path round trip.
    pub fn capture_wait_destination(&self, folder: PathBuf) -> Result<PathValue, Failure> {
        let folder = std::fs::canonicalize(folder).map_err(|error| {
            Failure::from(oflh_core::io("resolve selected destination folder", error))
        })?;
        if !std::fs::metadata(&folder)
            .map_err(|error| {
                Failure::from(oflh_core::io("inspect selected destination folder", error))
            })?
            .is_dir()
        {
            return Err(Failure::invalid("Choose a destination folder"));
        }
        let mut state = self.shared.lock();
        state.next_wait_destination = state
            .next_wait_destination
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to choose another folder"))?;
        let reference = format!("wait-folder-{}", state.next_wait_destination);
        let value = PathValue {
            display: display(&folder),
            reference: reference.clone(),
        };
        state.wait_destination = Some((reference, folder));
        Ok(value)
    }
    /// Resolve only the current picker reference, never a path supplied by the WebView.
    pub fn wait_destination(&self, reference: &str) -> Result<PathBuf, Failure> {
        self.shared
            .lock()
            .wait_destination
            .as_ref()
            .filter(|(key, _)| key == reference)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| Failure::invalid("Choose the destination folder again"))
    }

    /// Native path retained for the final guarded reveal action.
    pub fn shared_wait_path(&self, id: u64) -> Result<PathBuf, Failure> {
        let state = self.shared.lock();
        state
            .wait_completed
            .as_ref()
            .filter(|job| job.id == id && state.wait_outcome.as_ref().is_some_and(Result::is_ok))
            .and_then(|wait| wait.path.clone())
            .ok_or_else(|| Failure::invalid("No filesystem target is waiting"))
    }
    /// Describe only the successfully completed wait identified by the caller.
    pub fn completed_wait_target(&self, id: u64) -> Result<WaitTargetView, Failure> {
        let state = self.shared.lock();
        state
            .wait_completed
            .as_ref()
            .filter(|job| job.id == id && state.wait_outcome.as_ref().is_some_and(Result::is_ok))
            .map(|job| job.description.clone())
            .ok_or_else(|| Failure::invalid("This wait is no longer the completed operation"))
    }
    /// Run an explicit follow-up only after another complete free scan.
    pub fn wait_follow_up(
        &self,
        id: u64,
        follow_up: WaitFollowUp,
    ) -> Result<Option<PathBuf>, Failure> {
        let mut backend = oflh_platform::native().map_err(Failure::from)?;
        self.wait_follow_up_with_backend(id, follow_up, &mut *backend)
    }
    /// Rescan the last successful native target without a display-string round trip.
    pub fn refresh(&self) -> Result<Status, Failure> {
        {
            let state = self.shared.lock();
            if state.scanning {
                return Ok(status(&state));
            }
        }
        let path = self.shared.lock().target.clone();
        if path.is_none() && self.status().revision == 0 {
            return Err(Failure::invalid(
                "Choose a target or inspect local ports first",
            ));
        }
        self.enqueue(path, None)
    }
    /// Cancel running and queued scans while preserving accepted results.
    pub fn cancel(&self) -> Status {
        let mut state = self.shared.lock();
        cancel_scan(&mut state);
        status(&state)
    }
    /// Close the inspection, rejecting late publications and captured actions.
    /// Recent targets remain available; no process or file is changed.
    pub fn close_inspection(&self) -> Result<Status, Failure> {
        let mut state = self.shared.lock();
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to close this inspection"))?;
        state.cancellation.cancel();
        state.cancellation = Cancellation::default();
        cancel_wait_locked(&mut state);
        state.pending = None;
        state.scanning = false;
        state.started = None;
        state.elapsed_ms = 0;
        state.last_scan_elapsed_ms = None;
        state.dataset = Arc::new(Dataset::new(0, Snapshot::default()));
        state.target = None;
        state.error = None;
        state.action = None;
        state.admin_recovery = None;
        state.ancestry = None;
        Ok(status(&state))
    }
    /// List at most twelve persisted native targets as opaque IDs and display text.
    pub fn recent(&self) -> Vec<(u32, String)> {
        self.shared
            .lock()
            .recent
            .iter()
            .map(|(id, path)| (*id, display(path)))
            .collect()
    }
    /// Remove one saved target from the persisted recent list.
    pub fn remove_recent(&self, id: u32) -> Result<(), Failure> {
        let mut state = self.shared.lock();
        let path = state
            .recent
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| Failure::invalid("Recent target expired"))?;
        self.shared
            .lock_recent_targets()
            .remove(&path)
            .map_err(storage_failure)?;
        state.recent.retain(|(key, _)| *key != id);
        Ok(())
    }
    /// Clear all saved targets without changing the current inspection.
    pub fn clear_recent(&self) -> Result<(), Failure> {
        let mut state = self.shared.lock();
        self.shared
            .lock_recent_targets()
            .clear()
            .map_err(storage_failure)?;
        state.recent.clear();
        Ok(())
    }
    /// Reinspect a session target using its retained native path.
    pub fn revisit(&self, id: u32) -> Result<Status, Failure> {
        let path = self
            .shared
            .lock()
            .recent
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| Failure::invalid("Recent target expired"))?;
        self.inspect(path)
    }
    /// Read a small, consistent view of scan progress and coverage.
    pub fn status(&self) -> Status {
        status(&self.shared.lock())
    }
    /// Clone an immutable snapshot handle only if its revision is still current.
    pub fn dataset(&self, revision: u32) -> Result<Arc<Dataset>, Failure> {
        let dataset = self.shared.lock().dataset.clone();
        if dataset.revision != revision {
            return Err(Failure::invalid(
                "Results changed; retry using the current snapshot",
            ));
        }
        Ok(dataset)
    }
    /// Keep the focused ancestry's captured lifetimes stable across refreshes.
    pub fn details(&self, revision: u32, key: &str) -> Result<Details, Failure> {
        let dataset = self.dataset(revision)?;
        let mut details = dataset.details(key)?;
        let (_, process) = dataset.process(key)?;
        let mut state = self.shared.lock();
        state
            .ancestry
            .get_or_insert_with(|| (key.into(), process.ancestors.clone()));
        if state
            .ancestry
            .as_ref()
            .is_some_and(|(owner, _)| owner != key)
        {
            state.ancestry = Some((key.into(), process.ancestors.clone()));
        }
        details.owner = self.owner(process);
        details.session_actions = details
            .owner
            .session
            .as_ref()
            .map(|session| self.sessions.actions(session));
        if let Some((_, ancestors)) = &state.ancestry {
            details.ancestors = ancestors
                .iter()
                .map(|ancestor| AncestorView {
                    key: identity_key(ancestor.identity),
                    pid: ancestor.identity.pid,
                    name: oflh_core::safe(&ancestor.name),
                    actionable: ancestor.identity.validate().is_ok(),
                })
                .collect();
        }
        Ok(details)
    }
    /// Confirm only an ancestor captured in the focused tree, never a caller-supplied PID.
    pub fn prepare_ancestor(
        &self,
        owner: &str,
        key: &str,
        force: bool,
        backend: &mut dyn Backend,
    ) -> Result<Confirmation, Failure> {
        let ancestor = self
            .shared
            .lock()
            .ancestry
            .as_ref()
            .filter(|(captured_owner, _)| captured_owner == owner)
            .and_then(|(_, ancestors)| {
                ancestors
                    .iter()
                    .find(|ancestor| identity_key(ancestor.identity) == key)
            })
            .cloned()
            .ok_or_else(|| {
                Failure::invalid("Ancestry selection expired; reopen process details")
            })?;
        ancestor.identity.validate().map_err(Failure::from)?;
        // The ancestor may also appear in the scan; its files and ports count too.
        let dataset = self.shared.lock().dataset.clone();
        let scanned = dataset.process(key).ok().map(|(_, process)| process);
        let [native] = <[_; 1]>::try_from(backend.impact(&[ancestor.identity]))
            .unwrap_or_else(|_| [oflh_core::impact::Impact::default()]);
        self.confirm(
            vec![ancestor.identity],
            vec![ActionTarget {
                key: key.into(),
                name: oflh_core::safe(&ancestor.name),
                pid: ancestor.identity.pid,
                role: TargetRole::Ancestor,
                owner: scanned
                    .map(|process| self.owner(process))
                    .unwrap_or_else(|| self.sessions.owner("", ancestor.identity.pid)),
                impact: target_impact(native, scanned),
            }],
            force,
        )
    }
    /// Capture every selected lifetime, including selections hidden by the current filter.
    pub fn prepare(
        &self,
        revision: u32,
        keys: &[String],
        force: bool,
        backend: &mut dyn Backend,
    ) -> Result<Confirmation, Failure> {
        if keys.is_empty() || keys.len() > 10000 {
            return Err(Failure::invalid("Select between 1 and 10,000 processes"));
        }
        let dataset = self.dataset(revision)?;
        let mut seen = HashSet::new();
        let mut identities = Vec::new();
        let mut processes = Vec::new();
        for key in keys {
            let (_, process) = dataset.process(key)?;
            process.identity.validate().map_err(Failure::from)?;
            if seen.insert(process.identity) {
                identities.push(process.identity);
                processes.push(process);
            }
        }
        let mut native = backend.impact(&identities).into_iter();
        let targets = processes
            .into_iter()
            .map(|process| ActionTarget {
                key: identity_key(process.identity),
                name: oflh_core::safe(&process.name),
                pid: process.identity.pid,
                role: TargetRole::Holder,
                owner: self.owner(process),
                impact: target_impact(native.next().unwrap_or_default(), Some(process)),
            })
            .collect();
        self.confirm(identities, targets, force)
    }
    fn confirm(
        &self,
        identities: Vec<Identity>,
        targets: Vec<ActionTarget>,
        force: bool,
    ) -> Result<Confirmation, Failure> {
        self.confirm_mode(identities, targets, force, false)
    }
    fn confirm_mode(
        &self,
        identities: Vec<Identity>,
        targets: Vec<ActionTarget>,
        force: bool,
        elevated: bool,
    ) -> Result<Confirmation, Failure> {
        let mut state = self.shared.lock();
        state.admin_recovery = None;
        state.next_ticket = state
            .next_ticket
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to perform another action"))?;
        let ticket = state.next_ticket.to_string();
        state.action = Some(PendingAction {
            ticket: ticket.clone(),
            identities,
            force,
            elevated,
            targets: targets.clone(),
        });
        Ok(Confirmation {
            ticket,
            force,
            elevated,
            targets,
        })
    }
    /// Confirm only permission-denied targets retained from the original result receipt.
    /// Refreshes do not replace captured identities; native validation occurs in the helper.
    pub fn prepare_elevated(&self, ticket: &str) -> Result<Confirmation, Failure> {
        let action = {
            let mut state = self.shared.lock();
            if state
                .admin_recovery
                .as_ref()
                .is_none_or(|action| action.ticket != ticket)
            {
                return Err(Failure::invalid(
                    "Administrator recovery expired; review the results again",
                ));
            }
            state
                .admin_recovery
                .take()
                .ok_or_else(|| Failure::invalid("No administrator recovery available"))?
        };
        for identity in &action.identities {
            identity.validate().map_err(Failure::from)?;
        }
        self.confirm_mode(action.identities, action.targets, action.force, true)
    }
    /// Invalidate an outstanding confirmation without sending any process signal.
    pub fn dismiss(&self) {
        self.shared.lock().action = None;
        self.sessions.dismiss();
    }
    /// Consume a confirmation exactly once. The caller cannot change the IDs or force mode.
    pub fn terminate(
        &self,
        ticket: &str,
        backend: &mut dyn Backend,
    ) -> Result<Vec<ActionResult>, Failure> {
        self.terminate_using(
            ticket,
            backend,
            oflh_platform::elevation::is_elevated(),
            |backend, identity, force, elevated, cancel| {
                if elevated {
                    let executable = std::env::current_exe()
                        .map_err(|error| oflh_core::io("locate administrator helper", error))?;
                    oflh_platform::elevation::terminate(&executable, identity, force, cancel)
                } else {
                    backend.terminate(identity, force, cancel)
                }
            },
        )
    }
    fn consume_action(&self, ticket: &str) -> Result<PendingAction, Failure> {
        {
            let mut state = self.shared.lock();
            state.admin_recovery = None;
            if state
                .action
                .as_ref()
                .is_none_or(|action| action.ticket != ticket)
            {
                return Err(Failure::invalid(
                    "Confirmation expired; review the selection again",
                ));
            }
            state
                .action
                .take()
                .ok_or_else(|| Failure::invalid("Confirmation expired"))
        }
    }
    fn terminate_using(
        &self,
        ticket: &str,
        backend: &mut dyn Backend,
        already_elevated: bool,
        mut request: impl FnMut(
            &mut dyn Backend,
            Identity,
            bool,
            bool,
            &Cancellation,
        ) -> oflh_core::Result<()>,
    ) -> Result<Vec<ActionResult>, Failure> {
        let action = self.consume_action(ticket)?;
        let cancel = Cancellation::default();
        let mut results: Vec<_> = action
            .identities
            .iter()
            .map(|&identity| {
                let error = identity
                    .validate()
                    .and_then(|()| cancel.check())
                    .and_then(|()| {
                        request(backend, identity, action.force, action.elevated, &cancel)
                    })
                    .err()
                    .map(Failure::from);
                if action.elevated
                    && error.as_ref().is_some_and(|failure| {
                        matches!(failure.kind.as_str(), "cancelled" | "unavailable")
                    })
                {
                    cancel.cancel();
                }
                let admin_recovery = !already_elevated
                    && !action.elevated
                    && error
                        .as_ref()
                        .is_some_and(|failure| failure.kind == "permission_denied");
                ActionResult {
                    admin_recovery,
                    pid: identity.pid,
                    outcome: if error.is_some() {
                        ActionOutcome::Failed
                    } else {
                        ActionOutcome::StillRunning
                    },
                    error,
                }
            })
            .collect();
        verify_action_results(backend, &action.identities, &mut results);
        let mut recovery = PendingAction {
            ticket: action.ticket.clone(),
            force: action.force,
            elevated: false,
            identities: Vec::new(),
            targets: Vec::new(),
        };
        for ((identity, target), result) in
            action.identities.iter().zip(&action.targets).zip(&results)
        {
            if result.admin_recovery {
                recovery.identities.push(*identity);
                recovery.targets.push(target.clone());
            }
        }
        if !recovery.identities.is_empty() {
            self.shared.lock().admin_recovery = Some(recovery);
        }
        Ok(results)
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.shutdown = true;
        state.cancellation.cancel();
        if let Some(wait) = &state.wait {
            wait.cancel.cancel();
        }
        state.pending = None;
        state.action = None;
        self.shared.ready.notify_one();
    }
}
fn verify_action_results(
    backend: &mut dyn Backend,
    identities: &[Identity],
    results: &mut [ActionResult],
) {
    // One shared deadline bounds multi-selection latency. Never signal again or
    // infer exit from a missing file/port row: verify the captured birth identity.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    loop {
        for (result, &identity) in results.iter_mut().zip(identities) {
            if result.outcome != ActionOutcome::StillRunning {
                continue;
            }
            match backend.is_running(identity) {
                Ok(false) => result.outcome = ActionOutcome::Exited,
                Ok(true) => {}
                Err(error) => {
                    result.outcome = ActionOutcome::Unverified;
                    result.error = Some(Failure::from(error));
                }
            }
        }
        if results
            .iter()
            .all(|result| result.outcome != ActionOutcome::StillRunning)
            || std::time::Instant::now() >= deadline
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
fn scan(backend: &mut dyn Backend, job: &ScanJob) -> Result<(Option<PathBuf>, Dataset), Failure> {
    job.cancel.check().map_err(Failure::from)?;
    let (path, mut snapshot) = match (&job.path, job.owner) {
        (Some(path), Some(owner)) => {
            let (target, snapshot) =
                oflh_platform::scan_port_folder(backend, owner, path, &job.cancel)
                    .map_err(Failure::from)?;
            (Some(target.path), snapshot)
        }
        (Some(path), None) => {
            let target = Target::new(path).map_err(Failure::from)?;
            let mut snapshot = oflh_platform::scan_with_ports(backend, &target, &job.cancel)
                .map_err(Failure::from)?;
            if target.metadata.is_none() {
                snapshot.warnings.push("The target no longer exists. Results use path matching and may include deleted open files.".into());
            }
            (Some(target.path), snapshot)
        }
        (None, _) => (
            None,
            oflh_platform::scan_ports(&job.cancel).map_err(Failure::from)?,
        ),
    };
    job.cancel.set_phase(oflh_core::InspectionPhase::Indexing);
    snapshot.normalize();
    let dataset = Dataset::new(job.generation, snapshot);
    job.cancel.check().map_err(Failure::from)?;
    Ok((path, dataset))
}

fn cancel_scan(state: &mut State) {
    state.cancellation.cancel();
    state.elapsed_ms = elapsed_ms(state);
    state.pending = None;
    // Advance the generation so even a native backend that returns after cancellation
    // cannot publish over the last accepted snapshot.
    state.generation = state.generation.saturating_add(1);
    state.scanning = false;
}

fn elapsed_ms(state: &State) -> u64 {
    if state.scanning {
        state.started.map_or(0, |started| {
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
        })
    } else {
        state.elapsed_ms
    }
}

fn status(state: &State) -> Status {
    Status {
        generation: state.generation,
        revision: state.dataset.revision,
        scanning: state.scanning,
        elapsed_ms: elapsed_ms(state),
        last_scan_elapsed_ms: state.last_scan_elapsed_ms,
        progress: state.cancellation.progress().into(),
        target: state.target.as_deref().map(display).unwrap_or_default(),
        processes: state.dataset.file_users(),
        ports: state.dataset.port_count(),
        usages: state.dataset.usage_count(),
        warnings: state
            .dataset
            .snapshot
            .warnings
            .iter()
            .map(|warning| oflh_core::safe(warning))
            .collect(),
        error: state.error.clone(),
        wait: WaitStatus {
            id: state.next_wait,
            target: state
                .wait
                .as_ref()
                .or(state.wait_completed.as_ref())
                .map(|job| job.description.clone()),
            waiting: state.wait.is_some(),
            elapsed_ms: state.wait_started.map_or(0, |started| {
                started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
            }),
            outcome: state
                .wait_outcome
                .as_ref()
                .map(|result| if result.is_ok() { "free" } else { "failed" }),
            error: state
                .wait_outcome
                .as_ref()
                .and_then(|result| result.as_ref().err().cloned()),
        },
        version: oflh_core::display_version(),
        commit: oflh_core::BUILD_COMMIT,
        build_url: oflh_core::BUILD_URL,
        pull_request_url: oflh_core::PULL_REQUEST_URL,
    }
}
/// Combine native observations with what the latest scan attributed to the process.
fn target_impact(native: oflh_core::impact::Impact, scanned: Option<&Process>) -> TargetImpact {
    use oflh_core::impact::{Capped, scanned_resources};
    let (files, ports) = scanned.map(scanned_resources).unwrap_or_default();
    TargetImpact {
        windows: native
            .windows
            .map(|titles| Capped::new(titles.iter().map(|title| oflh_core::safe(title))).into()),
        children: native.children.map(|children| {
            Capped::new(children.iter().map(|child| {
                format!(
                    "{} (PID {})",
                    oflh_core::safe(&child.name),
                    child.identity.pid
                )
            }))
            .into()
        }),
        files: files.into(),
        ports: ports.into(),
        service: native.service.as_deref().map(oflh_core::safe),
        critical: native.critical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Error, Metrics, Process, Result, Usage};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel},
    };
    use std::time::Duration;
    use std::time::Instant;
    const TIMEOUT: Duration = Duration::from_secs(5);
    struct Gated {
        started: Sender<PathBuf>,
        release: Receiver<()>,
    }
    impl Backend for Gated {
        fn scan(&mut self, target: &Target, _: &Cancellation) -> Result<Snapshot> {
            self.started.send(target.path.clone()).unwrap();
            self.release.recv_timeout(TIMEOUT).unwrap();
            // Deliberately ignore cancellation: publication still must reject stale work.
            Ok(Snapshot::default())
        }
        fn sample(&mut self, _: &[Identity], _: &Cancellation) -> Result<Vec<(Identity, Metrics)>> {
            unreachable!()
        }
        fn terminate(&mut self, _: Identity, _: bool, _: &Cancellation) -> Result<()> {
            unreachable!()
        }
    }
    #[test]
    fn active_inspection_is_preserved_until_explicit_cancellation() {
        let (started_sender, started) = channel();
        let (release, release_receiver) = channel();
        let (notifications, received) = channel();
        let service = Service::new(
            Box::new(Gated {
                started: started_sender,
                release: release_receiver,
            }),
            move |status| {
                notifications.send(status).unwrap();
            },
        )
        .unwrap();
        let base = std::env::temp_dir();
        let initial = service.inspect(base.join("oflh-desktop-first")).unwrap();
        assert_eq!(initial.last_scan_elapsed_ms, None);
        started.recv_timeout(TIMEOUT).unwrap();
        service.shared.lock().started =
            std::time::Instant::now().checked_sub(Duration::from_secs(2));
        for _ in 0..20 {
            assert_eq!(service.refresh().unwrap().generation, initial.generation);
            assert_eq!(
                service
                    .inspect(base.join("oflh-desktop-obsolete"))
                    .unwrap()
                    .generation,
                initial.generation
            );
            assert_eq!(service.ports().unwrap().generation, initial.generation);
        }
        assert!(service.shared.lock().cancellation.check().is_ok());
        assert!(started.try_recv().is_err());
        release.send(()).unwrap();
        let completed = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(completed.revision, initial.generation);
        assert!(!completed.scanning);
        assert_eq!(completed.last_scan_elapsed_ms, Some(completed.elapsed_ms));
        assert!(completed.elapsed_ms >= 2000);
        assert!(completed.target.ends_with("oflh-desktop-first"));
        assert_eq!(service.recent().len(), 1);
        let second = service
            .inspect(base.join("oflh-desktop-cancelled"))
            .unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(second.last_scan_elapsed_ms, completed.last_scan_elapsed_ms);
        service
            .shared
            .lock()
            .cancellation
            .record(oflh_core::InspectionCounter::Files, 17);
        service.shared.lock().started =
            std::time::Instant::now().checked_sub(std::time::Duration::from_secs(2));
        let progress = service.status();
        assert_eq!(progress.progress.files, 17);
        assert!(progress.scanning);
        assert!(progress.elapsed_ms >= 2000);
        let cancelled = service.cancel();
        assert!(!cancelled.scanning);
        assert_eq!(cancelled.revision, completed.revision);
        assert_eq!(
            cancelled.last_scan_elapsed_ms,
            completed.last_scan_elapsed_ms
        );
        assert!(cancelled.generation > second.generation);
        assert!(cancelled.elapsed_ms >= progress.elapsed_ms);
        assert_eq!(service.status().elapsed_ms, cancelled.elapsed_ms);
        let next = service.inspect(base.join("oflh-desktop-next")).unwrap();
        assert_eq!(next.last_scan_elapsed_ms, completed.last_scan_elapsed_ms);
        assert_eq!(next.progress.files, 0);
        release.send(()).unwrap();
        assert!(
            started
                .recv_timeout(TIMEOUT)
                .unwrap()
                .ends_with("oflh-desktop-next")
        );
        release.send(()).unwrap();
        let completed = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(completed.revision, next.generation);
        assert_eq!(completed.last_scan_elapsed_ms, Some(completed.elapsed_ms));
        assert!(received.try_recv().is_err());
    }
    #[test]
    fn failed_inspection_preserves_completed_duration_and_refresh_updates_it() {
        let (started_sender, started) = channel();
        let (release, release_receiver) = channel();
        let (notifications, received) = channel();
        let service = Service::new(
            Box::new(Gated {
                started: started_sender,
                release: release_receiver,
            }),
            move |status| {
                notifications.send(status).unwrap();
            },
        )
        .unwrap();
        let folder = tempfile::tempdir().unwrap();
        service.inspect(folder.path().to_path_buf()).unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        service.shared.lock().started =
            std::time::Instant::now().checked_sub(Duration::from_secs(3));
        release.send(()).unwrap();
        let first = received.recv_timeout(TIMEOUT).unwrap();
        assert!(
            first
                .last_scan_elapsed_ms
                .is_some_and(|duration| duration >= 3000)
        );
        let native_target = service.shared.lock().target.clone().unwrap();
        assert_eq!(native_target, std::fs::canonicalize(folder.path()).unwrap());
        // The duration PR is independent of Windows path presentation. Both
        // ordinary and extended text must resolve to the exact native target.
        assert_eq!(std::fs::canonicalize(&first.target).unwrap(), native_target);

        service.inspect(folder.path().join("\0invalid")).unwrap();
        let failed = received.recv_timeout(TIMEOUT).unwrap();
        assert!(failed.error.is_some());
        assert_eq!(failed.revision, first.revision);
        assert_eq!(failed.last_scan_elapsed_ms, first.last_scan_elapsed_ms);
        assert!(started.try_recv().is_err());

        let refresh = service.refresh().unwrap();
        assert_eq!(refresh.last_scan_elapsed_ms, first.last_scan_elapsed_ms);
        assert_eq!(started.recv_timeout(TIMEOUT).unwrap(), native_target);
        service.shared.lock().started =
            std::time::Instant::now().checked_sub(Duration::from_secs(4));
        release.send(()).unwrap();
        let refreshed = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(refreshed.revision, refresh.generation);
        assert!(
            refreshed
                .last_scan_elapsed_ms
                .is_some_and(|duration| duration >= 4000)
        );
        assert_eq!(refreshed.last_scan_elapsed_ms, Some(refreshed.elapsed_ms));
        assert_eq!(service.shared.lock().target, Some(native_target));

        service.ports().unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        release.send(()).unwrap();
        let ports = received.recv_timeout(TIMEOUT).unwrap();
        assert!(!ports.scanning);
        assert!(ports.error.is_none());
        assert_eq!(ports.last_scan_elapsed_ms, Some(ports.elapsed_ms));
    }
    #[derive(Default)]
    struct Recorder {
        impact: Option<oflh_core::impact::Impact>,
        calls: Vec<(Identity, bool)>,
        changed: bool,
        running: bool,
        verification_error: bool,
    }
    impl Backend for Recorder {
        fn scan(&mut self, _: &Target, _: &Cancellation) -> Result<Snapshot> {
            Ok(Snapshot::default())
        }
        fn sample(&mut self, _: &[Identity], _: &Cancellation) -> Result<Vec<(Identity, Metrics)>> {
            Ok(vec![])
        }
        fn is_running(&mut self, _: Identity) -> Result<bool> {
            if self.verification_error {
                Err(Error::Unavailable(
                    "access denied during verification".into(),
                ))
            } else {
                Ok(self.running)
            }
        }
        fn impact(&mut self, ids: &[Identity]) -> Vec<oflh_core::impact::Impact> {
            vec![self.impact.clone().unwrap_or_default(); ids.len()]
        }
        fn terminate(&mut self, identity: Identity, force: bool, _: &Cancellation) -> Result<()> {
            self.calls.push((identity, force));
            if self.changed {
                Err(Error::Changed)
            } else {
                Ok(())
            }
        }
    }

    struct HeldFixture {
        scans: Arc<AtomicUsize>,
    }
    impl Backend for HeldFixture {
        fn scan(&mut self, _: &Target, _: &Cancellation) -> Result<Snapshot> {
            self.scans.fetch_add(1, Ordering::SeqCst);
            Ok(Snapshot {
                processes: vec![Process {
                    usages: vec![Usage::default()],
                    ..Process::default()
                }],
                warnings: vec![],
            })
        }
        fn sample(&mut self, _: &[Identity], _: &Cancellation) -> Result<Vec<(Identity, Metrics)>> {
            Ok(vec![])
        }
        fn terminate(&mut self, _: Identity, _: bool, _: &Cancellation) -> Result<()> {
            unreachable!()
        }
    }
    #[test]
    fn wait_acknowledges_immediately_and_can_cancel() {
        let service = Service::new(
            Box::new(HeldFixture {
                scans: Arc::new(AtomicUsize::new(0)),
            }),
            |_| {},
        )
        .unwrap();
        assert!(
            service
                .wait_until_free(WaitRequest {
                    path: Some(std::env::temp_dir()),
                    include_ports: false,
                    port: None,
                    timeout: Some(Duration::from_secs(30))
                })
                .wait
                .waiting
        );
        let cancelled = service.cancel_wait();
        assert!(!cancelled.wait.waiting);
        assert_eq!(cancelled.wait.error.as_ref().unwrap().kind, "cancelled");
    }
    #[test]
    fn selection_wait_retains_the_exact_native_file_and_rejects_cancelled_job_completions() {
        let folder = tempfile::tempdir().unwrap();
        let first = folder.path().join("first.pdf");
        let second = folder.path().join("second.pdf");
        std::fs::write(&first, b"fixture").unwrap();
        std::fs::write(&second, b"fixture").unwrap();
        let (started_sender, started) = channel();
        let (release, release_receiver) = channel();
        let (notifications, completed) = channel();
        let service = Service::new(
            Box::new(Gated {
                started: started_sender,
                release: release_receiver,
            }),
            move |status| {
                notifications.send(status).unwrap();
            },
        )
        .unwrap();
        {
            let mut state = service.shared.lock();
            state.target = Some(folder.path().into());
            state.dataset = Arc::new(Dataset::new(
                7,
                Snapshot {
                    processes: vec![Process {
                        identity: Identity {
                            pid: 424242,
                            started: 10,
                            started_sub: 1,
                        },
                        usages: vec![
                            oflh_core::Usage {
                                path: first.clone(),
                                ..Default::default()
                            },
                            oflh_core::Usage {
                                path: second.clone(),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    }],
                    warnings: vec![],
                },
            ));
        }
        let request = |revision, reference: &str| SelectionWaitRequest {
            revision,
            target: SelectionWaitTarget::File {
                reference: reference.into(),
            },
            timeout_seconds: None,
            delete_after: false,
        };
        assert!(service.wait_for_selection(request(6, "7:0:0")).is_err());
        assert!(service.wait_for_selection(request(7, "6:0:0")).is_err());
        assert!(!service.status().wait.waiting);
        let initial = service.wait_for_selection(request(7, "7:0:0")).unwrap();
        assert_eq!(initial.revision, 7);
        assert_eq!(
            started.recv_timeout(TIMEOUT).unwrap(),
            Target::new(&first).unwrap().path
        );
        assert!(service.wait_for_selection(request(7, "7:0:1")).is_err());
        service.cancel_wait();
        let next = service.wait_for_selection(request(7, "7:0:1")).unwrap();
        assert_ne!(initial.wait.id, next.wait.id);
        release.send(()).unwrap();
        assert_eq!(
            started.recv_timeout(TIMEOUT).unwrap(),
            Target::new(&second).unwrap().path
        );
        assert!(completed.try_recv().is_err());
        assert!(service.status().wait.waiting);
        release.send(()).unwrap();
        let done = completed.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(done.wait.id, next.wait.id);
        assert_eq!(done.wait.target.as_ref().unwrap().display, display(&second));
        assert_eq!(done.wait.outcome, Some("free"));
        assert!(service.completed_wait_target(initial.wait.id).is_err());
        assert!(service.shared_wait_path(initial.wait.id).is_err());
        assert_eq!(service.shared_wait_path(next.wait.id).unwrap(), second);
    }
    #[test]
    fn process_wait_rejects_pid_only_keys_and_never_authorizes_a_file_follow_up() {
        let (service, identity) = action_service();
        let request = |key| SelectionWaitRequest {
            revision: 1,
            target: SelectionWaitTarget::Process { key },
            timeout_seconds: Some(30),
            delete_after: false,
        };
        assert!(
            service
                .wait_for_selection(request(identity.pid.to_string()))
                .is_err()
        );
        assert!(
            service
                .wait_for_selection(request(format!("{}:0:0", identity.pid)))
                .is_err()
        );
        let started = service
            .wait_for_selection(request(identity_key(identity)))
            .unwrap();
        let deadline = Instant::now() + TIMEOUT;
        while service.status().wait.waiting && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(service.status().wait.outcome, Some("free"));
        assert_eq!(
            service.completed_wait_target(started.wait.id).unwrap().kind,
            "process"
        );
        assert!(
            service
                .wait_follow_up(started.wait.id, WaitFollowUp::Delete)
                .is_err()
        );
        assert!(service.shared_wait_path(started.wait.id).is_err());
    }
    #[test]
    fn selected_port_wait_counts_bindings_without_any_filesystem_target() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (notifications, completed) = channel();
        let service = Service::new(Box::new(Recorder::default()), move |status| {
            notifications.send(status).unwrap();
        })
        .unwrap();
        service.shared.lock().dataset = Arc::new(Dataset::new(
            1,
            Snapshot {
                processes: vec![Process {
                    ports: vec![oflh_core::Port {
                        protocol: oflh_core::Protocol::Tcp,
                        address: address.ip(),
                        number: address.port(),
                    }],
                    ..Process::default()
                }],
                warnings: vec![],
            },
        ));
        service
            .wait_for_selection(SelectionWaitRequest {
                revision: 1,
                target: SelectionWaitTarget::Port {
                    reference: "1:0:port-0".into(),
                },
                timeout_seconds: Some(5),
                delete_after: false,
            })
            .unwrap();
        assert!(matches!(
            completed.recv_timeout(Duration::from_millis(200)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        assert!(service.status().wait.waiting);
        drop(listener);
        let done = completed.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(done.wait.outcome, Some("free"));
        assert_eq!(
            done.wait.target.unwrap().display,
            format!("Port {}", address.port())
        );
    }
    #[test]
    fn wait_timeout_publishes_failure() {
        let scans = Arc::new(AtomicUsize::new(0));
        let (sender, received) = channel();
        let service = Service::new(
            Box::new(HeldFixture {
                scans: scans.clone(),
            }),
            move |status| {
                if status.wait.error.is_some() {
                    let _ = sender.send(status.wait);
                }
            },
        )
        .unwrap();
        request_with_short_timeout(&service);
        let wait = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(wait.error.unwrap().kind, "wait_timeout");
        assert!(scans.load(Ordering::SeqCst) > 0);
    }
    fn request_with_short_timeout(service: &Service) {
        service.wait_until_free(WaitRequest {
            path: Some(std::env::temp_dir()),
            include_ports: false,
            port: None,
            timeout: Some(Duration::from_millis(150)),
        });
    }
    fn action_service() -> (Service, Identity) {
        let service = Service::new(Box::new(Recorder::default()), |_| {}).unwrap();
        let identity = Identity {
            pid: u32::MAX - 1,
            started: u64::MAX,
            started_sub: 21,
        };
        service.shared.lock().dataset = Arc::new(Dataset::new(
            1,
            Snapshot {
                processes: vec![Process {
                    identity,
                    name: "fixture".into(),
                    ..Process::default()
                }],
                warnings: vec![],
            },
        ));
        (service, identity)
    }
    /// Integration: scan data, session backend and tickets together, from
    /// details through the termination confirmation to sign-out and share close.
    #[test]
    fn multi_user_flow_from_details_to_sign_out_and_share_close() {
        use oflh_sessions::{Fake, Os, Session, SessionKind, SessionState, ShareFile};
        let (service, identity) = action_service();
        {
            let mut shared = service.shared.lock();
            let mut snapshot = shared.dataset.snapshot.clone();
            snapshot.processes[0].user = "max.muster".into();
            shared.dataset = Arc::new(Dataset::new(1, snapshot));
            shared.target = Some(PathBuf::from("c:/shares/finance/Q3.XLSX"));
        }
        let fake = Arc::new(Fake {
            own: Some("1".into()),
            ..Fake::default()
        });
        fake.sessions.lock().unwrap().push(Session {
            id: "3".into(),
            user: "max.muster".into(),
            kind: SessionKind::Rdp,
            state: SessionState::Active,
            client: Some("LAPTOP-7".into()),
            since: Some(1_700_000_000),
            ..Session::default()
        });
        fake.processes
            .lock()
            .unwrap()
            .insert(identity.pid, "3".into());
        fake.share_files.lock().unwrap().push(ShareFile {
            id: 71,
            path: r"C:\Shares\Finance\Q3.xlsx".into(),
            user: "anna".into(),
            client: Some("PC-ANNA".into()),
            locks: 1,
            write: true,
        });
        let service = service.with_sessions(crate::sessions::Sessions::new(
            fake.clone(),
            Os::Windows,
            true,
        ));
        let key = identity_key(identity);

        let details = service.details(1, &key).unwrap();
        assert_eq!(details.owner.user, "max.muster");
        let session = details.owner.session.as_ref().unwrap();
        assert_eq!((session.id.as_str(), session.kind), ("3", "rdp"));
        let actions = details.session_actions.as_ref().unwrap();
        assert_eq!(
            (actions.send_message, actions.sign_out),
            ("available", "available")
        );

        let termination = service
            .prepare(
                1,
                std::slice::from_ref(&key),
                false,
                &mut Recorder::default(),
            )
            .unwrap();
        assert_eq!(
            termination.targets[0]
                .owner
                .session
                .as_ref()
                .map(|s| s.id.as_str()),
            Some("3")
        );

        service.send_message(1, &key, "Restart at 18:00").unwrap();
        let confirmation = service.prepare_sign_out(1, &key).unwrap();
        assert!(confirmation.processes.items[0].starts_with("fixture (PID "));
        service.sign_out(&confirmation.ticket).unwrap();
        assert!(
            service.sign_out(&confirmation.ticket).is_err(),
            "single use"
        );

        let shares = service.share_files();
        assert_eq!((shares.close, shares.files.len()), ("available", 1));
        let close = service.prepare_close_file(71).unwrap();
        service.close_file(&close.ticket).unwrap();
        assert_eq!(
            fake.actions(),
            ["message 3 Restart at 18:00", "sign_out 3", "close 71"]
        );
    }
    #[test]
    fn closing_an_inspection_invalidates_results_actions_and_retains_recent_targets() {
        let (service, identity) = action_service();
        let revision = service.status().revision;
        let confirmation = service
            .prepare(
                revision,
                &[identity_key(identity)],
                false,
                &mut Recorder::default(),
            )
            .unwrap();
        service
            .shared
            .lock()
            .recent
            .push((1, PathBuf::from("/workspace/project")));
        let old_cancel = service.shared.lock().cancellation.clone();
        let closed = service.close_inspection().unwrap();
        assert_eq!(closed.revision, 0);
        assert!(closed.target.is_empty());
        assert_eq!((closed.processes, closed.usages, closed.ports), (0, 0, 0));
        assert!(closed.last_scan_elapsed_ms.is_none());
        assert!(!closed.scanning);
        assert!(service.dataset(revision).is_err());
        assert!(service.consume_action(&confirmation.ticket).is_err());
        assert!(old_cancel.check().is_err());
        assert_eq!(service.recent().len(), 1);
        assert!(service.refresh().is_err());
    }
    #[test]
    fn administrator_recovery_preserves_receipt_identity_mode_and_cancel() {
        for force in [false, true] {
            let (service, identity) = action_service();
            let confirmation = service
                .prepare(
                    1,
                    &[identity_key(identity)],
                    force,
                    &mut Recorder::default(),
                )
                .unwrap();
            let mut backend = Recorder::default();
            let results = service
                .terminate_using(
                    &confirmation.ticket,
                    &mut backend,
                    false,
                    |_, _, _, elevated, _| {
                        assert!(!elevated);
                        Err(oflh_core::io(
                            "test termination",
                            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
                        ))
                    },
                )
                .unwrap();
            assert!(results[0].admin_recovery);
            assert!(service.prepare_elevated("forged").is_err());
            // A refresh cannot replace the identities retained by the result receipt.
            service.shared.lock().dataset = Arc::new(Dataset::new(2, Snapshot::default()));
            let retry = service.prepare_elevated(&confirmation.ticket).unwrap();
            assert!(retry.elevated);
            assert_eq!(retry.force, force);
            assert_eq!(retry.targets[0].key, identity_key(identity));
            assert!(service.prepare_elevated(&confirmation.ticket).is_err());
            let results = service
                .terminate_using(
                    &retry.ticket,
                    &mut backend,
                    false,
                    |_, captured, retry_force, elevated, _| {
                        assert_eq!(captured, identity);
                        assert_eq!(retry_force, force);
                        assert!(elevated);
                        Err(Error::Cancelled)
                    },
                )
                .unwrap();
            assert_eq!(results[0].error.as_ref().unwrap().kind, "cancelled");
            assert!(!results[0].admin_recovery);
            assert!(service.prepare_elevated(&retry.ticket).is_err());
        }
    }
    #[test]
    fn administrator_recovery_filters_mixed_results_and_stops_after_authorization_cancel() {
        let (service, identity) = action_service();
        let identities: Vec<_> = (0..4)
            .map(|index| Identity {
                pid: identity.pid - index,
                ..identity
            })
            .collect();
        let targets = identities
            .iter()
            .map(|identity| ActionTarget {
                key: identity_key(*identity),
                pid: identity.pid,
                name: "fixture".into(),
                role: TargetRole::Holder,
                owner: OwnerView::default(),
                impact: TargetImpact::default(),
            })
            .collect();
        let confirmation = service.confirm(identities.clone(), targets, true).unwrap();
        let mut call = 0;
        let results = service
            .terminate_using(
                &confirmation.ticket,
                &mut Recorder::default(),
                false,
                |_, _, _, _, _| {
                    call += 1;
                    match call {
                        1 | 2 => Err(oflh_core::io(
                            "test",
                            std::io::ErrorKind::PermissionDenied.into(),
                        )),
                        3 => Err(Error::Changed),
                        _ => Ok(()),
                    }
                },
            )
            .unwrap();
        assert_eq!(
            results
                .iter()
                .filter(|result| result.admin_recovery)
                .count(),
            2
        );
        let retry = service.prepare_elevated(&confirmation.ticket).unwrap();
        assert_eq!(retry.targets.len(), 2);
        call = 0;
        let results = service
            .terminate_using(
                &retry.ticket,
                &mut Recorder::default(),
                false,
                |_, captured, _, elevated, _| {
                    call += 1;
                    assert_eq!(captured, identities[0]);
                    assert!(elevated);
                    Err(Error::Cancelled)
                },
            )
            .unwrap();
        assert_eq!(
            call, 1,
            "cancelling authorization must stop the remaining requests"
        );
        assert!(
            results
                .iter()
                .all(|result| result.error.as_ref().unwrap().kind == "cancelled")
        );
        assert!(results.iter().all(|result| !result.admin_recovery));
    }
    #[test]
    fn administrator_recovery_excludes_other_errors_and_elevated_sessions() {
        for (already_elevated, error) in [
            (
                true,
                Error::Io {
                    operation: "test",
                    source: std::io::ErrorKind::PermissionDenied.into(),
                },
            ),
            (false, Error::Changed),
            (false, Error::Protected),
            (false, Error::Cancelled),
        ] {
            let (service, identity) = action_service();
            let confirmation = service
                .prepare(1, &[identity_key(identity)], true, &mut Recorder::default())
                .unwrap();
            let mut error = Some(error);
            let results = service
                .terminate_using(
                    &confirmation.ticket,
                    &mut Recorder::default(),
                    already_elevated,
                    |_, _, _, _, _| Err(error.take().unwrap()),
                )
                .unwrap();
            assert!(!results[0].admin_recovery);
            assert!(service.prepare_elevated(&confirmation.ticket).is_err());
        }
    }
    #[test]
    fn recent_target_removal_updates_service_and_database_together() {
        let (service, _) = action_service();
        let first = PathBuf::from("/tmp/oflh-recent-first");
        let second = PathBuf::from("/tmp/oflh-recent-second");
        {
            let mut recent_targets = service.shared.lock_recent_targets();
            recent_targets.record(&first).unwrap();
            recent_targets.record(&second).unwrap();
        }
        service.shared.lock().recent = vec![(1, first.clone()), (2, second.clone())];

        service.remove_recent(1).unwrap();
        assert_eq!(service.recent().len(), 1);
        assert_eq!(
            service.shared.lock_recent_targets().load().unwrap(),
            vec![second.clone()]
        );

        service.clear_recent().unwrap();
        assert!(service.recent().is_empty());
        assert!(
            service
                .shared
                .lock_recent_targets()
                .load()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn verification_distinguishes_exit_pending_and_unknown_without_resending() {
        for (running, verification_error, expected) in [
            (false, false, ActionOutcome::Exited),
            (true, false, ActionOutcome::StillRunning),
            (false, true, ActionOutcome::Unverified),
        ] {
            let (service, identity) = action_service();
            let confirmation = service
                .prepare(
                    1,
                    &[identity_key(identity)],
                    false,
                    &mut Recorder::default(),
                )
                .unwrap();
            let mut backend = Recorder {
                running,
                verification_error,
                ..Recorder::default()
            };
            let results = service
                .terminate(&confirmation.ticket, &mut backend)
                .unwrap();
            assert_eq!(results[0].outcome, expected);
            assert_eq!(backend.calls, [(identity, false)]);
        }
    }
    #[test]
    fn focused_ancestry_retains_birth_identity_and_rejects_forged_parent() {
        let (service, identity) = action_service();
        let parent = Identity {
            pid: 42000,
            started: 12,
            started_sub: 0,
        };
        let snapshot = |parent| Snapshot {
            processes: vec![Process {
                identity,
                ancestors: vec![Ancestor {
                    identity: parent,
                    name: "fixture-parent".into(),
                }],
                ..Process::default()
            }],
            ..Snapshot::default()
        };
        service.shared.lock().dataset = Arc::new(Dataset::new(2, snapshot(parent)));
        let owner = identity_key(identity);
        let captured = service.details(2, &owner).unwrap();
        service.shared.lock().dataset = Arc::new(Dataset::new(
            3,
            snapshot(Identity {
                started: 13,
                ..parent
            }),
        ));
        assert_eq!(
            service.details(3, &owner).unwrap().ancestors[0].key,
            captured.ancestors[0].key
        );
        assert!(
            service
                .prepare_ancestor(&owner, "42000:13:0", true, &mut Recorder::default())
                .is_err()
        );
        assert!(
            service
                .prepare_ancestor(
                    "forged",
                    &identity_key(parent),
                    true,
                    &mut Recorder::default()
                )
                .is_err()
        );
        let confirmation = service
            .prepare_ancestor(
                &owner,
                &identity_key(parent),
                true,
                &mut Recorder::default(),
            )
            .unwrap();
        let mut backend = Recorder::default();
        service
            .terminate(&confirmation.ticket, &mut backend)
            .unwrap();
        assert_eq!(backend.calls, [(parent, true)]);
        assert_eq!(confirmation.targets[0].role, TargetRole::Ancestor);
    }
    #[test]
    fn confirmation_carries_capped_impact_without_changing_the_action() {
        use oflh_core::{Port, Protocol, Usage, impact::Impact};
        let (service, identity) = action_service();
        service.shared.lock().dataset = Arc::new(Dataset::new(
            1,
            Snapshot {
                processes: vec![Process {
                    identity,
                    name: "blender".into(),
                    usages: vec![Usage {
                        path: "/build/scene.blend".into(),
                        ..Usage::default()
                    }],
                    ports: vec![Port {
                        protocol: Protocol::Tcp,
                        address: std::net::Ipv4Addr::UNSPECIFIED.into(),
                        number: 3000,
                    }],
                    ..Process::default()
                }],
                warnings: vec![],
            },
        ));
        let child = |pid| Ancestor {
            identity: Identity {
                pid,
                started: 1,
                started_sub: 0,
            },
            name: "worker".into(),
        };
        let mut backend = Recorder {
            impact: Some(Impact {
                windows: Some((0..7).map(|n| format!("scene {n}.blend")).collect()),
                children: Some(vec![child(7), child(8)]),
                service: Some("Blender Render Farm".into()),
                critical: false,
            }),
            ..Recorder::default()
        };
        let confirmation = service
            .prepare(1, &[identity_key(identity)], false, &mut backend)
            .unwrap();
        let target = &confirmation.targets[0];
        assert_eq!(target.role, TargetRole::Holder);
        let windows = target.impact.windows.as_ref().unwrap();
        assert_eq!((windows.items.len(), windows.more), (7, 0));
        assert_eq!(
            target.impact.children.as_ref().unwrap().items,
            ["worker (PID 7)", "worker (PID 8)"]
        );
        assert_eq!(target.impact.files.items, ["/build/scene.blend"]);
        assert_eq!(target.impact.ports.items, ["TCP 3000"]);
        assert_eq!(
            target.impact.service.as_deref(),
            Some("Blender Render Farm")
        );
        assert!(!target.impact.critical);
        let json = serde_json::to_value(&confirmation).unwrap();
        assert_eq!(json["targets"][0]["role"], "holder");
        assert_eq!(json["targets"][0]["impact"]["windows"]["more"], 0);
        assert_eq!(
            json["targets"][0]["impact"]["service"],
            "Blender Render Farm"
        );
        assert_eq!(
            json["targets"][0]["impact"]["ports"]["items"][0],
            "TCP 3000"
        );
        service
            .terminate(&confirmation.ticket, &mut backend)
            .unwrap();
        assert_eq!(backend.calls, [(identity, false)]);
    }
    #[test]
    fn unavailable_impact_serializes_as_null_not_empty() {
        let (service, identity) = action_service();
        let confirmation = service
            .prepare(
                1,
                &[identity_key(identity)],
                false,
                &mut Recorder::default(),
            )
            .unwrap();
        let json = serde_json::to_value(&confirmation).unwrap();
        assert!(json["targets"][0]["impact"]["windows"].is_null());
        assert!(json["targets"][0]["impact"]["children"].is_null());
        assert!(json["targets"][0]["impact"]["service"].is_null());
        assert_eq!(json["targets"][0]["impact"]["critical"], false);
        assert_eq!(
            json["targets"][0]["impact"]["files"]["items"],
            serde_json::json!([])
        );
    }
    #[test]
    fn tickets_capture_mode_deduplicate_and_cannot_be_replayed() {
        let (service, identity) = action_service();
        let key = identity_key(identity);
        let confirmation = service
            .prepare(1, &[key.clone(), key], true, &mut Recorder::default())
            .unwrap();
        assert_eq!(confirmation.targets.len(), 1);
        let mut backend = Recorder::default();
        assert!(service.terminate("forged", &mut backend).is_err());
        let result = service
            .terminate(&confirmation.ticket, &mut backend)
            .unwrap();
        assert!(result[0].error.is_none());
        assert_eq!(backend.calls, vec![(identity, true)]);
        assert!(
            service
                .terminate(&confirmation.ticket, &mut backend)
                .is_err()
        );
        assert_eq!(backend.calls.len(), 1);
    }
    #[test]
    fn changed_identity_reports_failure_without_escalation() {
        let (service, identity) = action_service();
        let confirmation = service
            .prepare(
                1,
                &[identity_key(identity)],
                false,
                &mut Recorder::default(),
            )
            .unwrap();
        let mut backend = Recorder {
            changed: true,
            ..Recorder::default()
        };
        // A refresh is allowed while confirmation is open; the ticket retains the old birth token.
        service.shared.lock().dataset = Arc::new(Dataset::new(2, Snapshot::default()));
        let results = service
            .terminate(&confirmation.ticket, &mut backend)
            .unwrap();
        assert_eq!(results[0].error.as_ref().unwrap().kind, "identity_changed");
        assert_eq!(backend.calls, vec![(identity, false)]);
    }
    #[test]
    fn invalid_selections_and_dismissed_confirmations_cannot_act() {
        let (service, identity) = action_service();
        assert!(
            service
                .prepare(1, &[], false, &mut Recorder::default())
                .is_err()
        );
        assert!(
            service
                .prepare(
                    0,
                    &[identity_key(identity)],
                    false,
                    &mut Recorder::default()
                )
                .is_err()
        );
        assert!(
            service
                .prepare(1, &["1:1:0".into()], false, &mut Recorder::default())
                .is_err()
        );
        let confirmation = service
            .prepare(
                1,
                &[identity_key(identity)],
                false,
                &mut Recorder::default(),
            )
            .unwrap();
        service.dismiss();
        let mut backend = Recorder::default();
        assert!(
            service
                .terminate(&confirmation.ticket, &mut backend)
                .is_err()
        );
        assert!(backend.calls.is_empty());
        service.shared.lock().dataset = Arc::new(Dataset::new(
            2,
            Snapshot {
                processes: vec![Process {
                    identity: Identity {
                        pid: 1,
                        started: 1,
                        started_sub: 0,
                    },
                    ..Process::default()
                }],
                warnings: vec![],
            },
        ));
        assert_eq!(
            service
                .prepare(2, &["1:1:0".into()], false, &mut Recorder::default())
                .unwrap_err()
                .kind,
            "protected"
        );
    }

    #[test]
    fn another_launch_replaces_a_running_inspection() {
        let (started_sender, started) = channel();
        let (release, release_receiver) = channel();
        let (notifications, received) = channel();
        let service = Service::new(
            Box::new(Gated {
                started: started_sender,
                release: release_receiver,
            }),
            move |status| {
                notifications.send(status).unwrap();
            },
        )
        .unwrap();
        let base = std::env::temp_dir();
        let running = service.inspect(base.join("oflh-desktop-running")).unwrap();
        assert!(
            started
                .recv_timeout(TIMEOUT)
                .unwrap()
                .ends_with("oflh-desktop-running")
        );
        let running_cancellation = service.shared.lock().cancellation.clone();

        let replaced = service
            .take_over(base.join("oflh-desktop-explorer"))
            .unwrap();
        assert!(replaced.scanning);
        assert!(replaced.generation > running.generation + 1);
        assert!(running_cancellation.check().is_err());

        // The cancelled scan returns late and must not publish.
        release.send(()).unwrap();
        assert!(
            started
                .recv_timeout(TIMEOUT)
                .unwrap()
                .ends_with("oflh-desktop-explorer")
        );
        release.send(()).unwrap();
        let published = received.recv_timeout(TIMEOUT).unwrap();
        assert!(!published.scanning);
        assert_eq!(published.generation, replaced.generation);
        assert!(published.target.ends_with("oflh-desktop-explorer"));
        assert_eq!(service.recent().len(), 1);
        assert!(service.take_over(PathBuf::new()).is_err());
    }
    #[test]
    fn explicit_wait_follow_up_rejects_target_held_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("held.bin");
        std::fs::write(&path, b"keep").unwrap();
        let (service, _) = action_service();
        service.shared.lock().wait_completed = Some(WaitJob {
            id: 1,
            process: None,
            description: WaitTargetView {
                kind: "file",
                display: display(&path),
            },
            path: Some(path.clone()),
            port: None,
            timeout: None,
            cancel: Cancellation::default(),
            started: Instant::now(),
        });
        service.shared.lock().wait_outcome = Some(Ok(()));
        let mut recorder = HeldFixture {
            scans: Arc::new(AtomicUsize::new(0)),
        };
        let held = service.wait_follow_up_with_backend(1, WaitFollowUp::Delete, &mut recorder);
        assert_eq!(held.unwrap_err().kind, "wait_target_held");
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    }
    fn completed_file_wait(path: &Path) -> Service {
        let service = Service::new(Box::new(Recorder::default()), |_| {}).unwrap();
        let mut state = service.shared.lock();
        state.next_wait = 1;
        state.wait_completed = Some(WaitJob {
            id: 1,
            process: None,
            description: WaitTargetView {
                kind: "file",
                display: display(path),
            },
            path: Some(path.to_owned()),
            port: None,
            timeout: None,
            cancel: Cancellation::default(),
            started: Instant::now(),
        });
        state.wait_outcome = Some(Ok(()));
        drop(state);
        service
    }
    #[test]
    fn move_and_rename_return_the_actual_result_and_cannot_be_replayed() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("Bestätigung.pdf");
        let folder = root.path().join("destination");
        std::fs::write(&original, b"keep").unwrap();
        std::fs::create_dir(&folder).unwrap();
        let service = completed_file_wait(&original);
        let selected = service.capture_wait_destination(folder.clone()).unwrap();
        assert!(service.wait_destination(&display(&folder)).is_err());
        let actual = service
            .wait_follow_up_with_backend(
                1,
                WaitFollowUp::Move(service.wait_destination(&selected.reference).unwrap()),
                &mut Recorder::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            actual,
            std::fs::canonicalize(folder)
                .unwrap()
                .join("Bestätigung.pdf")
        );
        assert_eq!(std::fs::read(&actual).unwrap(), b"keep");
        assert!(!original.exists());
        assert!(
            service
                .wait_follow_up_with_backend(1, WaitFollowUp::Delete, &mut Recorder::default())
                .is_err()
        );
        let service = completed_file_wait(&actual);
        let renamed = service
            .wait_follow_up_with_backend(
                1,
                WaitFollowUp::Rename("Neue Bestätigung.pdf".into()),
                &mut Recorder::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            renamed,
            actual.parent().unwrap().join("Neue Bestätigung.pdf")
        );
        assert_eq!(std::fs::read(renamed).unwrap(), b"keep");
        assert!(!actual.exists());
    }
    #[test]
    fn folder_picker_references_expire_and_reject_arbitrary_paths() {
        let root = tempfile::tempdir().unwrap();
        let service = completed_file_wait(&root.path().join("unused"));
        let first = service
            .capture_wait_destination(root.path().into())
            .unwrap();
        let second = service
            .capture_wait_destination(root.path().into())
            .unwrap();
        assert_ne!(first.reference, second.reference);
        assert!(service.wait_destination(&first.reference).is_err());
        assert_eq!(
            service.wait_destination(&second.reference).unwrap(),
            std::fs::canonicalize(root.path()).unwrap()
        );
        assert!(service.wait_destination("../../other").is_err());
    }
    #[test]
    fn renaming_and_moving_do_not_replace_a_destination_or_bypass_a_returning_holder() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("source.pdf");
        let destination = root.path().join("existing.pdf");
        std::fs::write(&original, b"source").unwrap();
        std::fs::write(&destination, b"destination").unwrap();
        let service = completed_file_wait(&original);
        let failure = service
            .wait_follow_up_with_backend(
                1,
                WaitFollowUp::Rename("existing.pdf".into()),
                &mut Recorder::default(),
            )
            .unwrap_err();
        assert!(failure.os_code.is_some());
        assert_eq!(std::fs::read(&original).unwrap(), b"source");
        assert_eq!(std::fs::read(&destination).unwrap(), b"destination");
        for action in [
            WaitFollowUp::Rename("changed.pdf".into()),
            WaitFollowUp::Move(root.path().into()),
            WaitFollowUp::Delete,
            WaitFollowUp::Reveal,
        ] {
            let service = completed_file_wait(&original);
            let error = service
                .wait_follow_up_with_backend(
                    1,
                    action,
                    &mut HeldFixture {
                        scans: Arc::new(AtomicUsize::new(0)),
                    },
                )
                .unwrap_err();
            assert_eq!(error.kind, "wait_target_held");
            assert_eq!(std::fs::read(&original).unwrap(), b"source");
        }
        for name in ["../escape.pdf", "NUL.pdf", "bad?.pdf"] {
            let service = completed_file_wait(&original);
            assert!(
                service
                    .wait_follow_up_with_backend(
                        1,
                        WaitFollowUp::Rename(name.into()),
                        &mut Recorder::default()
                    )
                    .is_err()
            );
            assert_eq!(std::fs::read(&original).unwrap(), b"source");
        }
    }
    #[test]
    fn cancelled_follow_up_cannot_change_files_or_overwrite_a_new_wait_outcome() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("source.pdf");
        let next = root.path().join("next.pdf");
        std::fs::write(&original, b"source").unwrap();
        std::fs::write(&next, b"next").unwrap();
        let service = Arc::new(completed_file_wait(&original));
        let (started_sender, started) = channel();
        let (release, release_receiver) = channel();
        let worker_service = service.clone();
        let worker = std::thread::spawn(move || {
            worker_service.wait_follow_up_with_backend(
                1,
                WaitFollowUp::Rename("changed.pdf".into()),
                &mut Gated {
                    started: started_sender,
                    release: release_receiver,
                },
            )
        });
        started.recv_timeout(TIMEOUT).unwrap();
        service.cancel_wait();
        let newer = service.wait_until_free(WaitRequest {
            path: Some(next),
            include_ports: false,
            port: None,
            timeout: Some(Duration::from_secs(5)),
        });
        assert_ne!(newer.wait.id, 1);
        release.send(()).unwrap();
        assert_eq!(worker.join().unwrap().unwrap_err().kind, "cancelled");
        assert_eq!(service.status().wait.id, newer.wait.id);
        assert!(service.status().wait.error.is_none());
        assert_eq!(std::fs::read(&original).unwrap(), b"source");
        assert!(!root.path().join("changed.pdf").exists());
    }
}
