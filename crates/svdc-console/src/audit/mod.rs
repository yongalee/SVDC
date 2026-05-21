//! Audit log — typed record of operator actions.
//!
//! Every operator-initiated state change in the SVDC (calibration,
//! SCD upload, manual MU register, northbound enable/disable, …)
//! flows through `audit::record(...)`. The audit log is a bounded
//! in-memory ring (newest-first) plus a `tracing::info!` side-effect
//! so events still show up in logs / journald. The web UI consumes
//! the ring via `GET /api/audit`.
//!
//! Phase 5 extension: when `configure_persistence(path)` has been
//! called (typically by the daemon at startup), every event is also
//! appended to a JSONL file at `path`. On startup the file is
//! replayed into the ring so audit history survives daemon restart.
//! The in-memory ring stays bounded; the on-disk file grows
//! unbounded — rotation lands in a later phase when retention policy
//! is defined.
//!
//! OWNER: claude-code.
//! NFR-10: English-only.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, LineWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Default ring capacity. ~1000 events is small enough that the
/// `/api/audit` payload is bounded, large enough to cover several
/// operator sessions.
pub const DEFAULT_CAPACITY: usize = 1000;

/// One audit entry: typed event plus a Unix-millis timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Unix milliseconds at the time the event was recorded.
    pub timestamp_ms: u64,
    /// Monotonic per-process sequence number. Useful for clients that
    /// want gap detection. Sequence numbers continue from the highest
    /// loaded value when persistence is configured.
    pub seq: u64,
    /// The event payload.
    #[serde(flatten)]
    pub event: AuditEvent,
}

/// Discriminated union of every audit-worthy operator action.
///
/// New variants land here when a new write path is added; the route
/// handler imports `AuditEvent::*` and calls `audit::record(...)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum AuditEvent {
    /// SCD file uploaded via the multipart endpoint.
    ScdUpload {
        /// Number of MUs registered as a result.
        mu_count: usize,
    },
    /// Built-in sample SCD loaded via the convenience button.
    ScdLoadSample {
        /// Number of MUs registered.
        mu_count: usize,
    },
    /// Single MU registered through the manual-register form / API.
    MuManualRegister {
        /// MU identifier registered.
        mu_id: String,
        /// Total MU count after the register.
        total: usize,
    },
    /// Channel registry cleared (all MUs removed).
    RegistryCleared {
        /// Number of MUs that were dropped.
        removed: usize,
    },
    /// Calibration triple set or updated on one channel.
    CalibrationSet {
        /// MU id.
        mu_id: String,
        /// 0-based channel index.
        channel_idx: usize,
        /// New gain value (raw).
        gain: f32,
        /// New offset value (raw).
        offset: f32,
        /// New unit_scale.
        unit_scale: f32,
    },
    /// Calibration on one channel reset back to identity.
    CalibrationReset {
        /// MU id.
        mu_id: String,
        /// 0-based channel index.
        channel_idx: usize,
        /// Whether the channel actually had an override before reset.
        had_override: bool,
    },
    /// Northbound layer enable / disable.
    NorthboundStateChange {
        /// `L0`..`L3`.
        layer: String,
        /// Enabled state after the action.
        enabled: bool,
    },
}

impl AuditEvent {
    /// One-line summary suitable for the audit table.
    pub fn summary(&self) -> String {
        match self {
            AuditEvent::ScdUpload { mu_count } => {
                format!("SCD uploaded ({mu_count} MU(s) registered)")
            }
            AuditEvent::ScdLoadSample { mu_count } => {
                format!("Built-in sample SCD loaded ({mu_count} MU(s))")
            }
            AuditEvent::MuManualRegister { mu_id, total } => {
                format!("MU '{mu_id}' registered manually (total {total})")
            }
            AuditEvent::RegistryCleared { removed } => {
                format!("Channel registry cleared ({removed} MU(s) dropped)")
            }
            AuditEvent::CalibrationSet {
                mu_id,
                channel_idx,
                gain,
                offset,
                unit_scale,
            } => format!(
                "Calibration set MU '{mu_id}' ch {channel_idx} (gain={gain}, offset={offset}, unit_scale={unit_scale})"
            ),
            AuditEvent::CalibrationReset {
                mu_id,
                channel_idx,
                had_override,
            } => {
                if *had_override {
                    format!("Calibration reset MU '{mu_id}' ch {channel_idx}")
                } else {
                    format!("Calibration reset MU '{mu_id}' ch {channel_idx} (no override existed)")
                }
            }
            AuditEvent::NorthboundStateChange { layer, enabled } => {
                let action = if *enabled { "enabled" } else { "disabled" };
                format!("Northbound layer {layer} {action}")
            }
        }
    }
}

