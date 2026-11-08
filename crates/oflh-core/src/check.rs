//! Shared free/held classification for one-shot and waiting inspection frontends.
use crate::Snapshot;

/// Inspection target used to decide whether matching holders remain.
#[derive(Clone, Copy, Debug)]
pub enum CheckTarget {
    /// A canonical file or folder target. Backends return matching usages only.
    Path,
    /// One local TCP listener or UDP binding number.
    Port(u16),
    /// A local port binding on a process that also references the current path.
    PortOnPathProcesses(u16),
}

/// Classify a completed snapshot without interpreting scan warnings as holders.
pub fn is_free(snapshot: &Snapshot, target: CheckTarget) -> bool {
    match target {
        CheckTarget::Path => snapshot
            .processes
            .iter()
            .all(|process| process.usages.is_empty()),
        CheckTarget::PortOnPathProcesses(number) => snapshot.processes.iter().all(|process| {
            process.usages.is_empty() || process.ports.iter().all(|port| port.number != number)
        }),
        CheckTarget::Port(number) => snapshot
            .processes
            .iter()
            .all(|process| process.ports.iter().all(|port| port.number != number)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Port, Process, Usage};

    #[test]
    fn path_and_port_classification_share_holder_semantics() {
        let mut snapshot = Snapshot::default();
        assert!(is_free(&snapshot, CheckTarget::Path));
        assert!(is_free(&snapshot, CheckTarget::Port(3000)));

        snapshot.processes.push(Process {
            usages: vec![Usage::default()],
            ports: vec![Port {
                protocol: crate::Protocol::Tcp,
                address: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                number: 3000,
            }],
            ..Process::default()
        });
        assert!(!is_free(&snapshot, CheckTarget::Path));
        assert!(!is_free(&snapshot, CheckTarget::Port(3000)));
        assert!(is_free(&snapshot, CheckTarget::Port(3001)));
    }

    #[test]
    fn warnings_alone_do_not_make_a_target_held() {
        let snapshot = Snapshot {
            processes: vec![],
            warnings: vec!["partial inspection".into()],
        };
        assert!(is_free(&snapshot, CheckTarget::Path));
        assert!(is_free(&snapshot, CheckTarget::Port(3000)));
    }
}
