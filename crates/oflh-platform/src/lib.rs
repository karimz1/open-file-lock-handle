//! Native OS boundaries. A scanner is owned by one worker, never shared concurrently.
#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
pub mod elevation;
pub mod inspection_helper;
mod locale;
pub mod updates;
pub use locale::system_locale;
#[cfg(any(windows, test))]
mod inspection_protocol;
#[cfg(any(windows, test))]
mod inspection_transport;
use oflh_core::*;
use std::collections::HashMap;
#[cfg(not(target_os = "linux"))]
use std::time::Instant;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "macos", test))]
mod native_buffer;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod ports;
#[cfg(any(target_os = "macos", test))]
mod process_pool;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

/// Operations provided by a single worker-owned native scanner.
pub trait Backend: Send + 'static {
    /// Collect current observations, honoring cancellation between native calls.
    fn scan(&mut self, target: &Target, cancel: &Cancellation) -> Result<Snapshot>;
    /// Sample resources only for the supplied process birth identities.
    fn sample(
        &mut self,
        ids: &[Identity],
        cancel: &Cancellation,
    ) -> Result<Vec<(Identity, Metrics)>>;
    /// Check whether this captured lifetime is still running, without sending a signal.
    /// Permission failures must remain errors, never evidence of process exit.
    fn is_running(&mut self, _id: Identity) -> Result<bool> {
        Err(Error::Unavailable(
            "process exit verification unavailable".into(),
        ))
    }
    /// Request termination after validating identity and protected-process guards.
    fn terminate(&mut self, id: Identity, force: bool, cancel: &Cancellation) -> Result<()>;
}
/// Construct the scanner for the current operating system.
pub fn native() -> Result<Box<dyn Backend>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::Native::default()))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::Native::default()))
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Native::default()))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Err(Error::Unavailable("unsupported operating system".into()))
    }
}

/// Construct a scanner with a headless helper command supplied by its embedding binary.
/// Windows helpers must call [`inspection_helper::run_stdio`] before normal startup.
/// Other platforms inspect process references directly and ignore this command.
pub fn native_with_inspection_helper(
    helper: inspection_helper::InspectionHelperCommand,
) -> Result<Box<dyn Backend>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::Native::with_helper(helper)))
    }
    #[cfg(not(windows))]
    {
        let _ = helper;
        native()
    }
}
#[derive(Default)]
struct Sampler {
    previous: HashMap<Identity, (u64, u64)>,
    #[cfg(not(target_os = "linux"))]
    epoch: Option<Instant>,
}
impl Sampler {
    #[cfg(not(target_os = "linux"))]
    fn clock(&mut self) -> u64 {
        let epoch = self.epoch.get_or_insert_with(Instant::now);
        epoch.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
    fn sample(
        &mut self,
        raw: Vec<(Identity, u64, Option<u64>)>,
        total: u64,
    ) -> Vec<(Identity, Metrics)> {
        let mut next = HashMap::with_capacity(raw.len());
        let result = raw
            .into_iter()
            .map(|(id, cpu, memory)| {
                let percent = self.previous.get(&id).and_then(|&(prev, old)| {
                    (total > old && cpu >= prev)
                        .then(|| (100.0 * (cpu - prev) as f64 / (total - old) as f64).min(100.0))
                });
                next.insert(id, (cpu, total));
                (
                    id,
                    Metrics {
                        cpu: percent,
                        memory,
                    },
                )
            })
            .collect();
        self.previous = next;
        result
    }
}
fn apply_metrics(snapshot: &mut Snapshot, metrics: Vec<(Identity, Metrics)>) {
    let metrics: HashMap<_, _> = metrics.into_iter().collect();
    for process in &mut snapshot.processes {
        if let Some(sample) = metrics.get(&process.identity) {
            process.memory = sample.memory;
            process.cpu = sample.cpu;
        }
    }
}

#[cfg(test)]
mod metric_identity_tests {
    use super::*;

    fn identity(started_sub: u64) -> Identity {
        Identity {
            pid: 4000,
            started: 10,
            started_sub,
        }
    }

    #[test]
    fn sampling_never_inherits_cpu_from_a_reused_pid_or_an_absent_lifetime() {
        let original = identity(1);
        let reused = identity(2);
        let mut sampler = Sampler::default();
        let first = sampler.sample(vec![(original, 100, Some(1024))], 1000);
        assert_eq!(first[0].1.cpu, None);
        assert_eq!(first[0].1.memory, Some(1024));
        let next = sampler.sample(vec![(original, 150, None), (reused, 900, Some(2048))], 1100);
        assert_eq!(next[0].1.cpu, Some(50.0));
        assert_eq!(next[0].1.memory, None); // Unknown memory is not zero.
        assert_eq!(next[1].1.cpu, None);
        assert_eq!(next[1].1.memory, Some(2048));
        assert!(sampler.sample(vec![], 1200).is_empty());
        assert_eq!(
            sampler.sample(vec![(original, 200, None)], 1300)[0].1.cpu,
            None
        );
    }