/// Errors raised by persistence setup. Errors during ongoing record
/// writes are not surfaced — they log and are swallowed so an audit
/// failure cannot block an operator action.
#[derive(Debug)]
pub enum PersistError {
    /// I/O failure opening or reading the JSONL file.
    Io(std::io::Error),
    /// One of the lines in the JSONL file failed JSON parse.
    BadLine {
        /// 1-based line number that failed.
        line: usize,
        /// Underlying serde error.
        source: serde_json::Error,
    },
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Io(e) => write!(f, "audit persistence I/O error: {e}"),
            PersistError::BadLine { line, source } => {
                write!(
                    f,
                    "audit persistence: invalid JSON at line {line}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PersistError::Io(e) => Some(e),
            PersistError::BadLine { source, .. } => Some(source),
        }
    }
}

/// Bounded in-memory ring of audit records.
#[derive(Debug)]
pub struct AuditLog {
    inner: RwLock<Inner>,
    writer: Mutex<Option<PersistWriter>>,
    capacity: usize,
}

#[derive(Debug, Default)]
struct Inner {
    ring: VecDeque<AuditRecord>,
    next_seq: u64,
}

#[derive(Debug)]
struct PersistWriter {
    path: PathBuf,
    file: LineWriter<File>,
}

impl AuditLog {
    /// Construct a fresh log with the given ring capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(Inner {
                ring: VecDeque::with_capacity(capacity),
                next_seq: 1,
            }),
            writer: Mutex::new(None),
            capacity,
        }
    }

    /// Configure this log to persist every recorded event as one JSON
    /// line appended to `path`.
    ///
    /// If `path` already exists, its lines are parsed and the most
    /// recent `capacity` records are loaded into the ring. The next
    /// sequence number is set to `max(loaded seq) + 1` so on-disk and
    /// in-memory sequences stay monotonic across restarts.
    ///
    /// Returns the number of records loaded.
    pub fn configure_persistence(&self, path: PathBuf) -> Result<usize, PersistError> {
        let loaded = if path.exists() {
            self.replay_from_file(&path)?
        } else {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(PersistError::Io)?;
                }
            }
            0
        };
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(PersistError::Io)?;
        let mut slot = self.writer.lock().expect("audit writer lock poisoned");
        *slot = Some(PersistWriter {
            path,
            file: LineWriter::new(file),
        });
        Ok(loaded)
    }

    fn replay_from_file(&self, path: &Path) -> Result<usize, PersistError> {
        let f = File::open(path).map_err(PersistError::Io)?;
        let reader = BufReader::new(f);
        let mut records: Vec<AuditRecord> = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(PersistError::Io)?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let rec: AuditRecord =
                serde_json::from_str(trimmed).map_err(|source| PersistError::BadLine {
                    line: idx + 1,
                    source,
                })?;
            records.push(rec);
        }
        let max_seq = records.iter().map(|r| r.seq).max().unwrap_or(0);
        let total = records.len();
        let mut g = self.inner.write().expect("audit log lock poisoned");
        g.ring.clear();
        for rec in records.into_iter().rev().take(self.capacity).rev() {
            g.ring.push_back(rec);
        }
        g.next_seq = max_seq + 1;
        Ok(total)
    }

    /// Current persistence path, if configured. Test helper.
    pub fn persistence_path(&self) -> Option<PathBuf> {
        self.writer
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|w| w.path.clone()))
    }

    /// Record one event. Also emits `tracing::info!` so the same line
    /// shows up in stderr / journald with structured fields. If
    /// persistence is configured, the record is appended to the JSONL
    /// file (best-effort: I/O errors warn and are swallowed).
    pub fn record(&self, event: AuditEvent) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let record = {
            let mut g = self.inner.write().expect("audit log lock poisoned");
            let seq = g.next_seq;
            g.next_seq += 1;
            let rec = AuditRecord {
                timestamp_ms: now_ms,
                seq,
                event: event.clone(),
            };
            g.ring.push_back(rec.clone());
            while g.ring.len() > self.capacity {
                g.ring.pop_front();
            }
            rec
        };
        self.persist_line(&record);
        tracing::info!(
            audit.seq = record.seq,
            audit.ts_ms = record.timestamp_ms,
            audit.summary = %record.event.summary(),
            "audit"
        );
    }

    fn persist_line(&self, record: &AuditRecord) {
        let mut slot = match self.writer.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(w) = slot.as_mut() else { return };
        match serde_json::to_string(record) {
            Ok(line) => {
                if let Err(e) = writeln!(w.file, "{line}") {
                    tracing::warn!(
                        path = %w.path.display(),
                        error = %e,
                        "audit persistence write failed; subsequent events may be lost"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "audit persistence: could not serialise record"
                );
            }
        }
    }

    /// Get the newest `n` records, newest first. Pass `usize::MAX` for
    /// "everything held".
    pub fn recent(&self, n: usize) -> Vec<AuditRecord> {
        let g = match self.inner.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        g.ring.iter().rev().take(n).cloned().collect()
    }

    /// Number of records currently held.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.ring.len()).unwrap_or(0)
    }

    /// Whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drop all records (test helper).
    pub fn clear(&self) {
        if let Ok(mut g) = self.inner.write() {
            g.ring.clear();
        }
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

/// Shared handle used by axum handlers and routes that record events.
pub type SharedAudit = Arc<AuditLog>;

/// Process-wide singleton, lazy-initialised. All route handlers should
/// fetch via this rather than constructing their own.
pub fn global() -> SharedAudit {
    static INSTANCE: OnceLock<SharedAudit> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Arc::new(AuditLog::default()))
        .clone()
}

