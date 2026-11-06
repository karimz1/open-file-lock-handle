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
    include_ports: bool,
    port: Option<u16>,
    cancel: &Cancellation,
) -> Result<()> {
    let snapshot = match path {
        Some(target) if include_ports => crate::scan_with_ports(backend, target, cancel)?,
        Some(target) => backend.scan(target, cancel)?,
        None => crate::scan_ports(cancel)?,
    };
    if is_free(&snapshot, port.map_or(CheckTarget::Path, CheckTarget::Port)) {
        Ok(())
    } else {
        Err(Error::WaitTargetHeld)
    }
}
