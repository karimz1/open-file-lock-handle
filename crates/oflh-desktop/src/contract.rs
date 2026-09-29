//! Explicit, versioned-by-application IPC values. Native identities never use JS numbers.
use oflh_core::{Error, Identity};
use serde::{Deserialize, Serialize};

/// A stable key for a captured process lifetime, including subsecond birth time.
pub fn identity_key(identity: Identity) -> String {
    format!(
        "{}:{}:{}",
        identity.pid, identity.started, identity.started_sub
    )
}

/// Structured desktop failure with the original native error code when present.
#[derive(Clone, Debug, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    /// Stable error category for UI handling.
    pub kind: String,
    /// Human-readable, display-sanitized operation context.
    pub message: String,
    /// Original native OS error code, when available.
    pub os_code: Option<i32>,
    /// Detailed error chain for user-requested diagnostics.
    pub details: Option<String>,
}
pub(crate) fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut lines = Vec::new();
    let mut source = Some(error);
    while let Some(error) = source {
        lines.push(error.to_string());
        source = error.source();
    }
    lines.join("\nCaused by: ")
}
pub(crate) fn safe_diagnostic(value: &str) -> String {
    value
        .split('\n')
        .map(oflh_core::safe)
        .collect::<Vec<_>>()
        .join("\n")
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        let (kind, os_code) = match &error {
            Error::Changed => ("identity_changed", None),
            Error::Protected => ("protected", None),
            Error::Cancelled => ("cancelled", None),
            Error::Unavailable(_) => ("unavailable", None),
            Error::Io { source, .. } => (
                match source.kind() {
                    std::io::ErrorKind::PermissionDenied => "permission_denied",
                    std::io::ErrorKind::NotFound => "not_found",
                    _ => "io",
                },
                source.raw_os_error(),
            ),
        };
        Self {
            kind: kind.into(),
            message: oflh_core::safe(&error.to_string()),
            os_code,
            details: Some(safe_diagnostic(&error_chain(&error))),
        }
    }
}
impl Failure {
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: "invalid_request".into(),
            message: message.into(),
            os_code: None,
            details: None,
        }
    }
}

/// Small scan notification. The frontend fetches only the visible result page.
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    /// Latest scan request sequence, including cancellations.
    pub generation: u32,
    /// Generation of the displayed immutable snapshot.
    pub revision: u32,
    /// Whether a scan is queued or running.
    pub scanning: bool,
    /// Sanitized display path of the last successful target.
    pub target: String,
    /// Number of observed process lifetimes.
    pub processes: usize,
    /// Total local TCP listeners and UDP bindings, independent of the file target.
    pub ports: usize,
    /// Number of target-matching file observations.
    pub usages: usize,
    /// Coverage limitations supplied by the native backend.
    pub warnings: Vec<String>,
    /// Failure for this operation, if any.
    pub error: Option<Failure>,
    /// Cargo workspace application version.
    pub version: &'static str,
}

