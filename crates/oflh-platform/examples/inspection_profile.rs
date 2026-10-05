//! Reproducible, privacy-safe native fixture profiler. Never linked into the app.
use oflh_core::{Cancellation, InspectionProgress, Snapshot, Target};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Instant,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn file_path(root: &Path, index: usize) -> PathBuf {
    root.join(format!("group-{:02}", index % 16))
        .join(format!("file-{index:05}.bin"))
}
fn hold_fixture(root: &Path, held: usize) -> Result<()> {
    let mut files = Vec::new();
    for index in 0..held {
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1); // Deny writes/deletes; never claim which user caused it.
        }
        files.push(options.open(file_path(root, index))?);
    }
    println!("READY");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    std::hint::black_box(files);
    Ok(())
}
fn start_fixture(root: &Path, held: usize) -> Result<Fixture> {
    let mut fixture = Fixture(
        Command::new(std::env::current_exe()?)
            .arg("--fixture")
            .arg(root)
            .arg(held.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let output = fixture.0.stdout.take().ok_or("missing fixture stdout")?;
    let (ready, received) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line);
        let _ = ready.send(result.is_ok() && line.trim() == "READY");
    });
    if !received.recv_timeout(std::time::Duration::from_secs(15))? {
        return Err("native fixture failed to become ready".into());
    }
    Ok(fixture)
}
fn coverage(snapshot: &Snapshot, pid: u32, root: &Path) -> BTreeSet<String> {
    snapshot
        .processes
        .iter()
        .filter(|process| process.identity.pid == pid)
        .flat_map(|process| &process.usages)
        .filter_map(|usage| {
            usage.path.strip_prefix(root).ok().map(|path| {
                format!(
                    "{}\t{}\t{}\t{}\t{}",
                    path.display(),
                    usage.relation.label(),
                    usage.access.label(),
                    usage.deleted,
                    usage
                        .lock
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default()
                )
            })
        })
        .collect()
}
fn baseline_coverage(binary: &Path, target: &Path, pid: u32) -> Result<BTreeSet<String>> {
    let output = Command::new(binary).arg(target).arg("--dump").output()?;
    if !output.status.success() {
        return Err("baseline scanner failed".into());
    }
    let mut rows = BTreeSet::new();
    for line in std::str::from_utf8(&output.stdout)?.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() == 7
            && fields[0].parse::<u32>()? == pid
            && let Ok(path) = Path::new(fields[2]).strip_prefix(target)
        {
            rows.insert(format!("{}\t{}", path.display(), fields[3..].join("\t")));
        }
    }
    Ok(rows)
}
fn counters(progress: InspectionProgress) -> Value {
    json!({"processes":progress.processes,"resources":progress.resources,
        "files":progress.files,"directories":progress.directories,
        "resource_queries":progress.resource_queries,"resource_query_ms":progress.resource_query_micros as f64 / 1000.0,
        "module_snapshots":progress.module_snapshots,"module_snapshot_ms":progress.module_snapshot_micros as f64 / 1000.0,
        "file_identity_queries":progress.file_identity_queries})
}
fn summarize(mut timings: Vec<f64>) -> Value {
    timings.sort_by(f64::total_cmp);
    json!({"samples":timings.len(),"median_ms":timings[timings.len()/2],
        "p95_ms":timings[(timings.len()*95/100).min(timings.len()-1)]})
}
fn baseline_timing(binary: &Path, target: &Path) -> Result<f64> {
    let output = Command::new(binary).arg(target).arg("1").output()?;
    if !output.status.success() {
        return Err("baseline scanner failed".into());
    }
    let text = std::str::from_utf8(&output.stdout)?;
    let timing = text
        .split_whitespace()
        .find_map(|part| part.strip_prefix("median_ms="))
        .ok_or("baseline timing missing")?;
    Ok(timing.parse()?)
}
fn profile(name: &str, held: usize, baseline: Option<&Path>) -> Result<Value> {
    let directory = tempfile::tempdir()?;
    for index in 0..2048 {
        let path = file_path(directory.path(), index);
        fs::create_dir_all(path.parent().ok_or("missing fixture parent")?)?;
        fs::write(path, [0u8; 64])?;
    }
    let fixture = start_fixture(directory.path(), held)?;
    let target = Target::new(directory.path())?;
    let expected_base = baseline
        .map(|binary| baseline_coverage(binary, &target.path, fixture.0.id()))
        .transpose()?;
    let mut timings = Vec::new();
    let mut baseline_timings = Vec::new();
    let mut last_counters = Value::Null;
    let mut expected = None;
    // Warm both implementations once, then alternate order to reduce cache/order bias.
    // Both construct a fresh backend for each sample; subprocess startup is excluded.
    for sample in 0..=5 {
        let first_baseline = if sample % 2 == 0 {
            baseline
                .map(|binary| baseline_timing(binary, &target.path))
                .transpose()?
        } else {
            None
        };
        let cancel = Cancellation::default();
        let mut backend = oflh_platform::native()?;
        let started = Instant::now();
        let snapshot = backend.scan(&target, &cancel)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let observed = coverage(&snapshot, fixture.0.id(), &target.path);
        for index in 0..held {
            let relative = file_path(Path::new(""), index).display().to_string();
            if !observed
                .iter()
                .any(|row| row.starts_with(&format!("{relative}\t")))
            {
                return Err("fixture file user missing; timing rejected".into());
            }
        }
        if expected
            .as_ref()
            .is_some_and(|previous| previous != &observed)
            || expected_base
                .as_ref()
                .is_some_and(|previous| previous != &observed)
        {
            return Err(
                "fixture observations changed or baseline coverage differs; timing rejected".into(),
            );
        }
        expected = Some(observed);
        last_counters = counters(cancel.progress());
        let baseline_time = if sample % 2 == 0 {
            first_baseline
        } else {
            baseline
                .map(|binary| baseline_timing(binary, &target.path))
                .transpose()?
        };
        if sample > 0 {
            timings.push(elapsed);
            if let Some(elapsed) = baseline_time {
                baseline_timings.push(elapsed);
            }
        }
    }
    let mut result = json!({"fixture":name,"files":2048,"held_files":held,
        "candidate":summarize(timings),"counters":last_counters,
        "coverage_rows":expected.as_ref().map_or(0,BTreeSet::len)});
    if let Some(binary) = baseline {
        let observed = baseline_coverage(binary, &target.path, fixture.0.id())?;
        if expected.as_ref() != Some(&observed) {
            return Err("baseline/candidate coverage differs; timing rejected".into());
        }
        result["baseline"] = summarize(baseline_timings);
        result["equivalent_fixture_coverage"] = json!(true);
    }
    Ok(result)
}
/// Whole-root results describe each backend's scope; they cannot assert equivalent
/// observations from a live machine. A budget is cancellation, never a faster scan.
fn root_diagnostic(baseline: Option<&Path>, budget: std::time::Duration) -> Result<Value> {
    let target = Target::new(if cfg!(windows) { "C:\\" } else { "/" })?;
    let cancel = Cancellation::default();
    let worker_cancel = cancel.clone();
    let (finished, receiver) = std::sync::mpsc::sync_channel(1);
    let timer = std::thread::spawn(move || {
        if receiver.recv_timeout(budget).is_err() {
            worker_cancel.cancel();
        }
    });
    let mut backend = oflh_platform::native()?;
    let started = Instant::now();
    let snapshot = backend.scan(&target, &cancel);
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    let _ = finished.send(());
    timer.join().map_err(|_| "root budget worker failed")?;
    let candidate = match snapshot {
        Ok(snapshot) => json!({"outcome":"completed","elapsed_ms":elapsed,
            "processes":snapshot.processes.len(),
            "usages":snapshot.processes.iter().map(|process| process.usages.len()).sum::<usize>(),
            "warning_count":snapshot.warnings.len(),
            "directory_cap_reached":snapshot.warnings.iter().any(|warning| warning.contains("limited to 10,000 files"))}),
        Err(oflh_core::Error::Cancelled) => {
            json!({"outcome":"budget_cancelled","elapsed_ms":elapsed})
        }
        Err(_) => json!({"outcome":"failed","elapsed_ms":elapsed}),
    };
    let mut result = json!({"scope":if cfg!(windows) {"C drive; directory resources capped at 10000 files"} else {"root process references; no disk traversal"},
        "budget_seconds":budget.as_secs(),"equivalent_coverage":false,"candidate":candidate,"counters":counters(cancel.progress())});
    if let Some(binary) = baseline {
        let mut child = Fixture(
            Command::new(binary)
                .arg(&target.path)
                .arg("1")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let output = child.0.stdout.take().ok_or("baseline stdout missing")?;
        let reader = std::thread::spawn(move || {
            let mut text = String::new();
            use std::io::Read;
            BufReader::new(output)
                .read_to_string(&mut text)
                .map(|_| text)
        });
        let started = Instant::now();
        let completed = loop {
            if let Some(status) = child.0.try_wait()? {
                break status.success();
            }
            if started.elapsed() >= budget {
                child.0.kill()?;
                child.0.wait()?;
                break false;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let output = reader
            .join()
            .map_err(|_| "baseline output worker failed")??;
        let timing = output
            .split_whitespace()
            .find_map(|part| part.strip_prefix("median_ms="))
            .and_then(|value| value.parse::<f64>().ok());
        result["baseline"] = if completed {
            json!({"outcome":"completed","elapsed_ms":timing})
        } else {
            json!({"outcome":"budget_or_failure","elapsed_ms":started.elapsed().as_secs_f64()*1000.0})
        };
    }
    Ok(result)
}
fn main() -> Result<()> {
    let mut arguments = std::env::args_os().skip(1).peekable();
    if arguments
        .peek()
        .is_some_and(|argument| argument == "--fixture")
    {
        arguments.next();
        let root = arguments.next().ok_or("fixture path missing")?;
        let held = arguments
            .next()
            .ok_or("fixture count missing")?
            .to_str()
            .ok_or("invalid count")?
            .parse()?;
        return hold_fixture(Path::new(&root), held);
    }
    let mut baseline = None;
    let mut whole_disk = false;
    let mut budget_seconds = 120;
    while let Some(argument) = arguments.next() {
        if argument == "--baseline" {
            baseline = Some(PathBuf::from(
                arguments.next().ok_or("baseline binary missing")?,
            ));
        } else if argument == "--whole-disk" {
            whole_disk = true;
        } else if argument == "--budget-seconds" {
            budget_seconds = arguments
                .next()
                .ok_or("budget missing")?
                .to_str()
                .ok_or("invalid budget")?
                .parse::<u64>()?;
        } else {
            return Err("usage: inspection_profile [--baseline scan_bench] [--whole-disk] [--budget-seconds 120]".into());
        }
    }
    if !(1..=600).contains(&budget_seconds) {
        return Err("budget must be between 1 and 600 seconds".into());
    }
    let mut result = json!({"schema":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "fixtures":[profile("sparse",8,baseline.as_deref())?,profile("dense",128,baseline.as_deref())?]});
    if whole_disk {
        result["whole_root"] = root_diagnostic(
            baseline.as_deref(),
            std::time::Duration::from_secs(budget_seconds),
        )?;
    }
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
