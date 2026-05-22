//! Route modules for the Operator Console.
//!
//! Per the WBS-9 handoff plan, each `routes/<screen>.rs` is owned by a
//! single lane. This file is structural and is owned by Claude (WBS-9.1a).

pub mod assets;
pub mod audit;
pub mod calibration;
pub mod config;
pub mod dashboard;
pub mod dataplane;
pub mod l1_wizard;
pub mod monitoring;
pub mod mu_detail;
pub mod mus_list;
pub mod northbound;
pub mod sse;
