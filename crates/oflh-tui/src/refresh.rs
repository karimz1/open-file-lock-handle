//! An automatic refresh interval starts at completion, never during active work.
use std::time::{Duration, Instant};
pub struct RefreshSchedule {
    interval: Duration,
    deadline: Option<Instant>,
}
impl RefreshSchedule {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            deadline: None,
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn started(&mut self) {
        self.deadline = None;
    }
    pub fn completed(&mut self, now: Instant) {
        self.deadline = Some(now + self.interval);
    }
    pub fn due(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|at| now >= at)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_scans_and_cancelled_work_receive_a_full_interval_after_completion() {
        let now = Instant::now();
        let mut refresh = RefreshSchedule::new(Duration::from_secs(5));
        assert!(!refresh.due(now + Duration::from_secs(90)));
        refresh.completed(now);
        assert!(!refresh.due(now + Duration::from_millis(4999)));
        assert!(refresh.due(now + Duration::from_secs(5)));
        refresh.started();
        assert_eq!(refresh.deadline(), None);
        assert!(!refresh.due(now + Duration::from_secs(90)));
        refresh.completed(now + Duration::from_secs(91));
        assert!(!refresh.due(now + Duration::from_millis(95999)));
        assert!(refresh.due(now + Duration::from_secs(96)));
    }
}
