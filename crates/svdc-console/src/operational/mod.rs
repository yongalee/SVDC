//! SVDC-local operational state.
//!
//! Distinct from the SCD-derived ChannelRegistry (`scd::registry`).
//! The SCD is an immutable input authored by the SCT and produced by
//! the system engineer per IEC 61850-6 — the SVDC must never write
//! back to it. Calibration triples, subscription enable/disable, and
//! similar operational tuning live here instead, persisted in the
//! SVDC's own config file (`/etc/svdc/operational.toml` in production;
//! in-memory for v0.1).
//!
//! OWNER: claude-code (WBS-9.6a extension for calibration).
//! NFR-10: English-only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

/// Per-channel calibration triple.
///
/// Applied to the raw SV sample as:
/// `corrected = (raw * gain + offset) * unit_scale`.
/// Default (1.0, 0.0, 1.0) is the identity transform.
///
/// `gain` and `offset` correct sensor non-idealities (gain error,
/// DC bias) and live as f32 in scaled-integer space. `unit_scale`
/// brings the result into engineering units for display (e.g. 0.01
/// for voltage to convert the 9-2 LE i32 representation to volts).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Multiplicative gain correction.
    pub gain: f32,
    /// Additive offset, in raw scaled-integer units.
    pub offset: f32,
    /// Display-unit scale (e.g. 0.01 for voltage → V).
    pub unit_scale: f32,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            gain: 1.0,
            offset: 0.0,
            unit_scale: 1.0,
        }
    }
}

impl Calibration {
    /// Apply the calibration to a raw i32 SV sample.
    pub fn apply(&self, raw: i32) -> f32 {
        (raw as f32 * self.gain + self.offset) * self.unit_scale
    }

    /// Whether this calibration is the identity transform.
    pub fn is_identity(&self) -> bool {
        self.gain == 1.0 && self.offset == 0.0 && self.unit_scale == 1.0
    }
}

/// Key for the per-channel calibration map.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelKey {
    /// MU id this channel belongs to (matches `MergingUnit::id`).
    pub mu_id: String,
    /// 0-based index into the MU's `channels` list.
    pub channel_idx: usize,
}

/// SVDC-local operational state. Read-write across routes.
#[derive(Debug, Default)]
pub struct OperationalState {
    calibrations: RwLock<HashMap<ChannelKey, Calibration>>,
    /// If set, every successful mutation triggers a best-effort write
    /// of the current state to this path (TOML format).
    persistence_path: RwLock<Option<PathBuf>>,
}

impl OperationalState {
    /// Construct an empty operational state.
    pub fn new() -> Self {
        Self {
            calibrations: RwLock::new(HashMap::new()),
            persistence_path: RwLock::new(None),
        }
    }

    /// Look up the calibration for one channel. Returns the identity
    /// transform if no override has been applied.
    pub fn calibration(&self, mu_id: &str, channel_idx: usize) -> Calibration {
        let key = ChannelKey {
            mu_id: mu_id.to_string(),
            channel_idx,
        };
        self.calibrations
            .read()
            .ok()
            .and_then(|g| g.get(&key).copied())
            .unwrap_or_default()
    }

    /// Set or update the calibration for one channel. Returns the
    /// previous calibration (default if none was set). Auto-saves to
    /// the persistence path if one is configured.
    pub fn set_calibration(&self, key: ChannelKey, value: Calibration) -> Calibration {
        let prev = {
            let mut g = self
                .calibrations
                .write()
                .expect("operational state lock poisoned");
            g.insert(key.clone(), value).unwrap_or_default()
        };
        self.auto_save();
        prev
    }

    /// Reset a channel back to identity (remove the override). Auto-
    /// saves to the persistence path if one is configured.
    pub fn reset_calibration(&self, key: &ChannelKey) -> Option<Calibration> {
        let removed = {
            let mut g = self
                .calibrations
                .write()
                .expect("operational state lock poisoned");
            g.remove(key)
        };
        if removed.is_some() {
            self.auto_save();
        }
        removed
    }