/// A backend query over the loaded snapshot, not a new system scan.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TableQuery {
    /// Independent filters on displayed columns, applied before paging and selection.
    pub columns: ColumnFilters,
    /// Shared Rust search expression.
    pub text: String,
    /// Captured lifetime key used for selection or optional query scope.
    pub process_key: Option<String>,
    /// Requested ordering.
    pub sort: Sort,
    /// Reverse the selected ordering.
    pub descending: bool,
    /// Return one row per matching file observation.
    pub handles: bool,
    /// Query local ports using the shared port matcher.
    pub ports: bool,
    /// Restrict ports to processes also referencing the current file target.
    pub ports_path_only: bool,
    /// Require additional lock or sharing-conflict evidence.
    pub locks_only: bool,
    /// Zero-based first result to return.
    pub offset: usize,
    /// Requested page size; Rust clamps it to 1 through 200.
    pub limit: usize,
}
/// Optional column predicates. Unknown metrics never satisfy a numeric bound.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ColumnFilters {
    /// Shared text matcher restricted to the process name.
    pub name: String,
    /// Exact observed PID (query only, never an action identity).
    pub pid: Option<u32>,
    /// Shared text matcher restricted to the full displayed path or local address.
    pub path: String,
    /// Minimum total-capacity CPU percentage, inclusive.
    pub cpu_min: Option<f64>,
    /// Maximum total-capacity CPU percentage, inclusive.
    pub cpu_max: Option<f64>,
    /// Minimum resident memory in MiB, inclusive.
    pub memory_min: Option<f64>,
    /// Maximum resident memory in MiB, inclusive.
    pub memory_max: Option<f64>,
    /// Evidence category, distinct from an open handle alone.
    pub evidence: EvidenceFilter,
    /// Shared text matcher on access/relation or port protocol/state.
    pub access: String,
}
/// Evidence filter for file observations.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFilter {
    /// No evidence restriction.
    #[default]
    Any,
    /// Additional lock or sharing-conflict evidence is present.
    Present,
    /// A kernel lock was observed.
    Kernel,
    /// Sharing-conflict evidence without verified ownership.
    Sharing,
    /// No lock evidence was observed (not proof of no lock).
    None,
}
/// Ordering for a loaded snapshot query.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    /// Shared query ranking, highest score first.
    Relevance,
    /// Observed process name.
    Name,
    /// Numeric process identifier.
    Pid,
    /// Native path ordering.
    Path,
    /// Observed resident bytes.
    Memory,
    /// Observed total-machine CPU percentage.
    Cpu,
    /// Numeric local port.
    Port,
    /// TCP or UDP protocol.
    Protocol,
    /// Native local interface address.
    Address,
}

