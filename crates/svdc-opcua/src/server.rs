//! Live OPC UA server task per ADR-0017 §7.
//!
//! Builds an `async-opcua` 0.18 server, populates its address
//! space from PR K's [`build_nodes`] output, and exposes a
//! [`LatestTickSnapshot`] handle that the daemon's L1 publisher
//! task updates as each new `TickRecord` lands in the
//! `TickBuffer`. The server runs as a tokio task; a second tokio
//! task copies the latest snapshot into the node manager via
//! `set_values` on a configurable cadence (default 100 ms /
//! 10 Hz, matching ADR-0017 §4).
//!
//! The crate path is `opcua::*` even though the dependency is
//! `async-opcua = "0.18"` — the freeopcua fork keeps the
//! `[lib] name = "opcua"` of locka99/opcua so existing
//! companion-spec example code compiles unchanged.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use opcua::server::address_space::Variable;
use opcua::server::diagnostics::NamespaceMetadata;
use opcua::server::node_manager::memory::{
    simple_node_manager, InMemoryNodeManager, SimpleNodeManager, SimpleNodeManagerImpl,
};
use opcua::server::{ServerBuilder, ServerHandle, SubscriptionCache};
use opcua::sync::Mutex;
use opcua::types::{BuildInfo, DataValue, DateTime, NodeId};

use crate::address_space::{build_nodes, AddressSpaceNode, MuFolderSpec, NodeKind, VariableKind};
use crate::quality::{apply_origin_override, iec61850_to_opcua_status};
use crate::timestamp::utc_ns_to_opcua_ticks;

/// OPC UA namespace URI for SVDC's L1 address space. The L1
/// server registers this URI and every SVDC node ID lives in the
/// resulting namespace index (typically `ns=2` after the standard
/// + server URIs).
pub const NAMESPACE_URI: &str = "urn:svdc:l1";

/// Internal key the simple node manager uses to identify our
/// instance. Lets a future deployment co-host other managers
/// without colliding.
pub const MANAGER_KEY: &str = "svdc-l1";

/// Default per-channel publish interval (10 Hz) per ADR-0017 §4.
pub const DEFAULT_PUBLISH_INTERVAL: Duration = Duration::from_millis(100);

/// Snapshot of the most recent published tick, written by the
/// daemon's L1 task and read by the OPC UA publisher loop. Held
/// behind `opcua::sync::Mutex` so it interoperates with the rest
/// of the crate's lock types.
#[derive(Debug, Clone, Default)]
pub struct LatestTickSnapshot {
    /// `TickRecord.tick_id` of the latest published sample. Zero
    /// before the first publish.
    pub tick_id: u64,
    /// `TickRecord.ts_utc_ns` — Unix-epoch ns, ready for
    /// [`utc_ns_to_opcua_ticks`].
    pub ts_utc_ns: u64,
    /// Number of populated channels in the source tick.
    pub n_channels: u16,
    /// Per-channel sample data, indexed by channel position. Each
    /// entry: `(raw_q_value, calibrated_float, quality_byte, origin_byte)`.
    pub samples: Vec<(i32, f32, u8, u8)>,
}

impl LatestTickSnapshot {
    /// Reset to the no-data state. Called when the publisher
    /// shuts down.
    pub fn clear(&mut self) {
        self.tick_id = 0;
        self.ts_utc_ns = 0;
        self.n_channels = 0;
        self.samples.clear();
    }
}

/// Configuration for [`OpcuaServer::start`].
#[derive(Debug, Clone)]
pub struct OpcuaServerConfig {
    /// TCP endpoint the server binds. ADR-0017 §5 forbids non-
    /// loopback unless the daemon caller has opted in via
    /// `--allow-insecure-bind`; this struct does not re-enforce
    /// the guard (the daemon validates before calling
    /// [`OpcuaServer::start`]).
    pub bind_addr: SocketAddr,
    /// Substation folder name shown beneath `Objects/Substations/`
    /// in the OPC UA browse tree.
    pub substation_name: String,
    /// MU + channel specs to expose. The first slice deployment
    /// passes one [`MuFolderSpec::reference`] — eight channels
    /// (Va Vb Vc Vn Ia Ib Ic In).
    pub mu_specs: Vec<MuFolderSpec>,
    /// How often the publisher task pushes the latest snapshot
    /// into the node manager. Defaults to
    /// [`DEFAULT_PUBLISH_INTERVAL`] (10 Hz).
    pub publish_interval: Duration,
}

