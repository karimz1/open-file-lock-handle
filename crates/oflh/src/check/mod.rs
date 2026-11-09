//! Non-interactive file, folder, and port inspection for scripts and build jobs.
use crate::CliError;
use oflh_core::Snapshot;
use oflh_tui::{Language, StartOptions};
use std::ffi::OsString;

pub const EXIT_HELD: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_INSPECTION_FAILED: u8 = 3;

pub(crate) mod output;

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

/// One inspection of the requested scope, shared by `check` and `wait` timeouts.
pub(crate) struct Inspection {
    pub snapshot: Snapshot,
    pub target: Option<oflh_core::Target>,
    pub outcome: Outcome,
}

/// Resolve the scanned path: a port inspection without a path covers all bindings.
pub(crate) fn resolve_target(
    path: Option<&OsString>,
    ports: bool,
    follow_port_folder: bool,
) -> Result<Option<oflh_core::Target>, CliError> {
    if ports && follow_port_folder {
        return Ok(None);
    }
    Ok(Some(oflh_core::Target::new(
        path.cloned().unwrap_or_else(|| OsString::from(".")),
    )?))
}

/// The free/held rule for this scope, identical for `check` and `wait`.
pub(crate) fn check_target(
    target: Option<&oflh_core::Target>,
    ports: bool,
    port: Option<u16>,
) -> oflh_core::check::CheckTarget {
    oflh_core::check::CheckTarget::select(target.is_some(), ports, port)
}

/// Scan once, classify before trimming, and keep the evidence a port-filtered TUI shows.
pub(crate) fn inspect(
    backend: &mut dyn oflh_platform::Backend,
    target: Option<oflh_core::Target>,
    ports: bool,
    port: Option<u16>,
    ports_path_only: bool,
    cancel: &oflh_core::Cancellation,
) -> Result<Inspection, CliError> {
    let check = check_target(target.as_ref(), ports, port);
    let mut snapshot = match &target {
        Some(target) if ports => oflh_platform::scan_with_ports(backend, target, cancel)?,
        Some(target) => backend.scan(target, cancel)?,
        None => oflh_platform::scan_ports(cancel)?,
    };
    let outcome = if oflh_core::check::is_free(&snapshot, check) {
        Outcome::Free
    } else {
        Outcome::Held
    };
    prepare(&mut snapshot, port, ports, ports_path_only);
    Ok(Inspection {
        snapshot,
        target,
        outcome,
    })
}

pub fn run(options: Options) -> Result<Outcome, CliError> {
    let cancel = oflh_core::Cancellation::default();
    let target = resolve_target(
        options.path.as_ref(),
        options.ports,
        options.follow_port_folder,
    )?;
    let mut backend = oflh_platform::native()?;
    let inspection = inspect(
        backend.as_mut(),
        target,
        options.ports,
        options.port,
        options.ports_path_only,
        &cancel,
    )?;
    output::write(
        &options,
        &inspection.snapshot,
        inspection.target.as_ref(),
        inspection.outcome,
    )?;
    Ok(inspection.outcome)
}

/// Keep the same visible evidence as a port-filtered TUI result.
fn prepare(snapshot: &mut Snapshot, port: Option<u16>, ports: bool, ports_path_only: bool) {
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
        if let Some(number) = port {
            process.ports.retain(|binding| binding.number == number);
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
        prepare(&mut snapshot, Some(3000), true, true);
        assert_eq!(snapshot.processes.len(), 1);
        assert_eq!(snapshot.processes[0].identity.pid, 10);
        assert_eq!(snapshot.processes[0].ports.len(), 1);
        assert_eq!(snapshot.processes[0].ports[0].number, 3000);
        assert!(snapshot.processes[0].usages.is_empty());
        assert_eq!(snapshot.warnings, ["partial"]);
    }
}
