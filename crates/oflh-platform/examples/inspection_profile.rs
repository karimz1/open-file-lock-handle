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
fn main() -> Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    if let Some(first) = arguments.next() {
        if first == "--fixture" {
            let root = arguments.next().ok_or("fixture path missing")?;
            let held = arguments
                .next()
                .ok_or("fixture count missing")?
                .to_str()
                .ok_or("invalid count")?
                .parse()?;
            return hold_fixture(Path::new(&root), held);
        }
        if first != "--baseline" {
            return Err("usage: inspection_profile [--baseline scan_bench]".into());
        }
        let baseline = PathBuf::from(arguments.next().ok_or("baseline binary missing")?);
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"schema":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "fixtures":[profile("sparse",8,Some(&baseline))?,profile("dense",128,Some(&baseline))?]})
            )?
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"schema":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "fixtures":[profile("sparse",8,None)?,profile("dense",128,None)?]})
            )?
        );
    }
    Ok(())
}
