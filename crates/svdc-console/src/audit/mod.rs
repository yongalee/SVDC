//! Audit log — typed record of operator actions.
//!
//! Every operator-initiated state change in the SVDC (calibration,
//! SCD upload, manual MU register, northbound enable/disable, …)
//! flows through `audit::record(...)`. The audit log is a bounded
//! in-memory ring (newest-first) plus a `tracing::info!` side-effect
//! so events still show up in logs / journald. The web UI consumes
//! the ring via `GET /api/audit`.
//!
//! Phase 0 scope: in-memory only. Phase 5 will add disk persistence
//! (rotating JSONL file) so audit survives daemon restart.
//!
//! OWNER: claude-code.
//! NFR-10: English-only.

use std::collections::VecDeque;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Default ring capacity. ~1000 events is small enough that the
/// `/api/audit` payload is bounded, large enough to cover several
/// operator sessions.
pub const DEFAULT_CAPACITY: usize = 1000;

/// One audit entry: typed event plus a Unix-millis timestamp.
#[derive(Debug, Clone, Serialize)]
pub struct AuditRecord {
    /// Unix milliseconds at the time the event was recorded.
    pub timestamp_ms: u64,
    /// Monotonic per-process sequence number. Useful for clients that
    /// want gap detection.
    pub seq: u64,
    /// The event payload.
    #[serde(flatten)]
    pub event: AuditEvent,
}

/// Discriminated union of every audit-worthy operator action.
///
/// New variants land here when a new write path is added; the route
/// handler imports `AuditEvent::*` and calls `audit::record(...)`.
#[derive(Debug, Clone, Serialize)]
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

/// Bounded in-memory ring of audit records.
#[derive(Debug)]
pub struct AuditLog {
    inner: RwLock<Inner>,
    capacity: usize,
}

#[derive(Debug, Default)]
struct Inner {
    ring: VecDeque<AuditRecord>,
    next_seq: u64,
}

impl AuditLog {
    /// Construct a fresh log with the given ring capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: RwLock::new(Inner {
                ring: VecDeque::with_capacity(capacity),
                next_seq: 1,
            }),
            capacity,
        }
    }

    /// Record one event. Also emits `tracing::info!` so the same line
    /// shows up in stderr / journald with structured fields.
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
        tracing::info!(
            audit.seq = record.seq,
            audit.ts_ms = record.timestamp_ms,
            audit.summary = %record.event.summary(),
            "audit"
        );
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
}