/// A table row; path references are resolved against its snapshot revision.
#[derive(Clone, Debug, Serialize)]
pub struct Row {
    /// Stable lifetime or row key; native birth counters remain strings.
    pub key: String,
    /// Captured lifetime key used for selection or optional query scope.
    pub process_key: String,
    /// Sanitized process name.
    pub name: String,
    /// Observed PID; never sufficient on its own for an action.
    pub pid: u32,
    /// Sanitized observed account name.
    pub user: String,
    /// Sanitized display path, never used as an action input.
    pub path: String,
    /// Opaque snapshot path reference for native actions.
    pub path_ref: String,
    /// Observed association with the target file.
    pub relation: String,
    /// Observed access mode; unknown remains explicit.
    pub access: String,
    /// Additional evidence, including platform ownership uncertainty.
    pub evidence: Option<String>,
    /// Concise evidence category preserving Windows ownership uncertainty.
    pub evidence_label: Option<String>,
    /// Whether the observed path has been unlinked.
    pub deleted: bool,
    /// Resident bytes, or null when unavailable.
    pub memory: Option<u64>,
    /// Percentage of total CPU capacity, or null without valid samples.
    pub cpu: Option<f64>,
    /// Number of target-matching file observations.
    pub usages: usize,
    /// Whether intrinsic protected-process guards permit confirmation.
    pub actionable: bool,
    /// Socket observation for port-table rows.
    pub port: Option<PortView>,
}
/// A bounded window into the sorted, filtered snapshot.
#[derive(Clone, Debug, Serialize)]
pub struct Page {
    /// Generation of the displayed immutable snapshot.
    pub revision: u32,
    /// Total number of rows matching this query.
    pub total: usize,
    /// Only the requested result window.
    pub rows: Vec<Row>,
}
/// Display text paired with an opaque native path reference.
#[derive(Clone, Debug, Serialize)]
pub struct PathValue {
    /// Sanitized display text, potentially lossy for non-Unicode paths.
    pub display: String,
    /// Opaque reference that resolves to a native PathBuf.
    pub reference: String,
}
/// Read-only captured ancestor; its lifetime key is never reconstructed from a PID.
#[derive(Clone, Debug, Serialize)]
pub struct AncestorView {
    /// Whether the captured identity passes protected-process guards.
    pub actionable: bool,
    /// Sanitized process name.
    pub name: String,
    /// Observed PID; never sufficient on its own for an action.
    pub pid: u32,
    /// Stable lifetime or row key; native birth counters remain strings.
    pub key: String,
}
/// On-demand metadata for a selected process lifetime.
#[derive(Clone, Debug, Serialize)]
pub struct Details {
    /// Selected process summary.
    pub process: Row,
    /// Executable image path, when available.
    pub executable: PathValue,
    /// Working directory path, when available.
    pub cwd: PathValue,
    /// Nearest ancestor first, with captured birth identities.
    pub ancestors: Vec<AncestorView>,
    /// Number of captured local bindings owned by this lifetime.
    pub ports: usize,
    /// Whether an identity-checked owner-folder inspection can be requested.
    pub can_inspect_folder: bool,
}
/// A process displayed in a destructive-action confirmation.
#[derive(Clone, Debug, Serialize)]
pub struct ActionTarget {
    /// Stable lifetime or row key; native birth counters remain strings.
    pub key: String,
    /// Sanitized process name.
    pub name: String,
    /// Observed PID; never sufficient on its own for an action.
    pub pid: u32,
}
/// Single-use confirmation receipt. The Rust service retains the actual identities.
#[derive(Clone, Debug, Serialize)]
pub struct Confirmation {
    /// Single-use confirmation receipt; identities and mode stay in Rust.
    pub ticket: String,
    /// Explicit force mode captured when preparing the confirmation.
    pub force: bool,
    /// All confirmed processes, including any hidden by filters.
    pub targets: Vec<ActionTarget>,
}
/// Result of a request and a bounded check of its captured process lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    /// The original process is confirmed no longer running after the request.
    Exited,
    /// The original process remains running after the verification deadline.
    StillRunning,
    /// The request was sent, but process state could not be checked.
    Unverified,
    /// The request failed; no success is inferred.
    Failed,
}
/// Outcome for one captured process.
#[derive(Clone, Debug, Serialize)]
pub struct ActionResult {
    /// Verified outcome; sending a request alone does not prove exit.
    pub outcome: ActionOutcome,
    /// Observed PID; never sufficient on its own for an action.
    pub pid: u32,
    /// Failure for this operation, if any.
    pub error: Option<Failure>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_keys_preserve_full_native_counters() {
        let identity = Identity {
            pid: 42,
            started: u64::MAX,
            started_sub: 999999,
        };
        assert_eq!(identity_key(identity), "42:18446744073709551615:999999");
        assert_ne!(
            identity_key(identity),
            identity_key(Identity {
                started: u64::MAX - 1,
                ..identity
            })
        );
    }
    #[test]
    fn failures_keep_category_context_and_os_code() {
        let source = std::io::Error::from_raw_os_error(2);
        let failure = Failure::from(oflh_core::io("inspect target", source));
        assert_eq!(failure.os_code, Some(2));
        assert_eq!(failure.kind, "not_found");
        assert!(failure.message.contains("inspect target"));
        assert_eq!(Failure::from(Error::Changed).kind, "identity_changed");
        assert_eq!(Failure::from(Error::Protected).kind, "protected");
        let source = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "fixture");
        assert_eq!(
            Failure::from(oflh_core::io("scan", source)).kind,
            "permission_denied"
        );
    }
    #[test]
    fn ipc_query_rejects_unknown_fields_and_sort_modes() {
        assert!(serde_json::from_str::<TableQuery>(r#"{"sort":"pid","limit":200}"#).is_ok());
        assert!(serde_json::from_str::<TableQuery>(r#"{"sort":"shell"}"#).is_err());
        assert!(serde_json::from_str::<TableQuery>(r#"{"identity":42}"#).is_err());
    }
}

/// A local binding, not a claim of external network reachability.
#[derive(Clone, Debug, Serialize)]
pub struct PortView {
    /// TCP or UDP.
    pub protocol: &'static str,
    /// LISTEN for TCP; BOUND for UDP.
    pub state: &'static str,
    /// IPv4 or IPv6 interface address.
    pub address: String,
    /// Local port number.
    pub number: u16,
    /// Endpoint with IPv6 brackets where necessary.
    pub endpoint: String,
    /// Opaque snapshot reference for copying the exact binding.
    pub reference: String,
}
