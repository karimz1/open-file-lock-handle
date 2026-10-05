//! Bounded scan scheduling, immutable publication and captured destructive actions.
use crate::{
    contract::*,
    dataset::{Dataset, display},
    recent::RecentTargets,
};
use oflh_core::{Ancestor, Cancellation, Identity, Snapshot, Target};
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
struct State {
    generation: u32,
    dataset: Arc<Dataset>,
    pending: Option<ScanJob>,
    cancellation: Cancellation,
    scanning: bool,
    started: Option<std::time::Instant>,
    elapsed_ms: u64,
    shutdown: bool,
    target: Option<PathBuf>,
    recent: Vec<(u32, PathBuf)>,
    error: Option<Failure>,
    action: Option<PendingAction>,
    admin_recovery: Option<PendingAction>,
    next_ticket: u64,
    ancestry: Option<(String, Vec<Ancestor>)>,
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
                shutdown: false,
                target: None,
                recent,
                error: None,
                action: None,
                admin_recovery: None,
                next_ticket: 0,
                ancestry: None,
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
                        while state.pending.is_none() && !state.shutdown {
                            state = worker
                                .ready
                                .wait(state)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        if state.shutdown {
                            return;
                        }
                        state.pending.take()
                    };
                    let Some(job) = job else {
                        continue;
                    };
                    let result = scan(&mut *backend, &job);
                    let status = {
                        let mut state = worker.lock();
                        if state.generation != job.generation || state.shutdown {
                            continue;
                        }
                        state.elapsed_ms = elapsed_ms(&state);
                        state.scanning = false;
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
                        status(&state)
                    };
                    notify(status);
                }
            })?;
        Ok(Self { shared })
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
    fn enqueue(&self, path: Option<PathBuf>, owner: Option<Identity>) -> Result<Status, Failure> {
        let mut state = self.shared.lock();
        // Coalesce all entry points atomically: timer, keyboard, dialogs and drops
        // cannot cancel an inspection merely because its acknowledgment is delayed.
        if state.scanning {
            return Ok(status(&state));
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
        Ok(status(&state))
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
        state.cancellation.cancel();
        state.elapsed_ms = elapsed_ms(&state);
        state.pending = None;
        // Advance the generation so even a native backend that returns after cancellation
        // cannot publish over the last accepted snapshot.
        state.generation = state.generation.saturating_add(1);
        state.scanning = false;
        status(&state)
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
        self.confirm(
            vec![ancestor.identity],
            vec![ActionTarget {
                key: key.into(),
                name: oflh_core::safe(&ancestor.name),
                pid: ancestor.identity.pid,
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
    ) -> Result<Confirmation, Failure> {
        if keys.is_empty() || keys.len() > 10000 {
            return Err(Failure::invalid("Select between 1 and 10,000 processes"));
        }
        let dataset = self.dataset(revision)?;
        let mut seen = HashSet::new();
        let mut identities = Vec::new();
        let mut targets = Vec::new();
        for key in keys {
            let (_, process) = dataset.process(key)?;
            process.identity.validate().map_err(Failure::from)?;
            if seen.insert(process.identity) {
                identities.push(process.identity);
                targets.push(ActionTarget {
                    key: identity_key(process.identity),
                    name: oflh_core::safe(&process.name),
                    pid: process.identity.pid,
                });
            }
        }
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
        version: oflh_core::display_version(),
        commit: oflh_core::BUILD_COMMIT,
        build_url: oflh_core::BUILD_URL,
        pull_request_url: oflh_core::PULL_REQUEST_URL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Error, Metrics, Process, Result};
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::Duration;
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
        started.recv_timeout(TIMEOUT).unwrap();
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
        assert!(completed.target.ends_with("oflh-desktop-first"));
        assert_eq!(service.recent().len(), 1);
        let second = service
            .inspect(base.join("oflh-desktop-cancelled"))
            .unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
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
        assert!(cancelled.generation > second.generation);
        assert!(cancelled.elapsed_ms >= progress.elapsed_ms);
        assert_eq!(service.status().elapsed_ms, cancelled.elapsed_ms);
        let next = service.inspect(base.join("oflh-desktop-next")).unwrap();
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
        assert!(received.try_recv().is_err());
    }
    #[derive(Default)]
    struct Recorder {
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
        fn terminate(&mut self, identity: Identity, force: bool, _: &Cancellation) -> Result<()> {
            self.calls.push((identity, force));
            if self.changed {
                Err(Error::Changed)
            } else {
                Ok(())
            }
        }
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
    #[test]
    fn administrator_recovery_preserves_receipt_identity_mode_and_cancel() {
        for force in [false, true] {
            let (service, identity) = action_service();
            let confirmation = service
                .prepare(1, &[identity_key(identity)], force)
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
            let confirmation = service.prepare(1, &[identity_key(identity)], true).unwrap();
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
                .prepare(1, &[identity_key(identity)], false)
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
                .prepare_ancestor(&owner, "42000:13:0", true)
                .is_err()
        );
        assert!(
            service
                .prepare_ancestor("forged", &identity_key(parent), true)
                .is_err()
        );
        let confirmation = service
            .prepare_ancestor(&owner, &identity_key(parent), true)
            .unwrap();
        let mut backend = Recorder::default();
        service
            .terminate(&confirmation.ticket, &mut backend)
            .unwrap();
        assert_eq!(backend.calls, [(parent, true)]);
    }
    #[test]
    fn tickets_capture_mode_deduplicate_and_cannot_be_replayed() {
        let (service, identity) = action_service();
        let key = identity_key(identity);
        let confirmation = service.prepare(1, &[key.clone(), key], true).unwrap();
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
            .prepare(1, &[identity_key(identity)], false)
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
        assert!(service.prepare(1, &[], false).is_err());
        assert!(
            service
                .prepare(0, &[identity_key(identity)], false)
                .is_err()
        );
        assert!(service.prepare(1, &["1:1:0".into()], false).is_err());
        let confirmation = service
            .prepare(1, &[identity_key(identity)], false)
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
                .prepare(2, &["1:1:0".into()], false)
                .unwrap_err()
                .kind,
            "protected"
        );
    }
}
