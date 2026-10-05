//! Synthetic search/paging measurements; excluded from distributed application builds.
use oflh_core::{Identity, Process, Snapshot, Usage};
use oflh_desktop::{
    contract::{Sort, TableQuery},
    dataset::Dataset,
};
use serde_json::json;
use std::{path::PathBuf, process::Command, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let baseline = match arguments.next() {
        None => None,
        Some(flag) if flag == "--baseline" => {
            Some(arguments.next().ok_or("missing baseline executable")?)
        }
        Some(_) => return Err("unknown grid profiler argument".into()),
    };
    if arguments.next().is_some() {
        return Err("unknown grid profiler argument".into());
    }
    let baseline = baseline
        .map(
            |path| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
                let output = Command::new(path).output()?;
                if !output.status.success() {
                    return Err("baseline grid profile failed".into());
                }
                let profile: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                if profile["rows"] != 50000 || profile["pages"] != 100 {
                    return Err("baseline grid fixture mismatch".into());
                }
                Ok(profile)
            },
        )
        .transpose()?;
    let candidate = profile()?;
    let report = baseline.map_or_else(
        || candidate.clone(),
        |baseline| {
            json!({"schema":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
                "baseline":baseline,"candidate":candidate})
        },
    );
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn profile() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let snapshot = Snapshot {
        processes: (0..500)
            .map(|index| Process {
                identity: Identity {
                    pid: 4000 + index,
                    started: 10,
                    started_sub: 0,
                },
                name: format!("worker-{index:04}"),
                usages: (0..100)
                    .map(|usage| Usage {
                        path: PathBuf::from(format!("/fixture/{index:04}/file-{usage:03}.bin")),
                        ..Usage::default()
                    })
                    .collect(),
                ..Process::default()
            })
            .collect(),
        warnings: vec![],
    };
    let started = Instant::now();
    let dataset = Dataset::new(1, snapshot);
    let indexing_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut query = TableQuery {
        handles: true,
        text: "worker".into(),
        sort: Sort::Path,
        limit: 200,
        ..TableQuery::default()
    };
    let started = Instant::now();
    let first = dataset.page(&query)?;
    let search_ms = started.elapsed().as_secs_f64() * 1000.0;
    if first.total != 50000 {
        return Err("fixture coverage mismatch".into());
    }
    let mut times = Vec::new();
    for offset in (0..20000).step_by(200) {
        query.offset = offset;
        let started = Instant::now();
        let page = dataset.page(&query)?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
        if page.rows.len() != 200 || page.total != 50000 {
            return Err("paging coverage mismatch".into());
        }
    }
    times.sort_by(f64::total_cmp);
    Ok(json!({"schema":1,"rows":50000,"processes":500,"pages":100,
        "indexing_ms":indexing_ms,"initial_search_ms":search_ms,"page_median_ms":times[50],"page_p95_ms":times[95]}))
}
