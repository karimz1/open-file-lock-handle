//! Progress counts are observations attempted, not evidence of ownership or completeness.
use std::{
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
    time::Instant,
};

/// Current worker stage. Totals are unknown while native enumeration is running.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum InspectionPhase {
    /// Enumerating process references, descriptors and mappings.
    #[default]
    Processes,
    /// Inspecting file handles or correlating resource users (Windows only).
    Files,
    /// Collecting local port bindings.
    Ports,
    /// Building snapshot search indices.
    Indexing,
}

/// Native work counter; counts include unsuccessful inspection attempts.
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum InspectionCounter {
    /// Process lifetimes visited.
    Processes,
    /// Descriptors, mappings or modules visited; not unique files.
    Resources,
    /// Matching files observed, or files submitted for fallback resource inspection (Windows only).
    Files,
    /// Directories visited (Windows only).
    Directories,
    /// Restart Manager resource queries (Windows only).
    ResourceQueries,
    /// Sum of Restart Manager call durations, in microseconds; parallel calls overlap.
    ResourceQueryMicros,
    /// Module snapshot attempts (Windows only).
    ModuleSnapshots,
    /// Time spent creating module snapshots, in microseconds.
    ModuleSnapshotMicros,
    /// File identity probes (Windows only).
    FileIdentityQueries,
    /// Resource workers started for this inspection (Windows only).
    ResourceWorkers,
    /// Native file-user fallback queries (Windows only).
    NativeFileUserQueries,
    /// Completed native handle snapshots.
    NativeHandleSnapshots,
    /// Native handle snapshot duration in microseconds.
    NativeHandleSnapshotMicros,
    /// Disk-file handle path queries.
    NativeHandleNames,
    /// Process memory-region queries, including unmapped regions.
    MemoryRegions,
    /// Data-mapping path queries.
    MappedNames,
}

/// Small, approximate live view, read without blocking the native worker.
/// Counters are monotonic within one operation; a read is not an atomic transaction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InspectionProgress {
    /// Current enumeration or indexing stage.
    pub phase: InspectionPhase,
    /// Process inspection attempts.
    pub processes: u64,
    /// Descriptor, mapping and module inspection attempts.
    pub resources: u64,
    /// Matching Windows file paths, or files submitted for fallback inspection.
    pub files: u64,
    /// Windows directories visited.
    pub directories: u64,
    /// Restart Manager queries.
    pub resource_queries: u64,
    /// Restart Manager query duration in microseconds.
    pub resource_query_micros: u64,
    /// Module snapshot attempts.
    pub module_snapshots: u64,
    /// Module snapshot duration in microseconds.
    pub module_snapshot_micros: u64,
    /// File identity probes.
    pub file_identity_queries: u64,
    /// Bounded resource workers started; zero on Unix.
    pub resource_workers: u64,
    /// Queries to the native file-user fallback; not Restart Manager calls.
    pub native_file_user_queries: u64,
    /// Completed native handle snapshots.
    pub native_handle_snapshots: u64,
    /// Native handle snapshot duration in microseconds.
    pub native_handle_snapshot_micros: u64,
    /// Disk-file handle path queries.
    pub native_handle_names: u64,
    /// Process memory-region queries, including unmapped regions.
    pub memory_regions: u64,
    /// Data-mapping path queries.
    pub mapped_names: u64,
}

#[derive(Default)]
pub(crate) struct ProgressState {
    phase: AtomicU8,
    counters: [AtomicU64; 16],
}
impl ProgressState {
    pub(crate) fn phase(&self, phase: InspectionPhase) {
        self.phase.store(phase as u8, Ordering::Relaxed);
    }
    pub(crate) fn add(&self, counter: InspectionCounter, amount: u64) {
        self.counters[counter as usize].fetch_add(amount, Ordering::Relaxed);
    }
    pub(crate) fn read(&self) -> InspectionProgress {
        let counters = self
            .counters
            .each_ref()
            .map(|value| value.load(Ordering::Relaxed));
        InspectionProgress {
            phase: match self.phase.load(Ordering::Relaxed) {
                1 => InspectionPhase::Files,
                2 => InspectionPhase::Ports,
                3 => InspectionPhase::Indexing,
                _ => InspectionPhase::Processes,
            },
            processes: counters[0],
            resources: counters[1],
            files: counters[2],
            directories: counters[3],
            resource_queries: counters[4],
            resource_query_micros: counters[5],
            module_snapshots: counters[6],
            module_snapshot_micros: counters[7],
            file_identity_queries: counters[8],
            resource_workers: counters[9],
            native_file_user_queries: counters[10],
            native_handle_snapshots: counters[11],
            native_handle_snapshot_micros: counters[12],
            native_handle_names: counters[13],
            memory_regions: counters[14],
            mapped_names: counters[15],
        }
    }
}

/// Records a native call duration even when the operation fails or is cancelled.
pub struct InspectionTimer {
    cancellation: crate::Cancellation,
    counter: InspectionCounter,
    started: Instant,
}
impl InspectionTimer {
    pub(crate) fn new(cancellation: crate::Cancellation, counter: InspectionCounter) -> Self {
        Self {
            cancellation,
            counter,
            started: Instant::now(),
        }
    }
}
impl Drop for InspectionTimer {
    fn drop(&mut self) {
        self.cancellation.record(
            self.counter,
            self.started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cancellation;
    #[test]
    fn cloned_operation_shares_progress_but_new_operation_starts_empty() {
        let operation = Cancellation::default();
        let worker = operation.clone();
        worker.set_phase(InspectionPhase::Files);
        worker.record(InspectionCounter::Files, 12);
        worker.record(InspectionCounter::ResourceQueries, 2);
        assert_eq!(operation.progress().files, 12);
        assert_eq!(operation.progress().phase, InspectionPhase::Files);
        operation.cancel();
        assert!(worker.check().is_err());
        assert_eq!(operation.progress().resource_queries, 2);
        assert_eq!(
            Cancellation::default().progress(),
            InspectionProgress::default()
        );
    }
}
