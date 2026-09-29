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
struct PendingAction {
    ticket: String,
    identities: Vec<Identity>,
    force: bool,
}
struct State {
    generation: u32,
    dataset: Arc<Dataset>,
    pending: Option<ScanJob>,
    cancellation: Cancellation,
    scanning: bool,
    shutdown: bool,
    target: Option<PathBuf>,
    recent: Vec<(u32, PathBuf)>,
    error: Option<Failure>,
    action: Option<PendingAction>,
    next_ticket: u64,
    ancestry: Option<(String, Vec<Ancestor>)>,
}
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
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
        mut recent_targets: RecentTargets,
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
                shutdown: false,
                target: None,
                recent,
                error: None,
                action: None,
                next_ticket: 0,
                ancestry: None,
            }),
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
                        state.scanning = false;
                        match result {
                            Ok((path, dataset)) => {
                                state.error = None;
                                if let Some(path) = path {
                                    state.recent.retain(|(_, previous)| *previous != path);
                                    state.recent.insert(0, (job.generation, path.clone()));
                                    state.recent.truncate(12);
                                    state.target = Some(path.clone());
                                    if let Err(error) = recent_targets.record(&path) {
                                        state.error = Some(Failure {
                                            kind: "desktop_storage".into(),
                                            message: oflh_core::safe(&format!(
                                                "Could not save recent targets: {error}"
                                            )),
                                            os_code: None,
                                        });
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
    /// Replace queued scans, cancel active work, and preserve the displayed snapshot.
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
        state.scanning = true;
        state.error = None;
        self.shared.ready.notify_one();
        Ok(status(&state))
    }
    /// Rescan the last successful native target without a display-string round trip.
    pub fn refresh(&self) -> Result<Status, Failure> {
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
        let mut state = self.shared.lock();
        state.next_ticket = state
            .next_ticket
            .checked_add(1)
            .ok_or_else(|| Failure::invalid("Restart the application to perform another action"))?;
        let ticket = state.next_ticket.to_string();
        state.action = Some(PendingAction {
            ticket: ticket.clone(),
            identities,
            force,
        });
        Ok(Confirmation {
            ticket,
            force,
            targets,
        })
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
        let action = {
            let mut state = self.shared.lock();
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
                .ok_or_else(|| Failure::invalid("Confirmation expired"))?
        };
        let cancel = Cancellation::default();
        let mut results: Vec<_> = action
            .identities
            .iter()
            .map(|&identity| {
                let error = identity
                    .validate()
                    .and_then(|()| backend.terminate(identity, action.force, &cancel))
                    .err()
                    .map(Failure::from);
                ActionResult {
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
        // One shared deadline bounds multi-selection latency. Never signal again or
        // infer exit from a missing file/port row: verify the captured birth identity.
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
        loop {
            for (result, &identity) in results.iter_mut().zip(&action.identities) {
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
    snapshot.normalize();
    let dataset = Dataset::new(job.generation, snapshot);
    job.cancel.check().map_err(Failure::from)?;
    Ok((path, dataset))
}

fn status(state: &State) -> Status {
    Status {
        generation: state.generation,
        revision: state.dataset.revision,
        scanning: state.scanning,
        target: state.target.as_deref().map(display).unwrap_or_default(),
        processes: state
            .dataset
            .snapshot
            .processes
            .iter()
            .filter(|process| !process.usages.is_empty())
            .count(),
        ports: state
            .dataset
            .snapshot
            .processes
            .iter()
            .map(|process| process.ports.len())
            .sum(),
        usages: state
            .dataset
            .snapshot
            .processes
            .iter()
            .map(|process| process.usages.len())
            .sum(),
        warnings: state
            .dataset
            .snapshot
            .warnings
            .iter()
            .map(|warning| oflh_core::safe(warning))
            .collect(),
        error: state.error.clone(),
        version: oflh_core::VERSION,
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
    fn pending_scans_coalesce_and_late_results_never_publish() {
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
        service.inspect(base.join("oflh-desktop-first")).unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        service.inspect(base.join("oflh-desktop-obsolete")).unwrap();
        service.inspect(base.join("oflh-desktop-final")).unwrap();
        release.send(()).unwrap();
        assert!(
            started
                .recv_timeout(TIMEOUT)
                .unwrap()
                .ends_with("oflh-desktop-final")
        );
        release.send(()).unwrap();
        let status = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(status.revision, 3);
        assert!(!status.scanning);
        assert!(received.try_recv().is_err());
        assert_eq!(service.recent().len(), 1);
        service
            .inspect(base.join("oflh-desktop-cancelled"))
            .unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        let status = service.cancel();
        assert_eq!(status.revision, 3);
        assert!(!status.scanning);
        release.send(()).unwrap();
        service.inspect(base.join("oflh-desktop-next")).unwrap();
        started.recv_timeout(TIMEOUT).unwrap();
        release.send(()).unwrap();
        let status = received.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(status.revision, 6);
        assert_eq!(service.recent().len(), 2);
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