impl OpcuaServerConfig {
    /// Convenience for the reference deployment: one MU with the
    /// canonical 8-channel layout, 10 Hz publishing.
    pub fn reference(bind_addr: SocketAddr, sv_id: &str) -> Self {
        Self {
            bind_addr,
            substation_name: "Demo".to_string(),
            mu_specs: vec![MuFolderSpec::reference(sv_id)],
            publish_interval: DEFAULT_PUBLISH_INTERVAL,
        }
    }
}

/// Result of [`OpcuaServer::start`]. Holds the server task, the
/// publisher task, and the shared snapshot the daemon writes
/// into.
pub struct OpcuaServer {
    /// Shared snapshot the daemon updates on each new tick. Take
    /// a clone of the `Arc` and overwrite the contents under
    /// the lock; the publisher task picks the new value up on its
    /// next interval tick.
    pub latest: Arc<Mutex<LatestTickSnapshot>>,
    /// Tokio task running the OPC UA server. Awaiting this future
    /// blocks until the server shuts down.
    pub server_task: tokio::task::JoinHandle<Result<(), String>>,
    /// Tokio task pushing the latest snapshot into the address
    /// space at [`OpcuaServerConfig::publish_interval`] cadence.
    pub publisher_task: tokio::task::JoinHandle<()>,
    /// Server handle for graceful shutdown — drop this last after
    /// calling `cancel()`.
    pub server_handle: ServerHandle,
}

/// Errors returned by [`OpcuaServer::start`].
#[derive(Debug)]
pub enum OpcuaServerError {
    /// `ServerBuilder::build()` rejected the configuration.
    BuilderRejected(String),
    /// The namespace URI returned by the builder did not match
    /// our [`NAMESPACE_URI`] — should not happen in practice but
    /// the type lets us return a clear error rather than panic.
    NamespaceLookupFailed,
    /// The simple node manager was not registered with the
    /// expected key. Same defensive-error reasoning as above.
    NodeManagerMissing,
}

impl std::fmt::Display for OpcuaServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuilderRejected(msg) => write!(f, "OPC UA ServerBuilder rejected: {msg}"),
            Self::NamespaceLookupFailed => write!(f, "L1 namespace URI lookup failed"),
            Self::NodeManagerMissing => write!(f, "L1 simple node manager not registered"),
        }
    }
}

impl std::error::Error for OpcuaServerError {}

/// Build and start the L1 OPC UA server.
///
/// Returns once the listener is bound; the server task and the
/// publisher task continue running on the current tokio runtime.
/// The caller (`svdc-bin`) is responsible for writing fresh
/// snapshots into [`OpcuaServer::latest`].
pub fn start(cfg: OpcuaServerConfig) -> Result<OpcuaServer, OpcuaServerError> {
    // `new_anonymous` is the only constructor that pre-populates a
    // ServerEndpoint and the discovery URLs — without it the
    // builder rejects with "Server configuration is invalid. It
    // defines no endpoints". Anonymous + no-security matches the
    // ADR-0017 §5 thin slice; PR L++ swaps this for
    // `new()` + explicit endpoint config when security lands.
    let (server, handle) = ServerBuilder::new_anonymous("SVDC L1 OPC UA Server")
        .application_uri("urn:svdc:l1")
        .product_uri("https://github.com/yongalee/SVDC")
        .host(cfg.bind_addr.ip().to_string())
        .port(cfg.bind_addr.port())
        .build_info(build_info())
        .with_node_manager(simple_node_manager(
            NamespaceMetadata {
                namespace_uri: NAMESPACE_URI.to_owned(),
                ..Default::default()
            },
            MANAGER_KEY,
        ))
        .trust_client_certs(true)
        .build()
        .map_err(OpcuaServerError::BuilderRejected)?;

    let manager = handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or(OpcuaServerError::NodeManagerMissing)?;
    let ns = handle
        .get_namespace_index(NAMESPACE_URI)
        .ok_or(OpcuaServerError::NamespaceLookupFailed)?;
    let subscriptions = handle.subscriptions().clone();

    // Build the address space from PR K's node list.
    let nodes = build_nodes(&cfg.substation_name, &cfg.mu_specs);
    populate_address_space(&manager, ns, &nodes);

    // Shared snapshot the daemon writes into.
    let latest: Arc<Mutex<LatestTickSnapshot>> =
        Arc::new(Mutex::new(LatestTickSnapshot::default()));

    // Publisher task — periodically pushes the snapshot into the
    // node manager via set_values.
    let publisher_task = spawn_publisher(
        Arc::clone(&latest),
        manager.clone(),
        subscriptions,
        ns,
        cfg.mu_specs.clone(),
        cfg.publish_interval,
    );

    // Server task — blocks until cancelled.
    let server_task = tokio::spawn(async move { server.run().await.map_err(|e| format!("{e:?}")) });

    Ok(OpcuaServer {
        latest,
        server_task,
        publisher_task,
        server_handle: handle,
    })
}

