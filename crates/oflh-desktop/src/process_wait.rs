//! Cancellable, read-only waiting for one captured process lifetime.
use oflh_core::{
    Cancellation, Identity, Result,
    wait::{Options, Sleeper},
};
use oflh_platform::Backend;
use std::time::Instant;

/// Reuse native birth-identity checks; permission failures never mean the process exited.
pub(crate) fn wait_for_exit(
    backend: &mut dyn Backend,
    identity: Identity,
    options: Options,
    started: Instant,
    cancel: &Cancellation,
    sleeper: &dyn Sleeper,
) -> Result<()> {
    loop {
        cancel.check()?;
        options.check(started.elapsed())?;
        let running = backend.is_running(identity)?;
        cancel.check()?;
        options.check(started.elapsed())?;
        if !running {
            return Ok(());
        }
        sleeper.sleep(options.poll_interval, cancel)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Error, Metrics, Snapshot, Target};
    use std::{collections::VecDeque, time::Duration};
    struct Probe {
        responses: VecDeque<Result<bool>>,
        seen: Vec<Identity>,
    }
    impl Backend for Probe {
        fn is_running(&mut self, identity: Identity) -> Result<bool> {
            self.seen.push(identity);
            self.responses.pop_front().unwrap()
        }
        fn scan(&mut self, _: &Target, _: &Cancellation) -> Result<Snapshot> {
            panic!("process waiting must not scan a filesystem target")
        }
        fn sample(&mut self, _: &[Identity], _: &Cancellation) -> Result<Vec<(Identity, Metrics)>> {
            unreachable!()
        }
        fn terminate(&mut self, _: Identity, _: bool, _: &Cancellation) -> Result<()> {
            panic!("waiting must not stop a process")
        }
    }
    fn identity() -> Identity {
        Identity {
            pid: 424242,
            started: u64::MAX,
            started_sub: 21,
        }
    }
    #[test]
    fn waits_only_for_the_complete_captured_birth_identity_without_scanning_or_terminating() {
        let mut probe = Probe {
            responses: [Ok(true), Ok(true), Ok(false)].into(),
            seen: vec![],
        };
        wait_for_exit(
            &mut probe,
            identity(),
            Options {
                poll_interval: Duration::ZERO,
                timeout: None,
            },
            Instant::now(),
            &Cancellation::default(),
            &oflh_core::wait::PollSleeper,
        )
        .unwrap();
        assert_eq!(probe.seen, [identity(); 3]);
    }
    #[test]
    fn permission_errors_deadlines_and_cancellation_are_not_process_exit() {
        let mut probe = Probe {
            responses: [Err(oflh_core::io(
                "verify process",
                std::io::Error::from_raw_os_error(5),
            ))]
            .into(),
            seen: vec![],
        };
        let result = wait_for_exit(
            &mut probe,
            identity(),
            Options::default(),
            Instant::now(),
            &Cancellation::default(),
            &oflh_core::wait::PollSleeper,
        );
        assert!(
            matches!(result, Err(Error::Io { source, .. }) if source.raw_os_error() == Some(5))
        );
        probe.seen.clear();
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            wait_for_exit(
                &mut probe,
                identity(),
                Options::default(),
                Instant::now(),
                &cancel,
                &oflh_core::wait::PollSleeper
            ),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            wait_for_exit(
                &mut probe,
                identity(),
                Options {
                    timeout: Some(Duration::ZERO),
                    ..Default::default()
                },
                Instant::now(),
                &Cancellation::default(),
                &oflh_core::wait::PollSleeper
            ),
            Err(Error::WaitTimedOut)
        ));
        assert!(probe.seen.is_empty());
    }
}
