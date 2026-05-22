//! AddressSpace node-list builder per ADR-0017 §2.
//!
//! Produces the deterministic list of OPC UA nodes the L1 server
//! must register at startup. The output is a flat `Vec` of
//! [`AddressSpaceNode`]s — each entry carries the string node ID,
//! browse name, kind, and parent reference so PR L can walk the
//! list in order and call the equivalent `opcua::server::AddressSpace::add_…`
//! method for each one. Keeping the build step library-neutral
//! means the test surface here is `assert!` on string IDs, not
//! `tokio` runtime + network.
//!
//! Tree shape:
//!
//! ```text
//! Objects
//! └── Substations                                  (Folder)
//!     └── <substation>                             (Folder)
//!         └── <MU svID>                            (Object)
//!             ├── ChannelRegistry                  (Folder)
//!             │   ├── Ch00_<name>                  (Object)
//!             │   │   ├── instMag.i / instMag.f
//!             │   │   ├── q / t / tick_id
//!             │   ├── …
//!             └── TickStatus                       (Folder)
//!                 ├── last_tick_id / last_ts_utc_ns / n_channels
//! ```

/// Reference 8-channel layout for the Phase 4 thin slice (ADR-0017
/// §8). Matches the southbound publisher's channel ordering.
pub const REFERENCE_CHANNELS: &[&str] = &["Va", "Vb", "Vc", "Vn", "Ia", "Ib", "Ic", "In"];

/// Builder input: one MU's identity and its ordered channel names.
/// PR L feeds this from the live `ChannelRegistry`; tests feed it
/// from literals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuFolderSpec {
    /// The `svID` published in the MU's SV ASDU. Becomes the OPC UA
    /// object name and node-ID component.
    pub sv_id: String,
    /// Channel short names in publish order. Channel index = vector
    /// index. For the thin slice this is [`REFERENCE_CHANNELS`].
    pub channel_names: Vec<String>,
}

impl MuFolderSpec {
    /// Build an MuFolderSpec from the reference 8-channel layout
    /// for the given svID. Convenience for tests and the thin
    /// slice; PR L's real flow reads channels out of the SCD.
    pub fn reference(sv_id: &str) -> Self {
        Self {
            sv_id: sv_id.to_string(),
            channel_names: REFERENCE_CHANNELS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Convenience alias for a list of channel names — distinguishes
/// "this is a channel layout" from "this is just any string vec"
/// at call sites.
pub type ChannelLayout = Vec<String>;

/// One entry in the OPC UA address space the L1 server will
/// register. The list is ordered: a child node always appears
/// *after* its parent, so a single forward pass can call
/// `AddressSpace::add_…` without needing to defer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressSpaceNode {
    /// String node ID in namespace 2 per ADR-0017 §2. Format
    /// matches the dotted hierarchy of the tree.
    pub node_id: String,
    /// Human-readable browse name shown by UA clients (UA Expert,
    /// etc.) when walking the tree.
    pub browse_name: String,
    /// Object / Folder / Variable kind.
    pub kind: NodeKind,
    /// String node ID of the parent. `None` only for the top
    /// `Substations` folder, which hangs off the standard
    /// `Objects` node defined by the OPC UA core.
    pub parent: Option<String>,
}

/// Kind discriminator for an [`AddressSpaceNode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A `FolderType` instance — used for `Substations`, individual
    /// substation folders, `ChannelRegistry`, and `TickStatus`.
    Folder,
    /// A generic object node. For the thin slice this is each MU
    /// and each per-channel container; PR L may switch to OPC UA's
    /// `AnalogValueType` once the SDD-to-companion-spec mapping is
    /// finalized.
    Object,
    /// A leaf variable. The [`VariableKind`] disambiguates the
    /// SVDC-side value source and OPC UA data type.
    Variable(VariableKind),
}

/// Variable role per ADR-0017 §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableKind {
    /// `instMag.i` — raw Q-format value (`Int32`), unscaled.
    InstMagI,
    /// `instMag.f` — calibrated engineering-unit value (`Float`).
    InstMagF,
    /// `q` — IEC 61850 quality byte mirrored as `UInt16`.
    Quality,
    /// `t` — sample timestamp as OPC UA `UtcTime` (`DateTime`).
    Time,
    /// `tick_id` — alignment tick the sample belongs to (`UInt64`).
    TickId,
    /// `TickStatus.last_tick_id` — MU-scoped (`UInt64`).
    LastTickId,
    /// `TickStatus.last_ts_utc_ns` — last observed `ts_utc_ns`
    /// (`UInt64`).
    LastTsUtcNs,
    /// `TickStatus.n_channels` — count of populated channels in the
    /// most recent tick (`UInt16`).
    NChannels,
}