fn build_info() -> BuildInfo {
    BuildInfo {
        product_uri: "https://github.com/yongalee/SVDC".into(),
        manufacturer_name: "Shinsung Industrial Electric (SSIEC)".into(),
        product_name: "SVDC L1 OPC UA Server".into(),
        software_version: env!("CARGO_PKG_VERSION").into(),
        build_number: "1".into(),
        build_date: DateTime::now(),
    }
}

/// Convert each [`AddressSpaceNode`] from [`build_nodes`] into the
/// equivalent `add_folder` / `add_variables` call on the
/// `async-opcua` address space.
fn populate_address_space(
    manager: &Arc<InMemoryNodeManager<SimpleNodeManagerImpl>>,
    ns: u16,
    nodes: &[AddressSpaceNode],
) {
    let address_space = manager.address_space();
    let mut address_space = address_space.write();
    for n in nodes {
        let node_id = NodeId::new(ns, n.node_id.clone());
        match n.kind {
            NodeKind::Folder => {
                let parent_id = parent_node_id(n.parent.as_deref(), ns);
                let _ = address_space.add_folder(
                    &node_id,
                    n.browse_name.as_str(),
                    n.browse_name.as_str(),
                    &parent_id,
                );
            }
            NodeKind::Object => {
                // The simple node manager uses folders for objects too in this slice.
                let parent_id = parent_node_id(n.parent.as_deref(), ns);
                let _ = address_space.add_folder(
                    &node_id,
                    n.browse_name.as_str(),
                    n.browse_name.as_str(),
                    &parent_id,
                );
            }
            NodeKind::Variable(kind) => {
                let parent_id = parent_node_id(n.parent.as_deref(), ns);
                let var = build_variable(kind, &node_id, &n.browse_name);
                let _ = address_space.add_variables(vec![var], &parent_id);
            }
        }
    }
}

/// Resolve a parent node ID. `None` means "the root of the OPC UA
/// objects folder"; any other value is interpreted as an SVDC
/// string node ID in `ns`.
fn parent_node_id(parent: Option<&str>, ns: u16) -> NodeId {
    match parent {
        Some(p) => NodeId::new(ns, p.to_string()),
        None => NodeId::objects_folder_id(),
    }
}

/// Default-typed variable for a given [`VariableKind`]. Initial
/// values are zero / empty; the publisher task overwrites them
/// as soon as the first tick lands.
fn build_variable(kind: VariableKind, node_id: &NodeId, browse: &str) -> Variable {
    match kind {
        VariableKind::InstMagI | VariableKind::TickId => {
            Variable::new(node_id, browse, browse, 0_i32)
        }
        VariableKind::InstMagF => Variable::new(node_id, browse, browse, 0_f32),
        VariableKind::Quality => Variable::new(node_id, browse, browse, 0_u16),
        VariableKind::Time => Variable::new(node_id, browse, browse, DateTime::null()),
        VariableKind::LastTickId | VariableKind::LastTsUtcNs => {
            Variable::new(node_id, browse, browse, 0_u64)
        }
        VariableKind::NChannels => Variable::new(node_id, browse, browse, 0_u16),
    }
}

/// Spawn the tokio task that copies [`LatestTickSnapshot`] into
/// the address space at `publish_interval`.
fn spawn_publisher(
    latest: Arc<Mutex<LatestTickSnapshot>>,
    manager: Arc<InMemoryNodeManager<SimpleNodeManagerImpl>>,
    subscriptions: Arc<SubscriptionCache>,
    ns: u16,
    mu_specs: Vec<MuFolderSpec>,
    publish_interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(publish_interval);
        loop {
            interval.tick().await;
            let snapshot = latest.lock().clone();
            if snapshot.tick_id == 0 && snapshot.samples.is_empty() {
                // No data yet — keep ticking but don't push zeros.
                continue;
            }
            let updates = build_updates(ns, &mu_specs, &snapshot);
            // `set_values` wants `(&NodeId, Option<&NumericRange>, DataValue)`
            // borrowed from a backing vec; we rebuild the iterator here so
            // each iteration owns the references it needs.
            let iter = updates.iter().map(|(id, value)| (id, None, value.clone()));
            let _ = manager.set_values(&subscriptions, iter);
        }
    })
}

