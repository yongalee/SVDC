//! L1 northbound layer — OPC UA Information Model for the SVDC.
//!
//! Implements the address-space layout and value mappings locked in
//! [ADR-0017](../../docs/decisions/0017-l1-opcua-server.md):
//!
//! - [`address_space`] — library-neutral `AddressSpaceNode` list
//!   builder driven by a [`MuFolderSpec`]. PR L will iterate the
//!   list and call `opcua::server::AddressSpace::add_…` for each
//!   node; this crate stays free of the `opcua` dependency so the
//!   mapping is unit-testable in isolation and survives a library
//!   swap (the trade-off matrix in ADR-0017 §1 is the relevant
//!   discussion).
//! - [`quality`] — IEC 61850 `q` byte → OPC UA `StatusCode` per
//!   OPC 10040 §6.3 with the substatus override table from
//!   ADR-0017 §3.
//! - [`timestamp`] — `TickRecord.ts_utc_ns` → OPC UA `DateTime`
//!   100-ns ticks since 1601-01-01 UTC. Round-trippable; pure.
//!
//! No daemon wiring lives here. The thin-slice server (PR L) will
//! consume this crate's public API to build its own `AddressSpace`
//! and run a `tokio` task that polls the shared `TickBuffer` via
//! `svdc-subscribe`. The `--enable-opcua` flag, the `/north/L1`
//! UI integration, and the `--allow-insecure-bind` guard from
//! ADR-0017 §5 all land in PR L.
//!
//! OWNER: claude-code (WBS-3.7). NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod address_space;
pub mod quality;
pub mod timestamp;

pub use address_space::{
    build_nodes, AddressSpaceNode, ChannelLayout, MuFolderSpec, NodeKind, VariableKind,
    REFERENCE_CHANNELS,
};
pub use quality::{apply_origin_override, iec61850_to_opcua_status, q_bits, status_codes};
pub use timestamp::{utc_ns_to_opcua_ticks, NS_PER_OPCUA_TICK, UNIX_TO_OPCUA_EPOCH_NS};
