//! Bounded independent native-process work. Join all workers before publication.
use oflh_core::{Cancellation, Error, InspectionCounter, Result, io};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Share a CPU-sized native-work limit across independent scan instances. The
/// counter lock is released before callbacks; waiting remains cancellable.
pub(crate) struct WorkBudget {
    maximum: usize,
    available: Mutex<usize>,
    changed: Condvar,
}
impl WorkBudget {
    pub(crate) fn new(maximum: usize) -> Self {
        let maximum = maximum.clamp(1, 8);
        Self {
            maximum,
            available: Mutex::new(maximum),
            changed: Condvar::new(),
        }
    }
    pub(crate) fn maximum(&self) -> usize {
        self.maximum
    }
    fn acquire(&self, cancel: &Cancellation) -> Result<WorkPermit<'_>> {
        cancel.check()?;
        let mut available = self
            .available
            .lock()
            .map_err(|_| Error::Unavailable("native process work budget poisoned".into()))?;
        loop {
            cancel.check()?;
            if *available > 0 {
                *available -= 1;
                return Ok(WorkPermit(self));
            }
            (available, _) = self
                .changed
                .wait_timeout(available, Duration::from_millis(25))
                .map_err(|_| Error::Unavailable("native process work budget poisoned".into()))?;
        }
    }
    pub(crate) fn run<T>(
        &self,
        cancel: &Cancellation,
        work: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let _permit = self.acquire(cancel)?;
        work()
    }
}
struct WorkPermit<'a>(&'a WorkBudget);
impl Drop for WorkPermit<'_> {
    fn drop(&mut self) {
        // No callback runs under this lock. Recovery during unwinding releases
        // this uniquely owned permit without causing a second panic.
        let mut available = self
            .0
            .available
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *available += 1;
        self.0.changed.notify_one();
    }
}

struct StopOnPanic<'a>(&'a AtomicBool);
impl Drop for StopOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(true, Ordering::Relaxed);
        }
    }
}

