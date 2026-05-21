//! Operator Console templates (maud).
//!
//! Composition rule: every full page goes through [`base::layout`].
//! Small repeated fragments live in [`components`].
//!
//! OWNER: claude-code (WBS-9.1a base; component sub-files split by WBS lane)
//! NFR-10: English-only.

pub mod base;
pub mod components;
