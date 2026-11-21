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
    /// Any local port binding (`--ports` without a path or port).
    AnyPort,
    /// Any local port binding on a process that also references the current path.
    AnyPortOnPathProcesses,
}

impl CheckTarget {
    /// The single rule shared by `check`, `wait`, and the terminal app: a port
    /// inspection with a path only counts processes that also use that path.
    pub fn select(path: bool, ports: bool, port: Option<u16>) -> Self {
        match (port, ports, path) {
            (Some(number), _, true) => Self::PortOnPathProcesses(number),
            (Some(number), _, false) => Self::Port(number),
            (None, true, true) => Self::AnyPortOnPathProcesses,
            (None, true, false) => Self::AnyPort,
            (None, false, _) => Self::Path,
        }
    }
    /// Whether ports must be collected for this classification.
    pub fn needs_ports(self) -> bool {
        !matches!(self, Self::Path)
    }
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
        CheckTarget::AnyPort => snapshot
            .processes
            .iter()
            .all(|process| process.ports.is_empty()),
        CheckTarget::AnyPortOnPathProcesses => snapshot
            .processes
            .iter()
            .all(|process| process.usages.is_empty() || process.ports.is_empty()),
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
    fn path_scoped_port_rules_ignore_unrelated_bindings() {
        let binding = crate::Port {
            protocol: crate::Protocol::Tcp,
            address: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            number: 3000,
        };
        let unrelated = Snapshot {
            processes: vec![Process {
                ports: vec![binding.clone()],
                ..Process::default()
            }],
            warnings: vec![],
        };
        assert!(is_free(&unrelated, CheckTarget::select(true, true, None)));
        assert!(is_free(
            &unrelated,
            CheckTarget::select(true, true, Some(3000))
        ));
        assert!(!is_free(&unrelated, CheckTarget::select(false, true, None)));
        assert!(!is_free(
            &unrelated,
            CheckTarget::select(false, true, Some(3000))
        ));
        let related = Snapshot {
            processes: vec![Process {
                usages: vec![Usage::default()],
                ports: vec![binding],
                ..Process::default()
            }],
            warnings: vec![],
        };
        assert!(!is_free(&related, CheckTarget::select(true, true, None)));
        assert!(!is_free(
            &related,
            CheckTarget::select(true, false, Some(3000))
        ));
        assert!(matches!(
            CheckTarget::select(true, false, None),
            CheckTarget::Path
        ));
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
