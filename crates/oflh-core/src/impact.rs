//! Best-effort side effects of terminating one process, shown before confirmation.
//!
//! Termination always targets a single process lifetime. These helpers only
//! describe what else that request may touch; they never widen the action.
use crate::{Ancestor, Identity, Port, Process, safe};

/// Entries shown per list before collapsing the rest into a count.
pub const IMPACT_LIST_LIMIT: usize = 5;

/// One row of a native process table read just before confirmation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessEntry {
    /// Captured lifetime; a zero birth token means it could not be read.
    pub identity: Identity,
    /// Parent PID as reported by the OS. Windows does not clear it when the
    /// parent exits, so it must be checked against the parent's birth time.
    pub parent: u32,
    /// Untrusted OS name, sanitized only when rendered.
    pub name: String,
}

/// Native observations about a process that the scan does not collect.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Impact {
    /// Titles of visible top-level windows owned by the process. `None` means
    /// this platform cannot list them, not that there are none.
    pub windows: Option<Vec<String>>,
    /// Direct children that are running now. `None` means the process table
    /// could not be read.
    pub children: Option<Vec<Ancestor>>,
}

/// A display list capped at [`IMPACT_LIST_LIMIT`], with the remainder counted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Capped {
    /// The first entries, in input order.
    pub items: Vec<String>,
    /// Entries left out of `items`.
    pub more: usize,
}

impl Capped {
    /// Keep the first [`IMPACT_LIST_LIMIT`] entries and count the rest.
    pub fn new(items: impl IntoIterator<Item = String>) -> Self {
        let mut capped = Self::default();
        for item in items {
            if capped.items.len() < IMPACT_LIST_LIMIT {
                capped.items.push(item)
            } else {
                capped.more += 1
            }
        }
        capped
    }
    /// Total number of entries before capping.
    pub fn total(&self) -> usize {
        self.items.len() + self.more
    }
}

fn born(identity: Identity) -> (u64, u64) {
    (identity.started, identity.started_sub)
}

/// Running direct children of `parent`, oldest first.
///
/// A recorded parent PID is only trusted when the child started no earlier
/// than the parent: on Windows a child keeps the PID of an exited parent, and
/// that PID can be reused by an unrelated, younger process. Children whose
/// birth time is unreadable are left out for the same reason.
pub fn children_of(table: &[ProcessEntry], parent: Identity) -> Vec<Ancestor> {
    let mut children: Vec<_> = table
        .iter()
        .filter(|entry| {
            entry.parent == parent.pid
                && entry.identity.pid != parent.pid
                && entry.identity.started != 0
                && born(entry.identity) >= born(parent)
        })
        .collect();
    children.sort_by_key(|entry| (born(entry.identity), entry.identity.pid));
    children
        .into_iter()
        .map(|entry| Ancestor {
            identity: entry.identity,
            name: entry.name.clone(),
        })
        .collect()
}

/// Short label for a local binding, such as `TCP 3000` or `UDP [::1]:53`.
pub fn port_label(port: &Port) -> String {
    if port.address.is_unspecified() {
        format!("{} {}", port.protocol.label(), port.number)
    } else if port.address.is_ipv6() {
        format!(
            "{} [{}]:{}",
            port.protocol.label(),
            port.address,
            port.number
        )
    } else {
        format!("{} {}:{}", port.protocol.label(), port.address, port.number)
    }
}

/// Files and ports the latest scan attributed to this process lifetime.
///
/// Files are limited to what matched the scanned target; the process may hold
/// others the scan did not look for.
pub fn scanned_resources(process: &Process) -> (Capped, Capped) {
    let mut files: Vec<String> = process
        .usages
        .iter()
        .map(|usage| safe(&crate::display_path(&usage.path)))
        .collect();
    files.sort();
    files.dedup();
    let mut ports: Vec<_> = process.ports.iter().collect();
    ports.sort();
    ports.dedup();
    (
        Capped::new(files),
        Capped::new(ports.into_iter().map(port_label)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Protocol, Usage};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    fn entry(pid: u32, parent: u32, started: u64) -> ProcessEntry {
        ProcessEntry {
            identity: Identity {
                pid,
                started,
                started_sub: 0,
            },
            parent,
            name: format!("p{pid}"),
        }
    }
    fn parent() -> Identity {
        Identity {
            pid: 100,
            started: 50,
            started_sub: 0,
        }
    }

    #[test]
    fn children_are_direct_and_sorted_by_birth() {
        let table = [
            entry(203, 100, 70),
            entry(201, 100, 60),
            entry(300, 201, 80),
            entry(100, 1, 50),
            entry(400, 7, 90),
        ];
        let pids: Vec<_> = children_of(&table, parent())
            .iter()
            .map(|child| child.identity.pid)
            .collect();
        assert_eq!(pids, [201, 203]);
    }

    #[test]
    fn reused_parent_pid_does_not_adopt_older_processes() {
        // 150 was started by an earlier process that also had PID 100.
        let table = [entry(150, 100, 10), entry(151, 100, 50), entry(152, 100, 0)];
        let pids: Vec<_> = children_of(&table, parent())
            .iter()
            .map(|child| child.identity.pid)
            .collect();
        assert_eq!(pids, [151]);
    }

    #[test]
    fn sub_second_birth_breaks_ties_on_macos() {
        let parent = Identity {
            pid: 100,
            started: 50,
            started_sub: 500,
        };
        let mut early = entry(1, 100, 50);
        early.identity.started_sub = 499;
        let mut late = entry(2, 100, 50);
        late.identity.started_sub = 501;
        let pids: Vec<_> = children_of(&[early, late], parent)
            .iter()
            .map(|child| child.identity.pid)
            .collect();
        assert_eq!(pids, [2]);
    }

    #[test]
    fn capped_lists_count_what_they_hide() {
        let capped = Capped::new((0..8).map(|n| n.to_string()));
        assert_eq!(capped.items, ["0", "1", "2", "3", "4"]);
        assert_eq!(capped.more, 3);
        assert_eq!(capped.total(), 8);
        assert_eq!(Capped::new(Vec::new()), Capped::default());
    }

    #[test]
    fn scanned_resources_are_deduplicated_and_labelled() {
        let port = |protocol, address, number| Port {
            protocol,
            address,
            number,
        };
        let process = Process {
            usages: vec![
                Usage {
                    path: "/build/b.blend".into(),
                    ..Usage::default()
                },
                Usage {
                    path: "/build/a.blend".into(),
                    ..Usage::default()
                },
                Usage {
                    path: "/build/a.blend".into(),
                    ..Usage::default()
                },
            ],
            ports: vec![
                port(Protocol::Udp, IpAddr::V6(Ipv6Addr::LOCALHOST), 53),
                port(Protocol::Tcp, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 3000),
                port(Protocol::Tcp, IpAddr::V4(Ipv4Addr::LOCALHOST), 5432),
            ],
            ..Process::default()
        };
        let (files, ports) = scanned_resources(&process);
        assert_eq!(files.items, ["/build/a.blend", "/build/b.blend"]);
        assert_eq!(
            ports.items,
            ["TCP 3000", "TCP 127.0.0.1:5432", "UDP [::1]:53"]
        );
    }
}
