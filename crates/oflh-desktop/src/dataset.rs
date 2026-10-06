//! Immutable native snapshot and cached search fields. Display paths are never action inputs.
use crate::contract::*;
use oflh_core::{
    Process, Snapshot, safe,
    search::{ProcessIndex, Query, Scratch},
};
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

type RowMatch = (usize, Option<usize>, u32);
struct CachedQuery {
    query: TableQuery,
    rows: Arc<[RowMatch]>,
}

/// Snapshot-owned search cache. Cloning the surrounding Arc is cheap for IPC commands.
pub struct Dataset {
    /// Generation that owns every path reference in this dataset.
    pub revision: u32,
    /// Native observations retained losslessly.
    pub snapshot: Snapshot,
    indices: Vec<ProcessIndex>,
    port_indices: Vec<Vec<oflh_core::ports::PortIndex>>,
    identities: std::collections::HashMap<String, usize>,
    query_cache: Mutex<Option<CachedQuery>>,
    counts: (usize, usize, usize),
}
impl Dataset {
    /// Build an immutable snapshot and its reusable indices.
    pub fn new(revision: u32, snapshot: Snapshot) -> Self {
        let indices = snapshot.processes.iter().map(ProcessIndex::new).collect();
        let identities = snapshot
            .processes
            .iter()
            .enumerate()
            .map(|(index, process)| (identity_key(process.identity), index))
            .collect();
        let port_indices = snapshot
            .processes
            .iter()
            .map(|process| {
                process
                    .ports
                    .iter()
                    .map(|port| oflh_core::ports::PortIndex::new(process, port))
                    .collect()
            })
            .collect();
        let counts = (
            snapshot
                .processes
                .iter()
                .filter(|process| !process.usages.is_empty())
                .count(),
            snapshot
                .processes
                .iter()
                .map(|process| process.ports.len())
                .sum(),
            snapshot
                .processes
                .iter()
                .map(|process| process.usages.len())
                .sum(),
        );
        Self {
            query_cache: Mutex::new(None),
            counts,
            port_indices,
            identities,
            revision,
            snapshot,
            indices,
        }
    }
    /// File users in the accepted snapshot, computed once during indexing.
    pub fn file_users(&self) -> usize {
        self.counts.0
    }
    /// Local bindings in the accepted snapshot.
    pub fn port_count(&self) -> usize {
        self.counts.1
    }
    /// Target-matching observations in the accepted snapshot.
    pub fn usage_count(&self) -> usize {
        self.counts.2
    }
    /// One snapshot-local query cache bounds memory while reusing search/sort
    /// results across viewport pages, selection and content-width measurements.
    fn cached_matches(&self, request: &TableQuery) -> Result<Arc<[RowMatch]>, Failure> {
        let mut query = request.clone();
        query.offset = 0;
        query.limit = 0;
        let mut cache = self
            .query_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(cached) = cache.as_ref().filter(|cached| cached.query == query) {
            return Ok(cached.rows.clone());
        }
        let rows: Arc<[RowMatch]> = self.matched(&query)?.into();
        *cache = Some(CachedQuery {
            query,
            rows: rows.clone(),
        });
        Ok(rows)
    }
    /// Resolve a captured lifetime key without accepting a PID-only substitute.
    pub fn process(&self, key: &str) -> Result<(usize, &Process), Failure> {
        let index = self
            .identities
            .get(key)
            .copied()
            .ok_or_else(|| Failure::from(oflh_core::Error::Changed))?;
        Ok((index, &self.snapshot.processes[index]))
    }
    fn matched(&self, request: &TableQuery) -> Result<Vec<(usize, Option<usize>, u32)>, Failure> {
        let filters = &request.columns;
        for text in [&filters.name, &filters.path, &filters.access] {
            if text.len() > 4096 {
                return Err(Failure::invalid(
                    "Column searches are limited to 4096 bytes",
                ));
            }
        }
        for (minimum, maximum) in [
            (filters.cpu_min, filters.cpu_max),
            (filters.memory_min, filters.memory_max),
        ] {
            if minimum
                .into_iter()
                .chain(maximum)
                .any(|value| !value.is_finite() || value < 0.0)
                || minimum.zip(maximum).is_some_and(|(low, high)| low > high)
            {
                return Err(Failure::invalid(
                    "Filter bounds must be non-negative, with minimum no greater than maximum",
                ));
            }
        }
        let mut matches = self.matched_search(request)?;
        if *filters == ColumnFilters::default() {
            return Ok(matches);
        }
        let name = Query::new(&filters.name);
        let path = Query::new(&filters.path);
        let access = Query::new(&filters.access);
        let mut scratch = Scratch::default();
        matches.retain(|&(process, usage, _)| {
            let row = if request.ports {
                self.port_row(process, usage.unwrap_or_default())
            } else {
                self.row(process, usage)
            };
            let path_text = row
                .port
                .as_ref()
                .map_or(row.path.as_str(), |port| port.address.as_str());
            let access_text = row.port.as_ref().map_or_else(
                || format!("{} {}", row.relation, row.access),
                |port| format!("{} {}", port.protocol, port.state),
            );
            let text_matches = |query: &Query, text: &str, scratch: &mut Scratch| {
                query.is_empty()
                    || query.matches(&[oflh_core::search::Field::new(text, false)], scratch)
            };
            let owner = &self.snapshot.processes[process];
            let evidence = if request.ports {
                None
            } else if let Some(usage) = usage {
                owner.usages[usage].lock.as_ref()
            } else {
                owner.usages.iter().find_map(|usage| usage.lock.as_ref())
            };
            let evidence_matches = match filters.evidence {
                EvidenceFilter::Any => true,
                EvidenceFilter::Present => evidence.is_some(),
                EvidenceFilter::None => evidence.is_none(),
                EvidenceFilter::Kernel => {
                    matches!(evidence, Some(oflh_core::LockEvidence::Kernel(_)))
                }
                EvidenceFilter::Sharing => {
                    matches!(evidence, Some(oflh_core::LockEvidence::SharingConflict(_)))
                }
            };
            filters.pid.is_none_or(|pid| row.pid == pid)
                && within_bounds(row.cpu, filters.cpu_min, filters.cpu_max)
                && within_bounds(
                    row.memory.map(|bytes| bytes as f64 / 1048576.0),
                    filters.memory_min,
                    filters.memory_max,
                )
                && evidence_matches
                && text_matches(&name, &row.name, &mut scratch)
                && text_matches(&path, path_text, &mut scratch)
                && if !request.ports && usage.is_none() && !access.is_empty() {
                    owner.usages.iter().any(|usage| {
                        text_matches(
                            &access,
                            &format!("{} {}", usage.relation.label(), usage.access.label()),
                            &mut scratch,
                        )
                    })
                } else {
                    text_matches(&access, &access_text, &mut scratch)
                }
        });
        Ok(matches)
    }
    /// Search in Rust using exactly the shared word-boundary/wildcard matcher.
    fn matched_search(
        &self,
        request: &TableQuery,
    ) -> Result<Vec<(usize, Option<usize>, u32)>, Failure> {
        if request.text.len() > 4096 {
            return Err(Failure::invalid("Search is limited to 4096 bytes"));
        }
        if request.ports {
            return Ok(self.matched_ports(request));
        }
        let query = Query::new(&request.text);
        let mut scratch = Scratch::default();
        let mut matches = Vec::new();
        for (process_index, index) in self.indices.iter().enumerate() {
            let process = &self.snapshot.processes[process_index];
            if process.usages.is_empty() {
                continue;
            }
            if request
                .process_key
                .as_ref()
                .is_some_and(|key| *key != identity_key(process.identity))
            {
                continue;
            }
            if !index.matches(&query, &mut scratch) {
                continue;
            }
            // As in the TUI, metadata may satisfy terms, but all remaining terms
            // must match one observation rather than unrelated paths in the same process.
            let file_query = query.file_terms(&index.metadata, &mut scratch);
            for (usage_index, fields) in index.usages.iter().enumerate() {
                if request.locks_only && process.usages[usage_index].lock.is_none() {
                    continue;
                }
                if file_query.matches(fields, &mut scratch) {
                    if !request.handles {
                        matches.push((process_index, None, index.score(&query, &mut scratch)));
                        break;
                    }
                    let score = query
                        .score(fields, &mut scratch)
                        .max(query.score(&index.metadata, &mut scratch));
                    matches.push((process_index, Some(usage_index), score));
                }
            }
        }
        matches.sort_by(|left, right| {
            let first = &self.snapshot.processes[left.0];
            let second = &self.snapshot.processes[right.0];
            let ordering = match request.sort {
                Sort::Relevance => right.2.cmp(&left.2),
                Sort::Port | Sort::Protocol | Sort::Address => Ordering::Equal,
                Sort::Name => first.name.cmp(&second.name),
                Sort::Pid => first.identity.pid.cmp(&second.identity.pid),
                Sort::Path => row_path(first, left.1).cmp(row_path(second, right.1)),
                Sort::Memory => first.memory.cmp(&second.memory),
                Sort::Cpu => first
                    .cpu
                    .partial_cmp(&second.cpu)
                    .unwrap_or(Ordering::Equal),
            };
            let ordering = if request.descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering
                .then_with(|| left.0.cmp(&right.0))
                .then_with(|| left.1.cmp(&right.1))
        });
        Ok(matches)
    }
    fn matched_ports(&self, request: &TableQuery) -> Vec<(usize, Option<usize>, u32)> {
        let query = oflh_core::ports::PortQuery::new(&request.text);
        let mut scratch = Scratch::default();
        let mut matches = Vec::new();
        for (process_index, process) in self.snapshot.processes.iter().enumerate() {
            if request.ports_path_only && process.usages.is_empty() {
                continue;
            }
            if request
                .process_key
                .as_ref()
                .is_some_and(|key| *key != identity_key(process.identity))
            {
                continue;
            }
            for (port_index, port) in process.ports.iter().enumerate() {
                if query.matches(
                    port,
                    &self.port_indices[process_index][port_index],
                    &mut scratch,
                ) {
                    matches.push((process_index, Some(port_index), 0));
                }
            }
        }
        matches.sort_by(|left, right| {
            let first = &self.snapshot.processes[left.0];
            let second = &self.snapshot.processes[right.0];
            let first_port = &first.ports[left.1.unwrap_or_default()];
            let second_port = &second.ports[right.1.unwrap_or_default()];
            let ordering = match request.sort {
                Sort::Relevance | Sort::Port => first_port.number.cmp(&second_port.number),
                Sort::Protocol => first_port.protocol.cmp(&second_port.protocol),
                Sort::Address => first_port.address.cmp(&second_port.address),
                Sort::Name => first.name.cmp(&second.name),
                Sort::Pid => first.identity.pid.cmp(&second.identity.pid),
                Sort::Path => first.executable.cmp(&second.executable),
                Sort::Memory => first.memory.cmp(&second.memory),
                Sort::Cpu => first
                    .cpu
                    .partial_cmp(&second.cpu)
                    .unwrap_or(Ordering::Equal),
            };
            let ordering = if request.descending {
                ordering.reverse()
            } else {
                ordering
            };
            ordering
                .then_with(|| left.0.cmp(&right.0))
                .then_with(|| left.1.cmp(&right.1))
        });
        matches
    }
    fn port_row(&self, process_index: usize, port_index: usize) -> Row {
        let mut row = self.row(process_index, None);
        let port = &self.snapshot.processes[process_index].ports[port_index];
        let endpoint = std::net::SocketAddr::new(port.address, port.number).to_string();
        row.key = format!("{}/{}-{endpoint}", row.process_key, port.protocol.label());
        row.port = Some(PortView {
            protocol: port.protocol.label(),
            state: port.protocol.state(),
            address: port.address.to_string(),
            number: port.number,
            endpoint,
            reference: self.path_ref(process_index, &format!("port-{port_index}")),
        });
        row
    }
    fn port(&self, reference: &str) -> Result<&oflh_core::Port, Failure> {
        let parts: Vec<_> = reference.split(':').collect();
        if parts.len() != 3 || parts[0].parse::<u32>().ok() != Some(self.revision) {
            return Err(Failure::invalid(
                "This binding belongs to an older snapshot",
            ));
        }
        let process = parts[1]
            .parse::<usize>()
            .ok()
            .and_then(|index| self.snapshot.processes.get(index));
        let index = parts[2]
            .strip_prefix("port-")
            .and_then(|value| value.parse::<usize>().ok());
        process
            .zip(index)
            .and_then(|(process, index)| process.ports.get(index))
            .ok_or_else(|| Failure::invalid("Unknown local binding"))
    }
    /// Return unique process lifetime keys matching the full query, capped at 10,000.
    pub fn keys(&self, request: &TableQuery) -> Result<Vec<String>, Failure> {
        let mut seen = std::collections::HashSet::new();
        let keys: Vec<_> = self
            .cached_matches(request)?
            .iter()
            .map(|(index, _, _)| identity_key(self.snapshot.processes[*index].identity))
            .filter(|key| seen.insert(key.clone()))
            .collect();
        if keys.len() > 10000 {
            return Err(Failure::invalid(
                "Narrow the search to select at most 10,000 processes",
            ));
        }
        Ok(keys)
    }
    /// Return at most 200 sorted rows; the full native snapshot remains in Rust.
    pub fn page(&self, request: &TableQuery) -> Result<Page, Failure> {
        let matches = self.cached_matches(request)?;
        let total = matches.len();
        let rows = matches
            .iter()
            .copied()
            .skip(request.offset)
            .take(request.limit.clamp(1, 200))
            .map(|(process, usage, _)| {
                if request.ports {
                    self.port_row(process, usage.unwrap_or_default())
                } else {
                    self.row(process, usage)
                }
            })
            .collect();
        Ok(Page {
            revision: self.revision,
            total,
            rows,
        })
    }
    /// Build a display row from validated internal indices.
    fn row(&self, process_index: usize, usage_index: Option<usize>) -> Row {
        let process = &self.snapshot.processes[process_index];
        let process_key = identity_key(process.identity);
        let usage = usage_index.and_then(|index| process.usages.get(index));
        let evidence = if usage_index.is_some() {
            usage.and_then(|value| value.lock.as_ref())
        } else {
            process.usages.iter().find_map(|value| value.lock.as_ref())
        };
        let path_index = usage_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "exe".into());
        Row {
            port: None,
            key: format!("{process_key}/{path_index}"),
            process_key,
            name: safe(&process.name),
            pid: process.identity.pid,
            user: safe(&process.user),
            path: display(row_path(process, usage_index)),
            path_ref: self.path_ref(process_index, &path_index),
            relation: usage
                .map(|value| value.relation.label())
                .unwrap_or("process")
                .into(),
            access: usage
                .map(|value| value.access.label())
                .unwrap_or("unknown")
                .into(),
            evidence: evidence.map(|value| safe(&value.to_string())),
            evidence_label: evidence.map(|value| {
                match value {
                    oflh_core::LockEvidence::Kernel(_) => "Kernel lock",
                    oflh_core::LockEvidence::SharingConflict(_) => {
                        "Sharing conflict; owner unverified"
                    }
                }
                .into()
            }),
            deleted: usage.is_some_and(|value| value.deleted),
            memory: process.memory,
            cpu: process.cpu,
            usages: process.usages.len(),
            actionable: process.identity.validate().is_ok(),
        }
    }
    fn path_ref(&self, process: usize, kind: &str) -> String {
        format!("{}:{process}:{kind}", self.revision)
    }
    /// Return metadata without serializing the entire handle collection.
    pub fn details(&self, key: &str) -> Result<Details, Failure> {
        let (index, process) = self.process(key)?;
        Ok(Details {
            ports: process.ports.len(),
            can_inspect_folder: process.inspection_folder().is_some(),
            process: self.row(index, None),
            executable: PathValue {
                display: display(&process.executable),
                reference: self.path_ref(index, "exe"),
            },
            cwd: PathValue {
                display: display(&process.cwd),
                reference: self.path_ref(index, "cwd"),
            },
            ancestors: process
                .ancestors
                .iter()
                .map(|ancestor| AncestorView {
                    actionable: ancestor.identity.validate().is_ok(),
                    name: safe(&ancestor.name),
                    pid: ancestor.identity.pid,
                    key: identity_key(ancestor.identity),
                })
                .collect(),
        })
    }
    /// Copy original text, never the sanitized presentation. Reject lossy path conversion.
    pub fn copy_text(
        &self,
        keys: &[String],
        field: &str,
        reference: Option<&str>,
    ) -> Result<String, Failure> {
        if matches!(field, "port" | "endpoint") {
            let binding = self
                .port(reference.ok_or_else(|| Failure::invalid("Select a local binding first"))?)?;
            return Ok(if field == "port" {
                binding.number.to_string()
            } else {
                std::net::SocketAddr::new(binding.address, binding.number).to_string()
            });
        }
        if matches!(field, "path" | "filename") {
            let path =
                self.path(reference.ok_or_else(|| Failure::invalid("Select a path first"))?)?;
            let value = if field == "filename" {
                path.file_name()
                    .ok_or_else(|| Failure::invalid("No filename is available"))?
            } else {
                path.as_os_str()
            };
            return value.to_str().map(str::to_owned).ok_or_else(|| {
                Failure::invalid(
                    "This native path cannot be represented losslessly as clipboard text",
                )
            });
        }
        if keys.is_empty() || keys.len() > 10000 {
            return Err(Failure::invalid("Select between 1 and 10,000 processes"));
        }
        let mut lines = Vec::new();
        for key in keys {
            let (_, process) = self.process(key)?;
            lines.push(match field {
                "pid" => process.identity.pid.to_string(),
                "name" => process.name.clone(),
                "rows" => format!(
                    "{}\t{}\t{}\t{}",
                    process.name,
                    process.identity.pid,
                    process.user,
                    process.executable.to_str().ok_or_else(|| Failure::invalid(
                        "An executable path cannot be represented losslessly as clipboard text"
                    ))?
                ),
                _ => return Err(Failure::invalid("Unknown copy field")),
            });
        }
        let text = lines.join("\n");
        if text.len() > 8 * 1024 * 1024 {
            return Err(Failure::invalid(
                "Selection exceeds the 8 MB clipboard limit",
            ));
        }
        Ok(text)
    }
    /// Reject stale/forged references; preserve non-Unicode native paths without round trips.
    pub fn path(&self, reference: &str) -> Result<PathBuf, Failure> {
        let parts: Vec<_> = reference.split(':').collect();
        if parts.len() != 3 || parts[0].parse::<u32>().ok() != Some(self.revision) {
            return Err(Failure::invalid(
                "This path belongs to an older snapshot; select it again",
            ));
        }
        let process = parts[1]
            .parse::<usize>()
            .ok()
            .and_then(|index| self.snapshot.processes.get(index))
            .ok_or_else(|| Failure::invalid("Unknown path reference"))?;
        let path = match parts[2] {
            "exe" => &process.executable,
            "cwd" => &process.cwd,
            index => {
                &process
                    .usages
                    .get(
                        index
                            .parse::<usize>()
                            .map_err(|_| Failure::invalid("Unknown path reference"))?,
                    )
                    .ok_or_else(|| Failure::invalid("Unknown path reference"))?
                    .path
            }
        };
        if path.as_os_str().is_empty() {
            return Err(Failure::invalid("Path is unavailable"));
        }
        Ok(path.clone())
    }
}
fn within_bounds(value: Option<f64>, minimum: Option<f64>, maximum: Option<f64>) -> bool {
    if minimum.is_none() && maximum.is_none() {
        return true;
    }
    value.is_some_and(|value| {
        minimum.is_none_or(|minimum| value >= minimum)
            && maximum.is_none_or(|maximum| value <= maximum)
    })
}
fn row_path(process: &Process, usage: Option<usize>) -> &Path {
    usage
        .and_then(|index| process.usages.get(index))
        .map(|usage| usage.path.as_path())
        .unwrap_or(&process.executable)
}
pub(crate) fn display(path: &Path) -> String {
    safe(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oflh_core::{Identity, LockEvidence, Relation, Usage};
    fn fixture() -> Dataset {
        Dataset::new(
            7,
            Snapshot {
                processes: vec![Process {
                    identity: Identity {
                        pid: 42,
                        started: u64::MAX,
                        started_sub: 0,
                    },
                    name: "FixtureWorker\u{202e}".into(),
                    user: "fixture".into(),
                    executable: PathBuf::from("/fixture/worker"),
                    usages: vec![
                        Usage {
                            path: PathBuf::from("/fixture/FileLockExampleCli.dll"),
                            ..Usage::default()
                        },
                        Usage {
                            path: PathBuf::from("/fixture/other.bin"),
                            lock: Some(LockEvidence::Kernel("POSIX WRITE".into())),
                            relation: Relation::Locked,
                            ..Usage::default()
                        },
                    ],
                    ..Process::default()
                }],
                warnings: vec![],
            },
        )
    }
    #[test]
    fn column_filters_disambiguate_shared_paths_and_bound_unknown_metrics() {
        let mut snapshot = fixture().snapshot;
        let mut second = snapshot.processes[0].clone();
        second.name = "OtherWorker".into();
        second.identity.pid += 1;
        snapshot.processes[0].memory = Some(64 * 1048576);
        snapshot.processes[0].cpu = Some(2.5);
        snapshot.processes.push(second);
        let dataset = Dataset::new(7, snapshot);
        // Both processes reference the same path: global search correctly keeps both.
        let mut request = TableQuery {
            text: "FileLockExample".into(),
            ..TableQuery::default()
        };
        assert_eq!(dataset.page(&request).unwrap().total, 2);
        request.columns.name = "Fixture*".into();
        assert_eq!(dataset.page(&request).unwrap().total, 1);
        assert_eq!(dataset.keys(&request).unwrap().len(), 1);
        request.columns.name.clear();
        request.columns.memory_min = Some(64.0);
        request.columns.memory_max = Some(64.0);
        request.columns.cpu_min = Some(2.5);
        request.columns.cpu_max = Some(2.5);
        assert_eq!(dataset.page(&request).unwrap().total, 1);
        request.columns.memory_min = Some(65.0);
        assert!(dataset.page(&request).is_err());
        request.columns = ColumnFilters {
            cpu_min: Some(0.0),
            ..ColumnFilters::default()
        };
        assert_eq!(dataset.page(&request).unwrap().total, 1); // Unknown is not zero.
        request.columns = ColumnFilters {
            evidence: EvidenceFilter::Kernel,
            path: "other.bin".into(),
            ..ColumnFilters::default()
        };
        request.text.clear();
        request.handles = true;
        assert_eq!(dataset.page(&request).unwrap().total, 2);
        request.columns.evidence = EvidenceFilter::None;
        assert_eq!(dataset.page(&request).unwrap().total, 0);
        request.columns = ColumnFilters::default();
        request.process_key = Some("999999:123:0".into());
        assert_eq!(dataset.page(&request).unwrap().total, 0);
        request.ports = true;
        assert_eq!(dataset.page(&request).unwrap().total, 0);
    }
    #[test]
    fn shared_search_applies_remaining_terms_to_individual_usages() {
        let dataset = fixture();
        let request = TableQuery {
            text: "FixtureWorker FLEC*dll".into(),
            handles: true,
            limit: 200,
            ..TableQuery::default()
        };
        let page = dataset.page(&request).unwrap();
        assert_eq!(page.total, 1);
        assert!(page.rows[0].path.ends_with("FileLockExampleCli.dll"));
        assert_eq!(page.rows[0].name, "FixtureWorker�");
        assert_eq!(page.rows[0].memory, None);
        assert!(serde_json::to_value(&page).unwrap()["rows"][0]["memory"].is_null());
        let request = TableQuery {
            locks_only: true,
            ..TableQuery::default()
        };
        assert_eq!(
            dataset.page(&request).unwrap().rows[0].evidence.as_deref(),
            Some("POSIX WRITE")
        );
    }
    #[test]
    fn process_queries_do_not_join_terms_across_unrelated_file_usages() {
        let dataset = fixture();
        let page = dataset
            .page(&TableQuery {
                text: "FileLockExampleCli.dll other.bin".into(),
                ..TableQuery::default()
            })
            .unwrap();
        assert_eq!(page.total, 0);
        let page = dataset
            .page(&TableQuery {
                text: "FixtureWorker FLEC*dll".into(),
                ..TableQuery::default()
            })
            .unwrap();
        assert_eq!(page.total, 1);
    }
    #[test]
    fn path_references_reject_stale_and_invalid_indices() {
        let dataset = fixture();
        assert_eq!(
            dataset.path("7:0:0").unwrap(),
            PathBuf::from("/fixture/FileLockExampleCli.dll")
        );
        for reference in ["6:0:0", "7:9:0", "7:0:999", "7:0:../../file", "7:0:cwd"] {
            assert!(dataset.path(reference).is_err(), "{reference}");
        }
    }
    #[test]
    fn paging_is_bounded_and_select_all_deduplicates_processes() {
        let mut snapshot = fixture().snapshot;
        let process = snapshot.processes[0].clone();
        snapshot.processes = (10..510)
            .map(|pid| Process {
                identity: Identity {
                    pid,
                    ..process.identity
                },
                ..process.clone()
            })
            .collect();
        let dataset = Dataset::new(8, snapshot);
        let query = TableQuery {
            sort: Sort::Pid,
            offset: 499,
            limit: usize::MAX,
            ..TableQuery::default()
        };
        let page = dataset.page(&query).unwrap();
        assert_eq!(page.total, 500);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].pid, 509);
        assert_eq!(
            dataset
                .page(&TableQuery {
                    offset: 0,
                    ..query.clone()
                })
                .unwrap()
                .rows
                .len(),
            200
        );
        assert_eq!(
            dataset
                .keys(&TableQuery {
                    handles: true,
                    ..query
                })
                .unwrap()
                .len(),
            500
        );
    }
    #[test]
    fn copy_uses_original_text_and_validates_all_keys() {
        let dataset = fixture();
        let key = identity_key(dataset.snapshot.processes[0].identity);
        assert_eq!(
            dataset.copy_text(&[key], "name", None).unwrap(),
            "FixtureWorker\u{202e}"
        );
        assert!(dataset.copy_text(&["42:1:0".into()], "pid", None).is_err());
        assert_eq!(
            dataset.copy_text(&[], "filename", Some("7:0:0")).unwrap(),
            "FileLockExampleCli.dll"
        );
    }
    #[cfg(unix)]
    #[test]
    fn non_unicode_paths_are_retained_and_never_copied_lossily() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"/fixture/\xff".to_vec()));
        let mut snapshot = fixture().snapshot;
        snapshot.processes[0].usages[0].path = path.clone();
        let dataset = Dataset::new(9, snapshot);
        assert_eq!(dataset.path("9:0:0").unwrap(), path);
        assert!(dataset.copy_text(&[], "path", Some("9:0:0")).is_err());
    }
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
    use oflh_core::{AccessKind, Identity, LockEvidence, Usage};
    #[test]
    fn process_evidence_filter_deduplicates_and_preserves_owner_uncertainty() {
        let process = Process {
            identity: Identity {
                pid: 42,
                started: 1,
                started_sub: 0,
            },
            usages: vec![
                Usage {
                    lock: Some(LockEvidence::SharingConflict(AccessKind::Delete)),
                    ..Usage::default()
                };
                2
            ],
            ..Process::default()
        };
        let dataset = Dataset::new(
            1,
            Snapshot {
                processes: vec![process],
                warnings: vec![],
            },
        );
        let page = dataset
            .page(&TableQuery {
                locks_only: true,
                ..TableQuery::default()
            })
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(
            page.rows[0].evidence_label.as_deref(),
            Some("Sharing conflict; owner unverified")
        );
        assert_eq!(
            dataset
                .page(&TableQuery {
                    locks_only: true,
                    handles: true,
                    ..TableQuery::default()
                })
                .unwrap()
                .total,
            2
        );
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;
    use oflh_core::{Identity, Port, Protocol, Usage};
    fn fixture() -> Dataset {
        let make_port = |number, protocol| Port {
            protocol,
            address: "::1".parse().unwrap(),
            number,
        };
        Dataset::new(
            4,
            Snapshot {
                processes: vec![
                    Process {
                        identity: Identity {
                            pid: 42,
                            started: u64::MAX,
                            started_sub: 0,
                        },
                        name: "server".into(),
                        ports: vec![make_port(80, Protocol::Tcp), make_port(8080, Protocol::Tcp)],
                        usages: vec![Usage::default()],
                        ..Process::default()
                    },
                    Process {
                        name: "owner unavailable".into(),
                        ports: vec![make_port(8080, Protocol::Udp)],
                        ..Process::default()
                    },
                ],
                warnings: vec![],
            },
        )
    }
    #[test]
    fn numeric_fragments_exact_queries_and_scopes_use_shared_port_semantics() {
        let dataset = fixture();
        let query = TableQuery {
            ports: true,
            limit: 200,
            text: "80".into(),
            ..TableQuery::default()
        };
        assert_eq!(dataset.page(&query).unwrap().total, 3);
        assert_eq!(
            dataset
                .page(&TableQuery {
                    text: "port:80".into(),
                    ..query.clone()
                })
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            dataset
                .page(&TableQuery {
                    text: "port:8080 tcp server".into(),
                    ..query.clone()
                })
                .unwrap()
                .total,
            1
        );
        assert_eq!(
            dataset
                .page(&TableQuery {
                    ports_path_only: true,
                    ..query.clone()
                })
                .unwrap()
                .total,
            2
        );
        assert_eq!(
            dataset
                .page(&TableQuery {
                    text: "port:65536".into(),
                    ..query
                })
                .unwrap()
                .total,
            0
        );
        // File tables must not misrepresent port-only owners as target users.
        assert_eq!(dataset.page(&TableQuery::default()).unwrap().total, 1);
    }
    #[test]
    fn ipv6_copy_and_unknown_owner_safety_are_preserved() {
        let dataset = fixture();
        let page = dataset
            .page(&TableQuery {
                ports: true,
                text: "udp".into(),
                ..TableQuery::default()
            })
            .unwrap();
        let row = &page.rows[0];
        assert!(!row.actionable);
        assert_eq!(row.process_key, "0:0:0");
        let port = row.port.as_ref().unwrap();
        assert_eq!(port.state, "BOUND");
        assert_eq!(
            dataset
                .copy_text(&[], "endpoint", Some(&port.reference))
                .unwrap(),
            "[::1]:8080"
        );
        assert!(
            dataset
                .copy_text(&[], "endpoint", Some("3:0:port-0"))
                .is_err()
        );
        assert!(
            !dataset
                .details(&row.process_key)
                .unwrap()
                .can_inspect_folder
        );
    }
}