/// Convenience: record into the global log.
pub fn record(event: AuditEvent) {
    global().record(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_log() -> AuditLog {
        AuditLog::with_capacity(4)
    }

    #[test]
    fn record_assigns_monotonic_sequence() {
        let log = fresh_log();
        log.record(AuditEvent::ScdLoadSample { mu_count: 1 });
        log.record(AuditEvent::RegistryCleared { removed: 1 });
        let recent = log.recent(usize::MAX);
        assert_eq!(recent.len(), 2);
        // recent() is newest-first → seq 2 before seq 1.
        assert_eq!(recent[0].seq, 2);
        assert_eq!(recent[1].seq, 1);
    }

    #[test]
    fn ring_drops_oldest_when_capacity_exceeded() {
        let log = fresh_log(); // capacity 4
        for i in 0..6 {
            log.record(AuditEvent::ScdUpload { mu_count: i });
        }
        let recent = log.recent(usize::MAX);
        assert_eq!(recent.len(), 4);
        // The 4 newest are seq 6, 5, 4, 3.
        assert_eq!(recent[0].seq, 6);
        assert_eq!(recent[3].seq, 3);
    }

    #[test]
    fn recent_limit_is_respected() {
        let log = fresh_log();
        for _ in 0..4 {
            log.record(AuditEvent::ScdLoadSample { mu_count: 1 });
        }
        assert_eq!(log.recent(2).len(), 2);
        assert_eq!(log.recent(100).len(), 4);
    }

    #[test]
    fn summary_renders_each_variant() {
        let cases = [
            AuditEvent::ScdUpload { mu_count: 3 },
            AuditEvent::ScdLoadSample { mu_count: 1 },
            AuditEvent::MuManualRegister {
                mu_id: "MU-A".into(),
                total: 2,
            },
            AuditEvent::RegistryCleared { removed: 5 },
            AuditEvent::CalibrationSet {
                mu_id: "MU-A".into(),
                channel_idx: 0,
                gain: 1.05,
                offset: -50.0,
                unit_scale: 0.01,
            },
            AuditEvent::CalibrationReset {
                mu_id: "MU-A".into(),
                channel_idx: 0,
                had_override: true,
            },
            AuditEvent::NorthboundStateChange {
                layer: "L1".into(),
                enabled: true,
            },
        ];
        for ev in cases {
            let s = ev.summary();
            assert!(!s.is_empty(), "summary for {ev:?}");
        }
    }

    #[test]
    fn json_envelope_has_tag_and_flat_fields() {
        let rec = AuditRecord {
            timestamp_ms: 1700000000000,
            seq: 1,
            event: AuditEvent::CalibrationSet {
                mu_id: "MU-X".into(),
                channel_idx: 4,
                gain: 1.05,
                offset: 0.0,
                unit_scale: 0.01,
            },
        };
        let json = serde_json::to_string(&rec).unwrap();
        // serde(tag = "event") puts the variant name in the "event" field.
        assert!(json.contains(r#""event":"calibration_set""#));
        assert!(json.contains(r#""mu_id":"MU-X""#));
        assert!(json.contains(r#""seq":1"#));
    }

    #[test]
    fn clear_empties_the_ring() {
        let log = fresh_log();
        log.record(AuditEvent::RegistryCleared { removed: 0 });
        assert_eq!(log.len(), 1);
        log.clear();
        assert!(log.is_empty());
    }

    fn unique_audit_path(tag: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("svdc-audit-{tag}-{ts}.jsonl"))
    }

    #[test]
    fn configure_persistence_round_trips() {
        let path = unique_audit_path("roundtrip");
        let _ = std::fs::remove_file(&path);

        // Process A: records events, all of which append to disk.
        let log_a = AuditLog::with_capacity(100);
        let loaded = log_a.configure_persistence(path.clone()).unwrap();
        assert_eq!(loaded, 0, "fresh file → no records loaded");
        log_a.record(AuditEvent::ScdLoadSample { mu_count: 3 });
        log_a.record(AuditEvent::NorthboundStateChange {
            layer: "L1".into(),
            enabled: true,
        });
        log_a.record(AuditEvent::CalibrationSet {
            mu_id: "MU-A".into(),
            channel_idx: 0,
            gain: 1.05,
            offset: 0.0,
            unit_scale: 0.01,
        });
        drop(log_a);

        // File must have exactly 3 non-empty lines.
        let body = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = body.lines().filter(|s| !s.trim().is_empty()).collect();
        assert_eq!(lines.len(), 3);
        // Sanity: each line is valid JSON with the tagged variant.
        assert!(lines[0].contains(r#""event":"scd_load_sample""#));
        assert!(lines[2].contains(r#""mu_id":"MU-A""#));

        // Process B: starts up, configure_persistence replays the file.
        let log_b = AuditLog::with_capacity(100);
        let loaded = log_b.configure_persistence(path.clone()).unwrap();
        assert_eq!(loaded, 3, "all three records replay");
        let recent = log_b.recent(usize::MAX);
        assert_eq!(recent.len(), 3);
        // recent() is newest-first → seq 3 first.
        assert_eq!(recent[0].seq, 3);
        assert_eq!(recent[2].seq, 1);
        // New record continues the sequence at 4.
        log_b.record(AuditEvent::RegistryCleared { removed: 0 });
        let recent = log_b.recent(usize::MAX);
        assert_eq!(recent[0].seq, 4);

        // File now has 4 lines.
        drop(log_b);
        let body = std::fs::read_to_string(&path).unwrap();
        let n = body.lines().filter(|s| !s.trim().is_empty()).count();
        assert_eq!(n, 4);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn replay_keeps_only_capacity_newest_records() {
        let path = unique_audit_path("cap");
        let _ = std::fs::remove_file(&path);

        // Write 6 records on disk with capacity 4.
        let log_a = AuditLog::with_capacity(100);
        log_a.configure_persistence(path.clone()).unwrap();
        for i in 0..6u64 {
            log_a.record(AuditEvent::ScdUpload {
                mu_count: i as usize,
            });
        }
        drop(log_a);

        // Smaller capacity ring on restart.
        let log_b = AuditLog::with_capacity(4);
        let loaded = log_b.configure_persistence(path.clone()).unwrap();
        assert_eq!(loaded, 6, "configure_persistence reports total on disk");
        assert_eq!(log_b.len(), 4, "ring keeps only the newest 4");
        let recent = log_b.recent(usize::MAX);
        // The kept seq values are 6, 5, 4, 3 (newest-first).
        assert_eq!(recent[0].seq, 6);
        assert_eq!(recent[3].seq, 3);
        // Next record should be seq 7.
        log_b.record(AuditEvent::RegistryCleared { removed: 0 });
        assert_eq!(log_b.recent(1)[0].seq, 7);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn corrupt_line_surfaces_bad_line_error() {
        let path = unique_audit_path("corrupt");
        std::fs::write(&path, "{not json}\n").unwrap();
        let log = AuditLog::with_capacity(8);
        let err = log.configure_persistence(path.clone()).unwrap_err();
        match err {
            PersistError::BadLine { line, .. } => assert_eq!(line, 1),
            other => panic!("expected BadLine, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_lines_in_file_are_skipped() {
        let path = unique_audit_path("empty");
        // Hand-crafted file with a blank between two records.
        let line1 = serde_json::to_string(&AuditRecord {
            timestamp_ms: 1_700_000_000_000,
            seq: 1,
            event: AuditEvent::ScdUpload { mu_count: 1 },
        })
        .unwrap();
        let line2 = serde_json::to_string(&AuditRecord {
            timestamp_ms: 1_700_000_000_500,
            seq: 2,
            event: AuditEvent::RegistryCleared { removed: 1 },
        })
        .unwrap();
        std::fs::write(&path, format!("{line1}\n\n{line2}\n")).unwrap();

        let log = AuditLog::with_capacity(8);
        let loaded = log.configure_persistence(path.clone()).unwrap();
        assert_eq!(loaded, 2);
        let _ = std::fs::remove_file(&path);
    }
}
