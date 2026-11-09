//! Non-interactive file, folder, and port inspection for scripts and build jobs.
use crate::CliError;
use oflh_core::Snapshot;
use oflh_tui::{Language, StartOptions};
use std::ffi::OsString;

pub const EXIT_HELD: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_INSPECTION_FAILED: u8 = 3;

mod output;

pub struct Options {
    pub json: bool,
    pub language: Language,
    pub path: Option<OsString>,
    pub ports: bool,
    pub port: Option<u16>,
    pub follow_port_folder: bool,
    pub ports_path_only: bool,
}

impl Options {
    pub fn from_view(
        view: &StartOptions,
        language: Language,
        path: Option<OsString>,
        json: bool,
    ) -> Self {
        Self {
            json,
            language,
            path,
            ports: view.ports,
            port: view.port,
            follow_port_folder: view.follow_port_folder,
            ports_path_only: view.ports_path_only,
        }
    }
}

/// Exit status returned after a completed or failed one-shot inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Free,
    Held,
}

pub fn run(options: Options) -> Result<Outcome, CliError> {
    let cancel = oflh_core::Cancellation::default();
    let target = if options.ports && options.follow_port_folder {
        None
    } else {
        Some(oflh_core::Target::new(
            options.path.clone().unwrap_or_else(|| OsString::from(".")),
        )?)
    };
    let mut backend = oflh_platform::native()?;
    let mut snapshot = if let Some(target) = &target {
        if options.ports {
            oflh_platform::scan_with_ports(backend.as_mut(), target, &cancel)?
        } else {
            backend.scan(target, &cancel)?
        }
    } else {
        oflh_platform::scan_ports(&cancel)?
    };
    let check_target = match options.port {
        Some(port) => oflh_core::check::CheckTarget::Port(port),
        None => oflh_core::check::CheckTarget::Path,
    };
    prepare(
        &mut snapshot,
        &check_target,
        options.ports,
        options.ports_path_only,
    );
    let outcome = classify(&snapshot, check_target, options.ports);
    output::write(&options, &snapshot, target.as_ref(), outcome)?;
    Ok(outcome)
}

/// A port listing (`--ports` without `--port`) is held when it lists any binding;
/// `prepare` has already removed file usages, so the path rule would always say free.
fn classify(snapshot: &Snapshot, target: oflh_core::check::CheckTarget, ports: bool) -> Outcome {
    let free = match target {
        oflh_core::check::CheckTarget::Path if ports => snapshot.processes.is_empty(),
        target => oflh_core::check::is_free(snapshot, target),
    };
    if free { Outcome::Free } else { Outcome::Held }
}

/// Keep the same visible evidence as a port-filtered TUI result.
fn prepare(
    snapshot: &mut Snapshot,
    target: &oflh_core::check::CheckTarget,
    ports: bool,
    ports_path_only: bool,
) {
    let owned_identities: std::collections::HashSet<_> = snapshot
        .processes
        .iter()
        .filter(|process| !process.usages.is_empty())
        .map(|process| process.identity)
        .collect();
    snapshot.normalize();
    if !ports {
        for process in &mut snapshot.processes {
            process.ports.clear();
        }
        snapshot
            .processes
            .retain(|process| !process.usages.is_empty());
        return;
    }
    if ports_path_only {
        snapshot
            .processes
            .retain(|process| owned_identities.contains(&process.identity));
    }
    for process in &mut snapshot.processes {
        if let oflh_core::check::CheckTarget::Port(number) = target {
            process.ports.retain(|port| port.number == *number);
        }
        process.usages.clear();
    }
    snapshot
        .processes
        .retain(|process| !process.ports.is_empty());
}

pub fn error_exit_code(error: &CliError) -> u8 {
    if matches!(error, CliError::Usage(_)) {
        EXIT_USAGE
    } else {
        EXIT_INSPECTION_FAILED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Identity, Port, Process, Protocol, Usage};

    #[test]
    fn check_exit_codes_distinguish_free_held_usage_and_inspection_errors() {
        assert_eq!(EXIT_HELD, 1);
        assert_eq!(error_exit_code(&CliError::Usage("bad option".into())), 2);
        assert_eq!(
            error_exit_code(&CliError::Core(oflh_core::Error::Cancelled)),
            3
        );
    }

    #[test]
    fn preparation_keeps_only_requested_bindings_and_path_owned_ports() {
        let mut snapshot = Snapshot {
            processes: vec![
                Process {
                    identity: Identity {
                        pid: 10,
                        ..Identity::default()
                    },
                    usages: vec![Usage::default()],
                    ports: vec![
                        Port {
                            protocol: Protocol::Tcp,
                            address: "127.0.0.1".parse().unwrap(),
                            number: 3000,
                        },
                        Port {
                            protocol: Protocol::Udp,
                            address: "127.0.0.1".parse().unwrap(),
                            number: 3001,
                        },
                    ],
                    ..Process::default()
                },
                Process {
                    identity: Identity {
                        pid: 20,
                        ..Identity::default()
                    },
                    ports: vec![Port {
                        protocol: Protocol::Tcp,
                        address: "0.0.0.0".parse().unwrap(),
                        number: 3000,
                    }],
                    ..Process::default()
                },
            ],
            warnings: vec!["partial".into()],
        };
        prepare(
            &mut snapshot,
            &oflh_core::check::CheckTarget::Port(3000),
            true,
            true,
        );
        assert_eq!(snapshot.processes.len(), 1);
        assert_eq!(snapshot.processes[0].identity.pid, 10);
        assert_eq!(snapshot.processes[0].ports.len(), 1);
        assert_eq!(snapshot.processes[0].ports[0].number, 3000);
        assert!(snapshot.processes[0].usages.is_empty());
        assert_eq!(snapshot.warnings, ["partial"]);
    }

    #[test]
    fn port_listings_are_held_when_they_list_a_binding() {
        let mut snapshot = Snapshot {
            processes: vec![Process {
                usages: vec![Usage::default()],
                ports: vec![Port {
                    protocol: Protocol::Tcp,
                    address: "127.0.0.1".parse().unwrap(),
                    number: 3000,
                }],
                ..Process::default()
            }],
            warnings: vec![],
        };
        prepare(
            &mut snapshot,
            &oflh_core::check::CheckTarget::Path,
            true,
            true,
        );
        assert_eq!(
            classify(&snapshot, oflh_core::check::CheckTarget::Path, true),
            Outcome::Held
        );
        snapshot.processes.clear();
        assert_eq!(
            classify(&snapshot, oflh_core::check::CheckTarget::Path, true),
            Outcome::Free
        );
    }
}