    #[test]
    fn invalid_counter_deltas_are_unknown_and_a_later_sample_can_recover() {
        let process = identity(1);
        for (cpu, total) in [(150, 1000), (150, 999), (99, 1100)] {
            let mut sampler = Sampler::default();
            sampler.sample(vec![(process, 100, None)], 1000);
            assert_eq!(
                sampler.sample(vec![(process, cpu, None)], total)[0].1.cpu,
                None
            );
            assert_eq!(
                sampler.sample(vec![(process, cpu + 50, None)], total + 100)[0]
                    .1
                    .cpu,
                Some(50.0)
            );
        }
        let mut sampler = Sampler::default();
        sampler.sample(vec![(process, 0, None)], 1);
        assert_eq!(
            sampler.sample(vec![(process, 1000, None)], 2)[0].1.cpu,
            Some(100.0)
        );
    }

    #[test]
    fn applying_samples_matches_the_complete_birth_identity_and_keeps_unknown_metrics() {
        let original = identity(1);
        let reused = identity(2);
        let mut snapshot = Snapshot {
            processes: vec![Process {
                identity: original,
                memory: Some(1024),
                cpu: Some(25.0),
                ..Process::default()
            }],
            warnings: vec![],
        };
        apply_metrics(
            &mut snapshot,
            vec![(
                reused,
                Metrics {
                    memory: Some(2048),
                    cpu: Some(90.0),
                },
            )],
        );
        assert_eq!(snapshot.processes[0].memory, Some(1024));
        assert_eq!(snapshot.processes[0].cpu, Some(25.0));
        apply_metrics(
            &mut snapshot,
            vec![(
                original,
                Metrics {
                    memory: None,
                    cpu: None,
                },
            )],
        );
        assert_eq!(snapshot.processes[0].memory, None);
        assert_eq!(snapshot.processes[0].cpu, None);
    }
}

/// Inspect local TCP listeners and UDP bindings using native APIs.
/// Run on a worker thread; process ownership is checked against birth identities.
pub fn scan_ports(cancel: &Cancellation) -> Result<Snapshot> {
    cancel.set_phase(InspectionPhase::Ports);
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        ports::scan(cancel)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        cancel.check()?;
        Err(Error::Unavailable(
            "port discovery is unavailable on this platform".into(),
        ))
    }
}

/// Scan a target and merge global local-port observations by captured identity.
/// File-scan errors are fatal; port coverage failures remain partial-result warnings.
pub fn scan_with_ports(
    backend: &mut dyn Backend,
    target: &Target,
    cancel: &Cancellation,
) -> Result<Snapshot> {
    let mut snapshot = backend.scan(target, cancel)?;
    match scan_ports(cancel) {
        Ok(ports) => {
            snapshot.processes.extend(ports.processes);
            snapshot.warnings.extend(ports.warnings);
            snapshot.normalize();
        }
        Err(Error::Cancelled) => return Err(Error::Cancelled),
        Err(error) => snapshot.warnings.push(format!("Port scan failed: {error}")),
    }
    cancel.check()?;
    Ok(snapshot)
}

/// Inspect an owner folder only if the originally captured lifetime remains visible.
/// Resolve on a worker; callers must commit the target and snapshot together.
pub fn scan_port_folder(
    backend: &mut dyn Backend,
    identity: Identity,
    path: &std::path::Path,
    cancel: &Cancellation,
) -> Result<(Target, Snapshot)> {
    cancel.check()?;
    let target = Target::new(path)?;
    if !target.directory {
        return Err(Error::Unavailable(
            "Process folder is no longer a directory".into(),
        ));
    }
    let snapshot = scan_with_ports(backend, &target, cancel)?;
    if !snapshot
        .processes
        .iter()
        .any(|process| process.identity == identity)
    {
        return Err(Error::Changed);
    }
    Ok((target, snapshot))
}

#[cfg(test)]
mod port_folder_tests {
    use super::*;

    struct FolderBackend {
        identity: Identity,
        expected: std::path::PathBuf,
    }
    impl Backend for FolderBackend {
        fn scan(&mut self, target: &Target, _: &Cancellation) -> Result<Snapshot> {
            assert_eq!(target.path, self.expected);
            Ok(Snapshot {
                processes: vec![Process {
                    identity: self.identity,
                    usages: vec![Usage {
                        path: target.path.join("fixture.bin"),
                        ..Usage::default()
                    }],
                    ..Process::default()
                }],
                warnings: vec![],
            })
        }
        fn sample(&mut self, _: &[Identity], _: &Cancellation) -> Result<Vec<(Identity, Metrics)>> {
            unreachable!()
        }
        fn terminate(&mut self, _: Identity, _: bool, _: &Cancellation) -> Result<()> {
            unreachable!()
        }
    }

    #[test]
    fn folder_scan_keeps_file_results_and_rejects_reused_identity() {
        let target = Target::new(std::env::temp_dir()).unwrap();
        let identity = Identity {
            pid: u32::MAX,
            started: 10,
            started_sub: 0,
        };
        let mut backend = FolderBackend {
            identity,
            expected: target.path.clone(),
        };
        let (resolved, snapshot) = scan_port_folder(
            &mut backend,
            identity,
            &target.path,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(resolved.path, target.path);
        assert!(
            snapshot
                .processes
                .iter()
                .any(|process| process.identity == identity && process.usages.len() == 1)
        );
        let reused = Identity {
            started: 11,
            ..identity
        };
        assert!(matches!(
            scan_port_folder(&mut backend, reused, &target.path, &Cancellation::default(),),
            Err(Error::Changed)
        ));
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            scan_port_folder(&mut backend, identity, &target.path, &cancel,),
            Err(Error::Cancelled)
        ));
    }
}
