//! Bounded independent native-process work. Join all workers before publication.
use oflh_core::{Cancellation, Error, InspectionCounter, Result, io};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
        sync::{Arc, Barrier},
        time::Duration,
    };
    #[test]
    fn native_work_overlaps_with_a_fixed_bound_and_no_lost_processes() {
        let processes: Vec<_> = (0..512).collect();
        let barrier = Barrier::new(2);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let cancel = Cancellation::default();
        let states = collect(&processes, 2, &cancel, |state: &mut Vec<u32>, pid| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            if pid < 2 {
                barrier.wait();
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
            let barrier = Barrier::new(2);
            let result = collect(&[0, 1, 2, 3], 2, &cancel, |_: &mut Vec<u32>, pid| {
                running.fetch_add(1, Ordering::SeqCst);
                if pid < 2 {
                    barrier.wait();
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
        let barrier = Barrier::new(2);
        let result = collect(
            &[0, 1],
            2,
            &Cancellation::default(),
            |_: &mut Vec<u32>, pid| {
                barrier.wait();
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
}
