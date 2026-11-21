//! Fresh-scan guard shared by destructive desktop follow-ups.
use crate::Backend;
use oflh_core::{
    Cancellation, Error, Result, Target,
    check::{CheckTarget, is_free},
};

/// Refuse every follow-up if any matching holder reappeared after the wait.
pub fn confirm_free_before_action(
    backend: &mut dyn Backend,
    path: Option<&Target>,
    check: CheckTarget,
    cancel: &Cancellation,
) -> Result<()> {
    let snapshot = match path {
        Some(target) if check.needs_ports() => crate::scan_with_ports(backend, target, cancel)?,
        Some(target) => backend.scan(target, cancel)?,
        None => crate::scan_ports(cancel)?,
    };
    if is_free(&snapshot, check) {
        Ok(())
    } else {
        Err(Error::WaitTargetHeld)
    }
}