    /// Snapshot all non-default calibrations. Useful for the audit log
    /// and for the eventual TOML serializer.
    pub fn overrides(&self) -> HashMap<ChannelKey, Calibration> {
        self.calibrations
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    /// Serialize the current state as TOML.
    pub fn to_toml(&self) -> Result<String, PersistError> {
        let snapshot = self.overrides();
        let mut entries: Vec<PersistedEntry> = snapshot
            .into_iter()
            .map(|(k, v)| PersistedEntry {
                mu_id: k.mu_id,
                channel_idx: k.channel_idx,
                gain: v.gain,
                offset: v.offset,
                unit_scale: v.unit_scale,
            })
            .collect();
        // Deterministic order so the file is diff-friendly.
        entries.sort_by(|a, b| {
            a.mu_id
                .cmp(&b.mu_id)
                .then_with(|| a.channel_idx.cmp(&b.channel_idx))
        });
        let doc = PersistedDocument {
            version: PERSIST_VERSION,
            calibration: entries,
        };
        toml::to_string_pretty(&doc).map_err(PersistError::Serialize)
    }

    /// Replace the in-memory state from a TOML document. Used both by
    /// configure_persistence (load on startup) and by tests.
    pub fn replace_from_toml(&self, s: &str) -> Result<usize, PersistError> {
        let doc: PersistedDocument = toml::from_str(s).map_err(PersistError::Deserialize)?;
        if doc.version != PERSIST_VERSION {
            return Err(PersistError::UnsupportedVersion(doc.version));
        }
        let mut g = self
            .calibrations
            .write()
            .expect("operational state lock poisoned");
        g.clear();
        for e in doc.calibration {
            g.insert(
                ChannelKey {
                    mu_id: e.mu_id,
                    channel_idx: e.channel_idx,
                },
                Calibration {
                    gain: e.gain,
                    offset: e.offset,
                    unit_scale: e.unit_scale,
                },
            );
        }
        Ok(g.len())
    }

    /// Configure this state to load from / persist to `path`. If the
    /// file already exists, its contents replace the current state.
    /// All subsequent mutations auto-save to `path` (best-effort).
    pub fn configure_persistence(&self, path: PathBuf) -> Result<usize, PersistError> {
        let loaded = if path.exists() {
            let s = std::fs::read_to_string(&path).map_err(PersistError::Io)?;
            self.replace_from_toml(&s)?
        } else {
            0
        };
        *self
            .persistence_path
            .write()
            .expect("operational state lock poisoned") = Some(path);
        Ok(loaded)
    }

    /// Force-save the current state to the configured path. No-op if
    /// no path is configured.
    pub fn save_now(&self) -> Result<(), PersistError> {
        let path = self
            .persistence_path
            .read()
            .expect("operational state lock poisoned")
            .clone();
        let Some(path) = path else {
            return Ok(());
        };
        self.save_to_path(&path)
    }

    /// Save to an explicit path (atomic via tmp-rename).
    pub fn save_to_path(&self, path: &Path) -> Result<(), PersistError> {
        let body = self.to_toml()?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body.as_bytes()).map_err(PersistError::Io)?;
        std::fs::rename(&tmp, path).map_err(PersistError::Io)?;
        Ok(())
    }

    fn auto_save(&self) {
        let path = self
            .persistence_path
            .read()
            .expect("operational state lock poisoned")
            .clone();
        let Some(path) = path else {
            return;
        };
        if let Err(e) = self.save_to_path(&path) {
            // Best-effort: log and move on. Operational edits should
            // not block the operator if the disk is read-only.
            tracing::warn!(
                error = %e,
                path = %path.display(),
                "operational state auto-save failed"
            );
        }
    }
}

const PERSIST_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PersistedDocument {
    version: u32,
    #[serde(default)]
    calibration: Vec<PersistedEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedEntry {
    mu_id: String,
    channel_idx: usize,
    gain: f32,
    offset: f32,
    unit_scale: f32,
}

/// Errors from persistence I/O and (de)serialization.
#[derive(Debug)]
pub enum PersistError {
    /// File-system I/O error.
    Io(std::io::Error),
    /// TOML serialization error.
    Serialize(toml::ser::Error),
    /// TOML deserialization error.
    Deserialize(toml::de::Error),
    /// File on disk uses a version this build does not understand.
    UnsupportedVersion(u32),
}

impl std::fmt::Display for PersistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PersistError::Io(e) => write!(f, "operational state I/O: {e}"),
            PersistError::Serialize(e) => write!(f, "operational state serialize: {e}"),
            PersistError::Deserialize(e) => write!(f, "operational state deserialize: {e}"),
            PersistError::UnsupportedVersion(v) => write!(
                f,
                "operational state version {v} unsupported by this build (expected {PERSIST_VERSION})"
            ),
        }
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PersistError::Io(e) => Some(e),
            PersistError::Serialize(e) => Some(e),
            PersistError::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

/// Shared handle used by axum handlers.
pub type SharedOperational = Arc<OperationalState>;