/// Dispatch each captured PID once. State stays worker-local, and all workers
/// finish before any state can reach the caller, including on failure or panic.
pub(crate) fn collect<State: Default + Send>(
    processes: &[u32],
    workers: usize,
    cancel: &Cancellation,
    inspect: impl Fn(&mut State, u32) -> Result<()> + Sync,
) -> Result<Vec<State>> {
    cancel.check()?;
    if processes.is_empty() {
        return Ok(Vec::new());
    }
    let workers = workers.clamp(1, 8).min(processes.len());
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let mut threads = Vec::with_capacity(workers);
        let mut failure = None;
        for index in 0..workers {
            let inspect = &inspect;
            let next = &next;
            let stop = &stop;
            let thread = std::thread::Builder::new()
                .name(format!("oflh-process-{index}"))
                .spawn_scoped(scope, move || {
                    let _guard = StopOnPanic(stop);
                    let mut state = State::default();
                    while !stop.load(Ordering::Relaxed) {
                        cancel.check()?;
                        let Some(&pid) = processes.get(next.fetch_add(1, Ordering::Relaxed)) else {
                            break;
                        };
                        if let Err(error) = inspect(&mut state, pid) {
                            stop.store(true, Ordering::Relaxed);
                            return Err(error);
                        }
                    }
                    Ok(state)
                });
            match thread {
                Ok(thread) => {
                    cancel.record(InspectionCounter::ProcessWorkers, 1);
                    threads.push(thread);
                }
                Err(error) => {
                    stop.store(true, Ordering::Relaxed);
                    failure = Some(io("start process inspection worker", error));
                    break;
                }
            }
        }
        let mut states = Vec::with_capacity(threads.len());
        for thread in threads {
            match thread.join() {
                Ok(Ok(state)) => states.push(state),
                Ok(Err(error)) => {
                    if failure.is_none() {
                        failure = Some(error);
                    }
                }
                Err(_) => {
                    if failure.is_none() {
                        failure = Some(Error::Unavailable(
                            "native process inspection worker panicked".into(),
                        ));
                    }
                }
            }
        }
        cancel.check()?;
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(states)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeSet,
        sync::{Arc, Condvar, Mutex},
        time::Duration,
    };
    /// A fixture must fail clearly if a peer cannot start, rather than hanging
    /// CI forever while the pool is correctly joining a failed dispatch.
    fn meet_peer(gate: &(Mutex<usize>, Condvar)) {
        let (entered, changed) = gate;
        let mut entered = entered.lock().unwrap();
        *entered += 1;
        changed.notify_all();
        let (entered, timeout) = changed
            .wait_timeout_while(entered, Duration::from_secs(5), |entered| *entered < 2)
            .unwrap();
        assert!(
            !timeout.timed_out() || *entered >= 2,
            "native fixture peer never started"
        );
    }
    #[test]
    fn native_work_overlaps_with_a_fixed_bound_and_no_lost_processes() {
        let processes: Vec<_> = (0..512).collect();
        let gate = (Mutex::new(0), Condvar::new());
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let cancel = Cancellation::default();
        let states = collect(&processes, 2, &cancel, |state: &mut Vec<u32>, pid| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            if pid < 2 {
                meet_peer(&gate);
            }
            state.push(pid);
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        let all: Vec<_> = states.into_iter().flatten().collect();
        assert_eq!(all.len(), 512);
        assert_eq!(
            all.into_iter().collect::<BTreeSet<_>>(),
            processes.into_iter().collect()
        );
        assert_eq!(cancel.progress().process_workers, 2);
    }
    #[test]
    fn empty_dispatch_starts_nothing_and_worker_requests_are_bounded() {
        let empty = Cancellation::default();
        let states = collect::<Vec<u32>>(&[], usize::MAX, &empty, |_, _| {
            panic!("empty dispatch callback")
        })
        .unwrap();
        assert!(states.is_empty());
        assert_eq!(empty.progress().process_workers, 0);
        for (requested, expected) in [(0, 1), (1, 1), (usize::MAX, 8)] {
            let cancel = Cancellation::default();
            let processes: Vec<_> = (1..=32).collect();
            let states = collect(
                &processes,
                requested,
                &cancel,
                |state: &mut Vec<u32>, pid| {
                    state.push(pid);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(cancel.progress().process_workers, expected);
            let collected: BTreeSet<_> = states.into_iter().flatten().collect();
            assert_eq!(collected, processes.into_iter().collect());
        }
    }
    #[test]
    fn cancellation_and_typed_failure_finish_every_running_callback() {
        for cancelled in [false, true] {
            let cancel = Cancellation::default();
            let running = Arc::new(AtomicUsize::new(0));
            let gate = (Mutex::new(0), Condvar::new());
            let result = collect(&[0, 1, 2, 3], 2, &cancel, |_: &mut Vec<u32>, pid| {
                running.fetch_add(1, Ordering::SeqCst);
                if pid < 2 {
                    meet_peer(&gate);
                }
                if pid == 0 {
                    running.fetch_sub(1, Ordering::SeqCst);
                    if cancelled {
                        cancel.cancel();
                        return Err(Error::Cancelled);
                    }
                    return Err(io(
                        "native fixture operation",
                        std::io::Error::from_raw_os_error(5),
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
                running.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            });
            assert_eq!(running.load(Ordering::SeqCst), 0);
            if cancelled {
                assert!(matches!(result, Err(Error::Cancelled)));
            } else {
                assert!(
                    matches!(result, Err(Error::Io { source, .. }) if source.raw_os_error()==Some(5))
                );
            }
        }
    }
    #[test]
    fn panic_is_an_error_after_other_callbacks_finish_and_precancelled_work_never_starts() {
        let finished = AtomicUsize::new(0);
        let gate = (Mutex::new(0), Condvar::new());
        let result = collect(
            &[0, 1],
            2,
            &Cancellation::default(),
            |_: &mut Vec<u32>, pid| {
                meet_peer(&gate);
                if pid == 0 {
                    panic!("native fixture worker panic");
                }
                std::thread::sleep(Duration::from_millis(10));
                finished.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        );
        assert!(matches!(result, Err(Error::Unavailable(_))));
        assert_eq!(finished.load(Ordering::SeqCst), 1);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            collect(&[1], 8, &cancel, |_: &mut Vec<u32>, _| panic!(
                "precancelled callback"
            )),
            Err(Error::Cancelled)
        ));
        assert_eq!(cancel.progress().process_workers, 0);
    }

    #[test]
    fn independent_scans_share_a_work_ceiling_without_losing_processes() {
        let budget = WorkBudget::new(2);
        let processes: Vec<_> = (0..32).collect();
        let gate = (Mutex::new(0), Condvar::new());
        let entered = AtomicUsize::new(0);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let inspect = || {
            let cancel = Cancellation::default();
            let states = collect(&processes, 8, &cancel, |state: &mut Vec<u32>, pid| {
                budget.run(&cancel, || {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(current, Ordering::SeqCst);
                    if entered.fetch_add(1, Ordering::SeqCst) < 2 {
                        meet_peer(&gate);
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    state.push(pid);
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(())
                })
            })
            .unwrap();
            assert_eq!(cancel.progress().process_workers, 8);
            let collected: Vec<_> = states.into_iter().flatten().collect();
            assert_eq!(collected.len(), processes.len());
            assert_eq!(
                collected.into_iter().collect::<BTreeSet<_>>(),
                processes.iter().copied().collect()
            );
        };
        std::thread::scope(|scope| {
            scope.spawn(inspect);
            scope.spawn(inspect);
        });
        assert_eq!(peak.load(Ordering::SeqCst), budget.maximum());
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(entered.load(Ordering::SeqCst), 64);
    }

    #[test]
    fn waiting_for_native_work_is_cancellable_without_releasing_a_foreign_permit() {
        let budget = WorkBudget::new(1);
        let held = budget.acquire(&Cancellation::default()).unwrap();
        let cancel = Cancellation::default();
        let (started, received) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                started.send(()).unwrap();
                budget.run::<()>(&cancel, || panic!("cancelled budget callback"))
            });
            received.recv_timeout(Duration::from_secs(2)).unwrap();
            cancel.cancel();
            assert!(matches!(waiter.join().unwrap(), Err(Error::Cancelled)));
        });
        assert_eq!(*budget.available.lock().unwrap(), 0);
        drop(held);
        assert_eq!(*budget.available.lock().unwrap(), 1);
        assert_eq!(WorkBudget::new(0).maximum(), 1);
        assert_eq!(WorkBudget::new(usize::MAX).maximum(), 8);
    }

    #[test]
    fn native_error_and_callback_panic_release_the_shared_work_budget() {
        let budget = WorkBudget::new(1);
        let cancel = Cancellation::default();
        let result = budget.run(&cancel, || {
            Err::<(), _>(io(
                "native budget fixture",
                std::io::Error::from_raw_os_error(5),
            ))
        });
        assert!(matches!(result, Err(Error::Io {source, ..}) if source.raw_os_error()==Some(5)));
        let panic = std::panic::catch_unwind(|| {
            budget.run::<()>(&cancel, || panic!("native budget fixture panic"))
        });
        assert!(panic.is_err());
        assert_eq!(budget.run(&cancel, || Ok(42)).unwrap(), 42);
        assert_eq!(*budget.available.lock().unwrap(), 1);
    }
}
