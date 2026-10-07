//! Synthetic terminal workloads; developer-only, never linked into the CLI.
#![allow(dead_code, unexpected_cfgs)]
#[path = "../src/app.rs"]
mod app;
#[path = "../src/view.rs"]
mod view;
use app::{App, Sort};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oflh_core::{Identity, LockEvidence, Metrics, Process, Snapshot, Target, Usage};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

fn timed(action: impl FnOnce()) -> f64 {
    let started = Instant::now();
    action();
    started.elapsed().as_secs_f64() * 1000.0
}
fn install_snapshot(app: &mut App, snapshot: Snapshot) -> oflh_core::Result<()> {
    #[cfg(feature = "profiling")]
    app.accept(app::PreparedSnapshot::new(
        snapshot,
        &oflh_core::Cancellation::default(),
    )?);
    #[cfg(not(feature = "profiling"))]
    app.replace(snapshot);
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for process_count in [500, 10000] {
        let usages_per_process = 50000 / process_count;
        let snapshot = Snapshot {
            processes: (0..process_count)
                .map(|index| Process {
                    identity: Identity {
                        pid: 4000 + index,
                        started: 10,
                        started_sub: index as u64,
                    },
                    name: format!("Worker-{:05}", process_count - index),
                    usages: (0..usages_per_process)
                        .map(|usage| Usage {
                            path: format!("/fixture/{index:05}/File-{usage:03}.bin").into(),
                            lock: Some(LockEvidence::Kernel("POSIX WRITE".into())),
                            ..Usage::default()
                        })
                        .collect(),
                    ..Process::default()
                })
                .collect(),
            warnings: vec![],
        };
        let mut app = App::new(Target::new(".")?, "profile".into());
        let started = Instant::now();
        install_snapshot(&mut app, snapshot)?;
        let indexing_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
        for locked in [false, true] {
            app.locked = locked;
            app.query = "worker file*.bin".into();
            app.sort = Sort::Name;
            let search_ms = timed(|| app.refilter());
            let expected_rows = if locked {
                50000
            } else {
                process_count as usize
            };
            assert_eq!(app.rows.len(), expected_rows);
            assert_eq!(
                app.rows.iter().map(|row| row.usages.len()).sum::<usize>(),
                50000
            );
            let render_100_ms = timed(|| {
                for _ in 0..100 {
                    app.key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
                    terminal.draw(|frame| view::draw(frame, &mut app)).unwrap();
                }
            });
            assert_eq!(app.cursor, expected_rows - 1);
            println!(
                "{{\"processes\":{process_count},\"usages\":50000,\"locked\":{locked},\"indexing_ms\":{indexing_ms:.3},\"search_ms\":{search_ms:.3},\"render_100_ms\":{render_100_ms:.3}}}"
            );
        }
        let metrics = app
            .snapshot
            .processes
            .iter()
            .map(|process| {
                (
                    process.identity,
                    Metrics {
                        memory: Some(123),
                        cpu: Some(1.0),
                    },
                )
            })
            .collect();
        let metrics_ms = timed(|| app.metrics(metrics));
        assert!(
            app.snapshot
                .processes
                .iter()
                .all(|process| process.memory == Some(123))
        );
        println!("{{\"processes\":{process_count},\"metrics_ms\":{metrics_ms:.3}}}");
    }
    Ok(())
}