/// Build the OPC UA address-space node list for `substation` and
/// the given `mus`. The returned list is in pre-order traversal so
/// PR L can walk it forward.
pub fn build_nodes(substation: &str, mus: &[MuFolderSpec]) -> Vec<AddressSpaceNode> {
    let mut out = Vec::new();

    // Top-level Substations folder (one per SVDC daemon).
    let substations_id = "s=Substations".to_string();
    out.push(AddressSpaceNode {
        node_id: substations_id.clone(),
        browse_name: "Substations".to_string(),
        kind: NodeKind::Folder,
        parent: None,
    });

    // Per-substation folder.
    let substation_id = format!("s=Substations.{}", substation);
    out.push(AddressSpaceNode {
        node_id: substation_id.clone(),
        browse_name: substation.to_string(),
        kind: NodeKind::Folder,
        parent: Some(substations_id.clone()),
    });

    for mu in mus {
        let mu_id = format!("{}.{}", substation_id, mu.sv_id);
        out.push(AddressSpaceNode {
            node_id: mu_id.clone(),
            browse_name: mu.sv_id.clone(),
            kind: NodeKind::Object,
            parent: Some(substation_id.clone()),
        });

        // ChannelRegistry folder under the MU.
        let registry_id = format!("{}.ChannelRegistry", mu_id);
        out.push(AddressSpaceNode {
            node_id: registry_id.clone(),
            browse_name: "ChannelRegistry".to_string(),
            kind: NodeKind::Folder,
            parent: Some(mu_id.clone()),
        });

        for (idx, name) in mu.channel_names.iter().enumerate() {
            let ch_id = format!("{}.Ch{:02}_{}", registry_id, idx, name);
            let ch_browse = format!("Ch{:02}_{}", idx, name);
            out.push(AddressSpaceNode {
                node_id: ch_id.clone(),
                browse_name: ch_browse.clone(),
                kind: NodeKind::Object,
                parent: Some(registry_id.clone()),
            });
            for (suffix, kind) in &[
                ("instMag.i", VariableKind::InstMagI),
                ("instMag.f", VariableKind::InstMagF),
                ("q", VariableKind::Quality),
                ("t", VariableKind::Time),
                ("tick_id", VariableKind::TickId),
            ] {
                out.push(AddressSpaceNode {
                    node_id: format!("{}.{}", ch_id, suffix),
                    browse_name: (*suffix).to_string(),
                    kind: NodeKind::Variable(*kind),
                    parent: Some(ch_id.clone()),
                });
            }
        }

        // TickStatus folder + three diagnostic variables.
        let tick_id = format!("{}.TickStatus", mu_id);
        out.push(AddressSpaceNode {
            node_id: tick_id.clone(),
            browse_name: "TickStatus".to_string(),
            kind: NodeKind::Folder,
            parent: Some(mu_id.clone()),
        });
        for (suffix, kind) in &[
            ("last_tick_id", VariableKind::LastTickId),
            ("last_ts_utc_ns", VariableKind::LastTsUtcNs),
            ("n_channels", VariableKind::NChannels),
        ] {
            out.push(AddressSpaceNode {
                node_id: format!("{}.{}", tick_id, suffix),
                browse_name: (*suffix).to_string(),
                kind: NodeKind::Variable(*kind),
                parent: Some(tick_id.clone()),
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids_of(nodes: &[AddressSpaceNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.node_id.as_str()).collect()
    }

    #[test]
    fn empty_mus_yields_only_substations_and_substation_folder() {
        let nodes = build_nodes("Demo", &[]);
        assert_eq!(ids_of(&nodes), vec!["s=Substations", "s=Substations.Demo"]);
        assert!(matches!(nodes[0].kind, NodeKind::Folder));
        assert_eq!(nodes[0].parent, None);
        assert_eq!(nodes[1].parent.as_deref(), Some("s=Substations"));
    }

    #[test]
    fn reference_layout_has_eight_channels_each_with_five_variables() {
        let nodes = build_nodes("Demo", &[MuFolderSpec::reference("SVDC_DEMO_PB_MU")]);

        let variable_count = nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Variable(_)))
            .count();
        // 8 channels × 5 vars + 3 TickStatus vars = 43.
        assert_eq!(variable_count, 43);

        let channel_object_count = nodes
            .iter()
            .filter(|n| n.browse_name.starts_with("Ch") && matches!(n.kind, NodeKind::Object))
            .count();
        assert_eq!(channel_object_count, 8);
    }

    #[test]
    fn node_ids_follow_dotted_hierarchy() {
        let nodes = build_nodes("Demo", &[MuFolderSpec::reference("MU01")]);
        let ids: Vec<&str> = ids_of(&nodes);

        assert!(ids.contains(&"s=Substations.Demo.MU01"));
        assert!(ids.contains(&"s=Substations.Demo.MU01.ChannelRegistry"));
        assert!(ids.contains(&"s=Substations.Demo.MU01.ChannelRegistry.Ch00_Va"));
        assert!(ids.contains(&"s=Substations.Demo.MU01.ChannelRegistry.Ch00_Va.instMag.i"));
        assert!(ids.contains(&"s=Substations.Demo.MU01.ChannelRegistry.Ch07_In.tick_id"));
        assert!(ids.contains(&"s=Substations.Demo.MU01.TickStatus.last_tick_id"));
    }

    #[test]
    fn parent_appears_before_child_in_pre_order() {
        // PR L walks the list forward and adds each node by parent
        // node ID; the parent must already exist by then.
        let nodes = build_nodes(
            "Demo",
            &[
                MuFolderSpec::reference("MU01"),
                MuFolderSpec::reference("MU02"),
            ],
        );
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for n in &nodes {
            if let Some(p) = n.parent.as_deref() {
                assert!(
                    seen.contains(p),
                    "child {} appears before parent {}",
                    n.node_id,
                    p
                );
            }
            seen.insert(n.node_id.as_str());
        }
    }

    #[test]
    fn two_mus_produce_disjoint_subtrees() {
        let nodes = build_nodes(
            "Demo",
            &[
                MuFolderSpec::reference("MU01"),
                MuFolderSpec::reference("MU02"),
            ],
        );
        // Per MU: 1 (MU obj) + 1 (ChannelRegistry) + 8 (channels)
        //       + 8 × 5 (channel vars) + 1 (TickStatus) + 3 (status vars)
        //       = 54 nodes.
        // Total: 2 (Substations + substation folder) + 2 × 54 = 110.
        assert_eq!(nodes.len(), 110);
        let mu01_ids = nodes
            .iter()
            .filter(|n| n.node_id.starts_with("s=Substations.Demo.MU01"))
            .count();
        let mu02_ids = nodes
            .iter()
            .filter(|n| n.node_id.starts_with("s=Substations.Demo.MU02"))
            .count();
        assert_eq!(mu01_ids, mu02_ids);
        assert_eq!(mu01_ids, 54);
    }

    #[test]
    fn no_duplicate_node_ids() {
        let nodes = build_nodes(
            "Demo",
            &[
                MuFolderSpec::reference("MU01"),
                MuFolderSpec::reference("MU02"),
            ],
        );
        let mut ids: Vec<&str> = ids_of(&nodes);
        ids.sort_unstable();
        let n_before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n_before, "duplicate node IDs detected");
    }

    #[test]
    fn channel_index_zero_pads_to_two_digits() {
        // For deployments with > 9 channels — single-digit indices
        // would re-sort weirdly in UA Expert if not zero-padded.
        let nodes = build_nodes(
            "Demo",
            &[MuFolderSpec {
                sv_id: "MU01".to_string(),
                channel_names: (0..12).map(|i| format!("X{}", i)).collect(),
            }],
        );
        assert!(nodes.iter().any(|n| n.node_id.contains(".Ch00_X0")));
        assert!(nodes.iter().any(|n| n.node_id.contains(".Ch11_X11")));
    }

    #[test]
    fn reference_channel_layout_matches_sv_publisher_order() {
        // Phase-current grouping is canonical: Va Vb Vc Vn Ia Ib Ic In.
        assert_eq!(
            REFERENCE_CHANNELS,
            &["Va", "Vb", "Vc", "Vn", "Ia", "Ib", "Ic", "In"]
        );
    }
}