#[cfg(test)]
mod query_cache_tests {
    use super::*;
    use oflh_core::{Identity, Usage};
    fn large_dataset(revision: u32) -> Dataset {
        Dataset::new(
            revision,
            Snapshot {
                processes: (0..1000)
                    .map(|index| Process {
                        identity: Identity {
                            pid: 4000 + index,
                            started: 10,
                            started_sub: 0,
                        },
                        name: format!("worker-{index:04}"),
                        usages: (0..20)
                            .map(|usage| Usage {
                                path: PathBuf::from(format!(
                                    "/fixture/{index:04}/file-{usage:02}.bin"
                                )),
                                ..Usage::default()
                            })
                            .collect(),
                        ..Process::default()
                    })
                    .collect(),
                warnings: vec![],
            },
        )
    }
    #[test]
    fn pages_and_selection_reuse_the_full_query_without_aliasing_filter_changes() {
        let dataset = large_dataset(1);
        let mut query = TableQuery {
            handles: true,
            sort: Sort::Pid,
            ..TableQuery::default()
        };
        let original = dataset.cached_matches(&query).unwrap();
        assert_eq!(original.len(), 20000);
        query.offset = 199;
        query.limit = 2;
        assert!(Arc::ptr_eq(
            &original,
            &dataset.cached_matches(&query).unwrap()
        ));
        let boundary = dataset.page(&query).unwrap();
        assert_eq!(boundary.total, 20000);
        assert_eq!(boundary.rows.len(), 2);
        assert_eq!(boundary.rows[0].pid, 4009);
        assert_eq!(boundary.rows[1].pid, 4010);
        assert_eq!(dataset.keys(&query).unwrap().len(), 1000);
        assert!(Arc::ptr_eq(
            &original,
            &dataset.cached_matches(&query).unwrap()
        ));
        query.columns.pid = Some(4010);
        let filtered = dataset.cached_matches(&query).unwrap();
        assert_eq!(filtered.len(), 20);
        assert!(!Arc::ptr_eq(&original, &filtered));
        query.columns.pid = None;
        query.descending = true;
        assert_eq!(
            dataset
                .page(&TableQuery { offset: 0, ..query })
                .unwrap()
                .rows[0]
                .pid,
            4999
        );
        let refreshed = large_dataset(2);
        assert!(!Arc::ptr_eq(
            &original,
            &refreshed
                .cached_matches(&TableQuery {
                    handles: true,
                    sort: Sort::Pid,
                    ..TableQuery::default()
                })
                .unwrap()
        ));
        assert_eq!(refreshed.usage_count(), 20000);
    }
    #[test]
    fn cached_queries_do_not_bypass_validation_or_leak_between_ports_and_files() {
        let dataset = large_dataset(1);
        dataset.page(&TableQuery::default()).unwrap();
        let ports = dataset
            .page(&TableQuery {
                ports: true,
                ..TableQuery::default()
            })
            .unwrap();
        assert_eq!(ports.total, 0);
        assert!(
            dataset
                .page(&TableQuery {
                    text: "a".repeat(4097),
                    ..TableQuery::default()
                })
                .is_err()
        );
        assert!(
            dataset
                .page(&TableQuery {
                    columns: ColumnFilters {
                        cpu_min: Some(f64::NAN),
                        ..ColumnFilters::default()
                    },
                    ..TableQuery::default()
                })
                .is_err()
        );
    }
}