/// Build the `(NodeId, DataValue)` pairs that `set_values`
/// consumes (after the publisher wraps each in the
/// `(&NodeId, Option<&NumericRange>, DataValue)` triple the
/// async-opcua API requires).
fn build_updates(
    ns: u16,
    mu_specs: &[MuFolderSpec],
    snapshot: &LatestTickSnapshot,
) -> Vec<(NodeId, DataValue)> {
    let mut out: Vec<(NodeId, DataValue)> = Vec::new();
    let source_ts = DateTime::from(utc_ns_to_opcua_ticks(snapshot.ts_utc_ns));
    let server_ts = DateTime::now();

    for mu in mu_specs {
        let registry_prefix = format!("s=Substations.Demo.{}.ChannelRegistry", mu.sv_id);
        for (idx, name) in mu.channel_names.iter().enumerate() {
            let ch_prefix = format!("{}.Ch{:02}_{}", registry_prefix, idx, name);
            let (raw, calib, quality, origin) =
                snapshot.samples.get(idx).copied().unwrap_or((0, 0.0, 0, 1));
            let base_status = iec61850_to_opcua_status(quality);
            let status = apply_origin_override(base_status, origin);
            let status_code = opcua::types::StatusCode::from(status);

            push_value(
                &mut out,
                ns,
                format!("{}.instMag.i", ch_prefix),
                DataValue {
                    value: Some(raw.into()),
                    status: Some(status_code),
                    source_timestamp: Some(source_ts),
                    server_timestamp: Some(server_ts),
                    source_picoseconds: None,
                    server_picoseconds: None,
                },
            );
            push_value(
                &mut out,
                ns,
                format!("{}.instMag.f", ch_prefix),
                DataValue {
                    value: Some(calib.into()),
                    status: Some(status_code),
                    source_timestamp: Some(source_ts),
                    server_timestamp: Some(server_ts),
                    source_picoseconds: None,
                    server_picoseconds: None,
                },
            );
            push_value(
                &mut out,
                ns,
                format!("{}.q", ch_prefix),
                DataValue::new_now(quality as u16),
            );
            push_value(
                &mut out,
                ns,
                format!("{}.t", ch_prefix),
                DataValue::new_now(source_ts),
            );
            push_value(
                &mut out,
                ns,
                format!("{}.tick_id", ch_prefix),
                DataValue::new_now(snapshot.tick_id as i32),
            );
        }

        let status_prefix = format!("s=Substations.Demo.{}.TickStatus", mu.sv_id);
        push_value(
            &mut out,
            ns,
            format!("{}.last_tick_id", status_prefix),
            DataValue::new_now(snapshot.tick_id),
        );
        push_value(
            &mut out,
            ns,
            format!("{}.last_ts_utc_ns", status_prefix),
            DataValue::new_now(snapshot.ts_utc_ns),
        );
        push_value(
            &mut out,
            ns,
            format!("{}.n_channels", status_prefix),
            DataValue::new_now(snapshot.n_channels),
        );
    }
    out
}

fn push_value(out: &mut Vec<(NodeId, DataValue)>, ns: u16, string_id: String, value: DataValue) {
    out.push((NodeId::new(ns, string_id), value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_10hz_publish_interval() {
        let addr: SocketAddr = "127.0.0.1:4840".parse().unwrap();
        let cfg = OpcuaServerConfig::reference(addr, "MU01");
        assert_eq!(cfg.publish_interval, DEFAULT_PUBLISH_INTERVAL);
        assert_eq!(cfg.publish_interval, Duration::from_millis(100));
        assert_eq!(cfg.mu_specs.len(), 1);
        assert_eq!(cfg.mu_specs[0].channel_names.len(), 8);
    }

    #[test]
    fn latest_tick_snapshot_clear_resets_all_fields() {
        let mut snap = LatestTickSnapshot {
            tick_id: 42,
            ts_utc_ns: 1_700_000_000_000_000_000,
            n_channels: 8,
            samples: vec![(1, 1.0, 0, 1); 8],
        };
        snap.clear();
        assert_eq!(snap.tick_id, 0);
        assert_eq!(snap.ts_utc_ns, 0);
        assert_eq!(snap.n_channels, 0);
        assert!(snap.samples.is_empty());
    }

    #[test]
    fn build_updates_emits_five_vars_per_channel_plus_three_status_vars() {
        let mu = MuFolderSpec::reference("MU01");
        let snap = LatestTickSnapshot {
            tick_id: 480,
            ts_utc_ns: 1_700_000_000_000_000_000,
            n_channels: 8,
            samples: vec![(1234, 1.234, 0, 1); 8],
        };
        let updates = build_updates(2, &[mu], &snap);
        // 8 channels × 5 vars + 3 TickStatus vars = 43.
        assert_eq!(updates.len(), 43);
    }
}
