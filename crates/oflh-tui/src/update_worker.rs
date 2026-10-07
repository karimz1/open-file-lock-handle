//! Independent bounded release checks; inspection never waits for networking.
use crate::Event;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};
const INTERVAL: Duration = Duration::from_secs(3600);
pub struct UpdateSchedule {
    deadline: Option<Instant>,
}
impl UpdateSchedule {
    pub fn new(enabled: bool, now: Instant) -> Self {
        Self {
            deadline: enabled.then_some(now),
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn due(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|at| now >= at)
    }
    pub fn started(&mut self) {
        self.deadline = None;
    }
    pub fn completed(&mut self, now: Instant) {
        self.deadline = Some(now + INTERVAL);
    }
}
pub struct UpdateWorker {
    requests: SyncSender<()>,
    busy: bool,
    stopped: Arc<AtomicBool>,
}
impl UpdateWorker {
    pub fn new(events: SyncSender<Event>, installed: String) -> std::io::Result<Self> {
        Self::with_checker(events, move || {
            oflh_platform::updates::check_stable_release(&installed)
                .map_err(|error| error.to_string())
        })
    }
    fn with_checker(
        events: SyncSender<Event>,
        mut check: impl FnMut() -> Result<Option<String>, String> + Send + 'static,
    ) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel(1);
        let busy = false;
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = stopped.clone();
        std::thread::Builder::new()
            .name("oflh-update-check".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    if worker_stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let result = check();
                    if worker_stopped.load(Ordering::Acquire)
                        || events.send(Event::Update(result)).is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            busy,
            stopped,
        })
    }
    pub fn completed(&mut self) {
        self.busy = false;
    }
    /// Never queue another request while a check or its result delivery is active.
    pub fn request(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.busy = true;
        if self.requests.try_send(()).is_err() {
            self.busy = false;
            return false;
        }
        true
    }
}
impl Drop for UpdateWorker {
    fn drop(&mut self) {
        // Closing requests wakes an idle worker. An in-flight HTTP request has
        // its own total timeout; quitting never joins or blocks the UI thread.
        self.stopped.store(true, Ordering::Release);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedule_has_no_busy_deadline_and_waits_a_full_hour_after_completion() {
        let now = Instant::now();
        let mut schedule = UpdateSchedule::new(true, now);
        assert!(schedule.due(now));
        schedule.started();
        assert_eq!(schedule.deadline(), None);
        assert!(!schedule.due(now + Duration::from_secs(7200)));
        schedule.completed(now);
        assert!(!schedule.due(now + Duration::from_secs(3599)));
        assert!(schedule.due(now + INTERVAL));
        assert_eq!(UpdateSchedule::new(false, now).deadline(), None);
    }
    #[test]
    fn slow_checks_coalesce_without_blocking_inspection_events() {
        let (events, receiver) = mpsc::sync_channel(2);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut worker = UpdateWorker::with_checker(events.clone(), move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(Some("1.2.0".into()))
        })
        .unwrap();
        assert!(worker.request());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..1000 {
            assert!(!worker.request());
        }
        events.send(Event::Killed(0, vec![])).unwrap();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::Killed(0, _)
        ));
        release_tx.send(()).unwrap();
        assert!(
            matches!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(),Event::Update(Ok(Some(version))) if version=="1.2.0")
        );
        drop(worker);
        assert!(entered_rx.recv_timeout(Duration::from_secs(2)).is_err());
    }
    #[test]
    fn shutdown_discards_inflight_result_without_waiting_for_network() {
        let (events, receiver) = mpsc::sync_channel(1);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut worker = UpdateWorker::with_checker(events, move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Err("offline".into())
        })
        .unwrap();
        assert!(worker.request());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(worker);
        release_tx.send(()).unwrap();
        assert!(receiver.recv_timeout(Duration::from_secs(2)).is_err());
    }
}
