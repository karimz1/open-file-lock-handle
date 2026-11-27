//! Shared settings and deadline rules for bounded, non-destructive waiting.
use crate::{Error, Result};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

/// Cancellable sleeper for tests and platform event-driven wakeups.
pub trait Sleeper: Send {
    /// Sleep for the requested duration, or wake early on cancellation.
    fn sleep(&self, duration: Duration, cancel: &crate::Cancellation) -> Result<()>;
}
/// Cooperative polling sleeper; at most 50 ms passes between cancellation checks.
pub struct PollSleeper;
impl Sleeper for PollSleeper {
    fn sleep(&self, duration: Duration, cancel: &crate::Cancellation) -> Result<()> {
        let deadline = Instant::now() + duration;
        loop {
            cancel.check()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            std::thread::sleep(
                deadline
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(50)),
            );
        }
    }
}

/// Polling and optional deadline shared by all waiting frontends.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Time between completed scans and the next scan request.
    pub poll_interval: Duration,
    /// Maximum total wait time, including scans. `None` waits until free or cancellation.
    pub timeout: Option<Duration>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            timeout: None,
        }
    }
}
impl Options {
    /// Check cooperative cancellation before checking whether time has expired.
    pub fn check(&self, elapsed: Duration) -> Result<()> {
        if self.timeout.is_some_and(|timeout| elapsed >= timeout) {
            Err(Error::WaitTimedOut)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_is_distinct_and_disabled_deadlines_never_expire() {
        let options = Options {
            poll_interval: Duration::from_millis(50),
            timeout: Some(Duration::from_secs(2)),
        };
        assert!(options.check(Duration::from_millis(1999)).is_ok());
        assert!(matches!(
            options.check(Duration::from_secs(2)),
            Err(Error::WaitTimedOut)
        ));
        assert!(Options::default().check(Duration::from_secs(86400)).is_ok());
    }
}

/// Notification wakeups preserve once-only delivery.
pub struct NotifySleeper {
    state: Mutex<bool>,
    ready: Condvar,
}
impl NotifySleeper {
    /// Create an unsignalled sleeper.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(false),
            ready: Condvar::new(),
        })
    }
    /// Wake a current or future sleep once.
    pub fn notify(&self) {
        *self.state.lock().unwrap_or_else(|error| error.into_inner()) = true;
        self.ready.notify_all();
    }
}
impl Sleeper for NotifySleeper {
    fn sleep(&self, duration: Duration, cancel: &crate::Cancellation) -> Result<()> {
        let deadline = Instant::now() + duration;
        let mut notified = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while !*notified {
            cancel.check()?;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(());
            }
            let (next, _) = self
                .ready
                .wait_timeout(notified, remaining.min(Duration::from_millis(50)))
                .unwrap_or_else(|error| error.into_inner());
            notified = next;
        }
        *notified = false;
        Ok(())
    }
}
#[cfg(test)]
mod sleeper_tests {
    use super::*;
    #[test]
    fn a_notification_wakes_the_sleep_before_the_interval() {
        let sleeper = NotifySleeper::new();
        let worker = sleeper.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            worker.notify();
        });
        let started = Instant::now();
        Sleeper::sleep(
            sleeper.as_ref(),
            Duration::from_secs(5),
            &crate::Cancellation::default(),
        )
        .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
