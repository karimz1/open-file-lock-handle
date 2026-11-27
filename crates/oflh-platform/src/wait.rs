//! One-scanner polling with cancellation and a final confirming free scan.
/// Native target and port scope. All holders in supplied snapshots count.
#[derive(Clone, Copy)]
pub struct WaitTarget<'a> {
    /// File or folder target, absent for a global port inspection.
    pub path: Option<&'a Target>,
    /// Free/held rule; port rules also collect ports along with path results.
    pub check: CheckTarget,
}
use crate::Backend;
use oflh_core::{
    Cancellation, Result, Target,
    check::{CheckTarget, is_free},
    wait::{Options, PollSleeper, Sleeper},
};
use std::time::Instant;

/// Wait for all currently reported matching holders to disappear. No process or
/// resource is changed. A successful return is itself the final confirming scan.
pub fn wait_until_free(
    backend: &mut dyn Backend,
    target: WaitTarget,
    options: Options,
    started: Instant,
    cancel: &Cancellation,
) -> Result<()> {
    wait_with_sleeper(backend, target, options, started, cancel, &PollSleeper)
}

/// A final free scan gates every outcome; tests can supply deterministic sleep.
pub fn wait_with_sleeper(
    backend: &mut dyn Backend,
    target: WaitTarget,
    options: Options,
    started: Instant,
    cancel: &Cancellation,
    sleeper: &dyn Sleeper,
) -> Result<()> {
    loop {
        cancel.check()?;
        let snapshot = match target.path {
            Some(path) if target.check.needs_ports() => {
                crate::scan_with_ports(backend, path, cancel)?
            }
            Some(path) => backend.scan(path, cancel)?,
            None => crate::scan_ports(cancel)?,
        };
        cancel.check()?;
        if is_free(&snapshot, target.check) {
            return Ok(());
        }
        options.check(started.elapsed())?;
        sleeper.sleep(options.poll_interval, cancel)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Error, Identity, Process, Usage};
    use std::time::Duration;

    #[derive(Default)]
    struct Fixture {
        held: usize,
        scans: usize,
    }
    impl Backend for Fixture {
        fn scan(&mut self, _: &Target, _: &Cancellation) -> Result<oflh_core::Snapshot> {
            self.scans += 1;
            let mut snapshot = oflh_core::Snapshot::default();
            if self.scans <= self.held {
                snapshot.processes.push(Process {
                    usages: vec![Usage::default()],
                    ..Process::default()
                });
            }
            Ok(snapshot)
        }
        fn sample(
            &mut self,
            _: &[Identity],
            _: &Cancellation,
        ) -> Result<Vec<(Identity, oflh_core::Metrics)>> {
            unreachable!()
        }
        fn terminate(&mut self, _: Identity, _: bool, _: &Cancellation) -> Result<()> {
            unreachable!()
        }
    }

    #[test]
    fn waits_for_a_confirming_scan_without_terminating_the_holder() {
        let target = Target::new(std::env::temp_dir()).unwrap();
        let mut fixture = Fixture {
            held: 2,
            ..Fixture::default()
        };
        wait_until_free(
            &mut fixture,
            WaitTarget {
                path: Some(&target),
                check: CheckTarget::Path,
            },
            Options {
                poll_interval: Duration::ZERO,
                timeout: Some(Duration::from_secs(1)),
            },
            Instant::now(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(fixture.scans, 3);
    }

    #[test]
    fn cancellation_stops_a_wait_promptly() {
        let target = Target::new(std::env::temp_dir()).unwrap();
        let mut fixture = Fixture {
            held: usize::MAX,
            ..Fixture::default()
        };
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            wait_until_free(
                &mut fixture,
                WaitTarget {
                    path: Some(&target),
                    check: CheckTarget::Path,
                },
                Options::default(),
                Instant::now(),
                &cancel
            ),
            Err(Error::Cancelled)
        ));
        assert_eq!(fixture.scans, 0);
    }
}
