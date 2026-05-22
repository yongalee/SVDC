//! In-process channel registry shared across `routes::config`.
//!
//! Holds the most recently uploaded SCD as a `Vec<MergingUnit>`. The
//! daemon's hot path does not read from here — Phase 2/3 wires the
//! real configuration delivery to the ingest crate via a different
//! channel — but the Console's MU list / Dashboard tile count / OPC
//! UA AddressSpace builder all consult the registry.
//!
//! OWNER: claude-code (WBS-9.6a).

use std::sync::{Arc, OnceLock, RwLock};

use super::MergingUnit;

/// Thread-safe owner of the parsed SCD state.
#[derive(Debug, Default)]
pub struct ChannelRegistry {
    inner: RwLock<Vec<MergingUnit>>,
}

impl ChannelRegistry {
    /// Construct an empty registry.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
        }
    }

    /// Replace the registry contents wholesale. Returns the number of
    /// MUs now held. The semantics are "atomic swap": the registry is
    /// either entirely old or entirely new from a reader's point of
    /// view, never partially populated.
    pub fn replace(&self, mus: Vec<MergingUnit>) -> usize {
        let mut g = self.inner.write().expect("channel registry lock poisoned");
        *g = mus;
        g.len()
    }

    /// Read out an owned copy of the current MU list. Cheap for the
    /// sizes we deal with (a handful of MUs, ~8 channels each).
    pub fn snapshot(&self) -> Vec<MergingUnit> {
        self.inner.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Number of MUs currently registered.
    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn default_merging_units() -> Vec<super::MergingUnit> {
    use super::{Channel, ChannelUnit};
    (1..=6)
        .map(|i| {
            let smp_rate = if i == 5 { 4800 } else { 4000 };
            super::MergingUnit {
                id: format!("MU-0{}", i),
                mac: [0x00, 0x0a, 0x35, 0x01, 0x02, i as u8],
                appid: 0x4000,
                sv_id: format!("SSIEC_MU_0{}", i),
                smp_rate,
                channels: vec![
                    Channel {
                        name: "Ia".into(),
                        unit: ChannelUnit::Current,
                    },
                    Channel {
                        name: "Ib".into(),
                        unit: ChannelUnit::Current,
                    },
                    Channel {
                        name: "Ic".into(),
                        unit: ChannelUnit::Current,
                    },
                    Channel {
                        name: "In".into(),
                        unit: ChannelUnit::Current,
                    },
                    Channel {
                        name: "Va".into(),
                        unit: ChannelUnit::Voltage,
                    },
                    Channel {
                        name: "Vb".into(),
                        unit: ChannelUnit::Voltage,
                    },
                    Channel {
                        name: "Vc".into(),
                        unit: ChannelUnit::Voltage,
                    },
                    Channel {
                        name: "Vn".into(),
                        unit: ChannelUnit::Voltage,
                    },
                ],
            }
        })
        .collect()
}

/// Shared handle used by axum handlers.
pub type SharedRegistry = Arc<ChannelRegistry>;

/// Process-wide singleton used by every route that needs to look up
/// MUs by id (config, mu_detail, soon mus_list when Antigravity wires
/// it). One `ChannelRegistry` per process keeps the state consistent
/// across screens.
pub fn global() -> SharedRegistry {
    static INSTANCE: OnceLock<SharedRegistry> = OnceLock::new();
    let reg = INSTANCE
        .get_or_init(|| Arc::new(ChannelRegistry::new()))
        .clone();
    if reg.is_empty() {
        reg.replace(default_merging_units());
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scd::{Channel, ChannelUnit};

    fn sample_mu(id: &str) -> MergingUnit {
        MergingUnit {
            id: id.to_string(),
            mac: [1, 2, 3, 4, 5, 6],
            appid: 0x4000,
            sv_id: "SVDC_DEMO".to_string(),
            smp_rate: 4800,
            channels: vec![Channel {
                name: "VPhMMXU1.PhV.phsA.MX".to_string(),
                unit: ChannelUnit::Voltage,
            }],
        }
    }

    #[test]
    fn new_registry_is_empty() {
        let r = ChannelRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.snapshot().is_empty());
    }

    #[test]
    fn replace_swaps_atomically() {
        let r = ChannelRegistry::new();
        let n = r.replace(vec![sample_mu("MU-1"), sample_mu("MU-2")]);
        assert_eq!(n, 2);
        assert_eq!(r.len(), 2);
        let snap = r.snapshot();
        assert_eq!(snap[0].id, "MU-1");
        assert_eq!(snap[1].id, "MU-2");

        let n2 = r.replace(vec![sample_mu("MU-3")]);
        assert_eq!(n2, 1);
        assert_eq!(r.len(), 1);
        assert_eq!(r.snapshot()[0].id, "MU-3");
    }

    #[test]
    fn snapshot_is_owned_copy() {
        let r = ChannelRegistry::new();
        r.replace(vec![sample_mu("MU-X")]);
        let snap_a = r.snapshot();
        r.replace(vec![sample_mu("MU-Y")]);
        // snap_a still reflects the earlier state.
        assert_eq!(snap_a[0].id, "MU-X");
        assert_eq!(r.snapshot()[0].id, "MU-Y");
    }
}
