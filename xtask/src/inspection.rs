//! Summaries consume only synthetic counts and timings; never native paths or PIDs.
use crate::Result;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

const TARGETS: [(&str, &str); 6] = [
    ("linux", "x86_64"),
    ("linux", "aarch64"),
    ("macos", "x86_64"),
    ("macos", "aarch64"),
    ("windows", "x86_64"),
    ("windows", "aarch64"),
];
fn duration(profile: &Value, side: &str, metric: &str) -> Result<f64> {
    let value = profile[side][metric].as_f64().ok_or("missing timing")?;
    if !value.is_finite() || value <= 0.0 {
        return Err("invalid timing".into());
    }
    Ok(value)
}
fn summarize(profiles: &[Value]) -> Result<String> {
    if profiles.len() != 6 {
        return Err("expected six native inspection profiles".into());
    }
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let mut root_rows = Vec::new();
    for profile in profiles {
        let os = profile["os"].as_str().ok_or("missing OS")?;
        let arch = profile["arch"].as_str().ok_or("missing architecture")?;
        if profile["schema"] != 1 || !TARGETS.contains(&(os, arch)) || !seen.insert((os, arch)) {
            return Err("unexpected or duplicate native profile".into());
        }
        if let Some(root) = profile.get("whole_root") {
            let result = |side: &str| -> Result<String> {
                let outcome = root[side]["outcome"]
                    .as_str()
                    .ok_or("missing root outcome")?;
                if ![
                    "completed",
                    "budget_cancelled",
                    "budget_or_failure",
                    "failed",
                ]
                .contains(&outcome)
                {
                    return Err("unexpected root outcome".into());
                }
                let elapsed = root[side]["elapsed_ms"]
                    .as_f64()
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .ok_or("invalid root timing")?;
                Ok(format!("{elapsed:.3} ({outcome})"))
            };
            let cap = if os == "windows" {
                "10,000-file cap; partial coverage"
            } else {
                "process references; no disk traversal"
            };
            root_rows.push(format!(
                "| {os} | {arch} | {} | {} | {cap} |",
                result("baseline")?,
                result("candidate")?
            ));
        }
        let fixtures = profile["fixtures"].as_array().ok_or("missing fixtures")?;
        if fixtures.len() != 2 {
            return Err("expected sparse and dense fixtures".into());
        }
        let mut fixture_names = BTreeSet::new();
        for fixture in fixtures {
            let name = fixture["fixture"].as_str().ok_or("missing fixture name")?;
            let held = match name {
                "sparse" => 8,
                "dense" => 128,
                _ => return Err("unexpected fixture".into()),
            };
            if !fixture_names.insert(name)
                || fixture["held_files"] != held
                || fixture["files"] != 2048
                || fixture["equivalent_fixture_coverage"] != true
                || fixture["coverage_rows"]
                    .as_u64()
                    .is_none_or(|rows| rows < held)
                || fixture["candidate"]["samples"] != 5
                || fixture["baseline"]["samples"] != 5
            {
                return Err("incomplete or unequal fixture coverage".into());
            }
            let base = duration(fixture, "baseline", "median_ms")?;
            let candidate = duration(fixture, "candidate", "median_ms")?;
            let p95 = duration(fixture, "candidate", "p95_ms")?;
            rows.push(format!(
                "| {os} | {arch} | {name} | {base:.3} | {candidate:.3} | {p95:.3} | {:.3} |",
                candidate / base
            ));
        }
    }
    rows.sort();
    root_rows.sort();
    let root_report = if root_rows.is_empty() {
        String::new()
    } else {
        if root_rows.len() != 6 {
            return Err("expected six whole-root diagnostics".into());
        }
        format!(
            "\n## Whole-root diagnostics\n\nOne live-system scan per implementation with a two-minute budget. Coverage is not equivalent or stable across implementations/OSes; cancelled work is not a completed scan or a speedup. Native calls may finish after cancellation is requested.\n\n| OS | Architecture | Base ms / outcome | Candidate ms / outcome | Scope |\n|---|---|---:|---:|---|\n{}\n",
            root_rows.join("\n")
        )
    };
    Ok(format!(
        "# Native inspection comparison\n\nSame held-file fixtures and verified observations on each native runner. Timings are milliseconds; ratio is candidate / baseline (lower is faster). Shared runners and different OS evidence prevent cross-OS speed rankings. Five samples describe this run, not a stable performance guarantee.\n\n| OS | Architecture | Fixture | Base median | Candidate median | Candidate p95 | Ratio |\n|---|---|---|---:|---:|---:|---:|\n{}\n\nEach fixture has 2,048 files in 16 directories. Sparse holds 8 files; dense holds 128. The JSON artifacts include native work counters and Restart Manager/module timings. Whole-drive coverage remains limited on Windows; unused filesystem size does not drive Unix process enumeration.\n",
        rows.join("\n")
    ) + &root_report)
}

pub(super) fn write_summary(input: &Path, output: &Path) -> Result<()> {
    let mut profiles = Vec::new();
    for entry in fs::read_dir(input)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") || !path.is_file() {
            return Err("unexpected profile artifact".into());
        }
        profiles.push(serde_json::from_slice(&fs::read(path)?)?);
    }
    fs::write(output, summarize(&profiles)?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn profiles() -> Vec<Value> {
        let fixtures = ["sparse", "dense"].map(|name| {
            json!({"fixture":name,"files":2048,
                "held_files":if name == "sparse" {8} else {128},"coverage_rows":128,
                "equivalent_fixture_coverage":true,
                "baseline":{"samples":5,"median_ms":20,"p95_ms":22},
                "candidate":{"samples":5,"median_ms":10,"p95_ms":12}})
        });
        TARGETS
            .iter()
            .map(|(os, arch)| json!({"schema":1,"os":os,"arch":arch,"fixtures":fixtures}))
            .collect()
    }
    #[test]
    fn root_diagnostics_do_not_turn_cancelled_work_into_a_speedup() {
        let mut profiles = profiles();
        for profile in &mut profiles {
            profile["whole_root"] = json!({"baseline":{"outcome":"completed","elapsed_ms":90},"candidate":{"outcome":"budget_cancelled","elapsed_ms":120000}});
        }
        let summary = summarize(&profiles).unwrap();
        assert!(summary.contains("budget_cancelled"));
        assert!(summary.contains("cancelled work is not a completed scan or a speedup"));
        profiles[0].as_object_mut().unwrap().remove("whole_root");
        assert!(summarize(&profiles).is_err());
    }
    #[test]
    fn requires_six_unique_native_targets_and_equivalent_coverage() {
        let valid = profiles();
        let summary = summarize(&valid).unwrap();
        assert_eq!(summary.matches("0.500").count(), 12);
        assert!(summarize(&valid[..5]).is_err());
        let mut duplicate = valid.clone();
        duplicate[0] = duplicate[1].clone();
        assert!(summarize(&duplicate).is_err());
        let mut unequal = valid.clone();
        unequal[0]["fixtures"][0]["equivalent_fixture_coverage"] = json!(false);
        assert!(summarize(&unequal).is_err());
        let mut bad_time = valid;
        bad_time[0]["fixtures"][0]["candidate"]["median_ms"] = json!(0);
        assert!(summarize(&bad_time).is_err());
    }
}