/// Process-wide singleton.
pub fn global() -> SharedOperational {
    static INSTANCE: OnceLock<SharedOperational> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Arc::new(OperationalState::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_calibration_is_identity_transform() {
        let c = Calibration::default();
        assert!(c.is_identity());
        assert_eq!(c.apply(1000), 1000.0);
        assert_eq!(c.apply(-2500), -2500.0);
    }

    #[test]
    fn apply_corrects_sensor_gain_and_offset() {
        let c = Calibration {
            gain: 1.05,
            offset: -50.0,
            unit_scale: 0.01,
        };
        // raw 5000 -> (5000*1.05 - 50) * 0.01 = 52.0
        let r = c.apply(5000);
        assert!((r - 52.0).abs() < 1e-3, "got {r}");
    }

    #[test]
    fn operational_state_returns_identity_when_unset() {
        let s = OperationalState::new();
        let c = s.calibration("MU-X", 0);
        assert!(c.is_identity());
    }

    #[test]
    fn set_and_reset_calibration_roundtrips() {
        let s = OperationalState::new();
        let key = ChannelKey {
            mu_id: "MU-X".into(),
            channel_idx: 2,
        };
        let cal = Calibration {
            gain: 1.1,
            offset: 5.0,
            unit_scale: 0.01,
        };
        let prev = s.set_calibration(key.clone(), cal);
        assert!(prev.is_identity());
        let got = s.calibration("MU-X", 2);
        assert_eq!(got, cal);

        let removed = s.reset_calibration(&key);
        assert_eq!(removed, Some(cal));
        assert!(s.calibration("MU-X", 2).is_identity());
    }

    #[test]
    fn overrides_snapshot_excludes_unset_channels() {
        let s = OperationalState::new();
        s.set_calibration(
            ChannelKey {
                mu_id: "MU-A".into(),
                channel_idx: 0,
            },
            Calibration {
                gain: 2.0,
                offset: 0.0,
                unit_scale: 1.0,
            },
        );
        let snap = s.overrides();
        assert_eq!(snap.len(), 1);
        assert!(snap.contains_key(&ChannelKey {
            mu_id: "MU-A".into(),
            channel_idx: 0,
        }));
    }

    #[test]
    fn toml_roundtrips_through_state() {
        let s = OperationalState::new();
        s.set_calibration(
            ChannelKey {
                mu_id: "MU-A".into(),
                channel_idx: 0,
            },
            Calibration {
                gain: 1.05,
                offset: -50.0,
                unit_scale: 0.01,
            },
        );
        s.set_calibration(
            ChannelKey {
                mu_id: "MU-A".into(),
                channel_idx: 4,
            },
            Calibration {
                gain: 1.02,
                offset: 0.0,
                unit_scale: 0.01,
            },
        );

        let toml_text = s.to_toml().expect("serialize");
        // Diff-friendly: top-level version, then ordered entries.
        assert!(toml_text.contains("version = 1"));
        assert!(toml_text.contains(r#"mu_id = "MU-A""#));
        // Ordering is by (mu_id, channel_idx).
        let i0 = toml_text.find("channel_idx = 0").expect("entry 0");
        let i4 = toml_text.find("channel_idx = 4").expect("entry 4");
        assert!(i0 < i4, "entries should be ordered by channel_idx");

        let s2 = OperationalState::new();
        let n = s2.replace_from_toml(&toml_text).expect("deserialize");
        assert_eq!(n, 2);
        let cal0 = s2.calibration("MU-A", 0);
        assert!((cal0.gain - 1.05).abs() < 1e-6);
        assert!((cal0.unit_scale - 0.01).abs() < 1e-6);
    }

    #[test]
    fn empty_state_roundtrips_and_has_no_entries() {
        let s = OperationalState::new();
        let body = s.to_toml().expect("serialize");
        assert!(body.contains("version = 1"));
        let s2 = OperationalState::new();
        let n = s2.replace_from_toml(&body).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn replace_from_toml_rejects_unsupported_version() {
        let s = OperationalState::new();
        let bad = "version = 999\n";
        let r = s.replace_from_toml(bad);
        assert!(matches!(r, Err(PersistError::UnsupportedVersion(999))));
    }

    #[test]
    fn replace_from_toml_rejects_garbage() {
        let s = OperationalState::new();
        let r = s.replace_from_toml("not toml at all {{{");
        assert!(matches!(r, Err(PersistError::Deserialize(_))));
    }

    #[test]
    fn save_to_path_writes_file() {
        let s = OperationalState::new();
        s.set_calibration(
            ChannelKey {
                mu_id: "MU-X".into(),
                channel_idx: 1,
            },
            Calibration {
                gain: 1.5,
                offset: 0.0,
                unit_scale: 1.0,
            },
        );
        let dir = std::env::temp_dir().join(format!(
            "svdc-op-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("operational.toml");
        s.save_to_path(&path).expect("save");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("MU-X"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn configure_persistence_round_trips() {
        let dir = std::env::temp_dir().join(format!(
            "svdc-op-cfg-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("operational.toml");

        // Process A: writes calibration, configured to save automatically.
        let s_a = OperationalState::new();
        s_a.configure_persistence(path.clone())
            .expect("configure A");
        s_a.set_calibration(
            ChannelKey {
                mu_id: "MU-Z".into(),
                channel_idx: 7,
            },
            Calibration {
                gain: 1.1,
                offset: 5.0,
                unit_scale: 0.01,
            },
        );

        // Process B: starts up, configure_persistence reads the file.
        let s_b = OperationalState::new();
        let loaded = s_b
            .configure_persistence(path.clone())
            .expect("configure B");
        assert_eq!(loaded, 1);
        let cal = s_b.calibration("MU-Z", 7);
        assert!((cal.gain - 1.1).abs() < 1e-6);
        assert!((cal.offset - 5.0).abs() < 1e-6);

        std::fs::remove_dir_all(&dir).ok();
    }
}
