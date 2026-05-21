//! Reusable maud components.
//!
//! This file is shared. Per the WBS-9 handoff plan
//! (`docs/dual-agent/wbs-9-ui-handoff.md`), each lane owns specific
//! component fragments:
//!
//! - Dashboard tile / status pill / metric value — Claude (WBS-9.2a).
//! - MU card — Antigravity (WBS-9.3b).
//! - Layer card (L0..L3) — Antigravity (WBS-9.4b).
//!
//! Phase 0 lands only the Claude-owned bits.

use maud::{html, Markup, PreEscaped};

/// Status colour used by the small pill / dot indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    /// Green — nominal state.
    Healthy,
    /// Amber — degraded but operational.
    Degraded,
    /// Red — incident.
    Fault,
    /// Grey — unknown / not initialised.
    Unknown,
}

impl StatusLevel {
    /// CSS class fragment.
    pub fn class(self) -> &'static str {
        match self {
            StatusLevel::Healthy => "healthy",
            StatusLevel::Degraded => "degraded",
            StatusLevel::Fault => "fault",
            StatusLevel::Unknown => "unknown",
        }
    }

    /// Human-readable label for ARIA / tooltip.
    pub fn label(self) -> &'static str {
        match self {
            StatusLevel::Healthy => "Healthy",
            StatusLevel::Degraded => "Degraded",
            StatusLevel::Fault => "Fault",
            StatusLevel::Unknown => "Unknown",
        }
    }
}

/// Render a dashboard tile.
///
/// A tile shows: a small label (top), a large primary value (middle),
/// optional secondary line (bottom), and a status pill (top-right).
/// The optional `update_id` becomes the element `id`, so SSE-driven
/// JS can swap content in place.
pub fn tile(
    label: &str,
    primary: &str,
    secondary: Option<&str>,
    status: StatusLevel,
    update_id: Option<&str>,
) -> Markup {
    html! {
        section.tile id=[update_id] {
            div.tile-head {
                span.tile-label { (label) }
                span.status-pill .{ "status-" (status.class()) } title=(status.label()) {
                    (status.label())
                }
            }
            div.tile-primary { (primary) }
            @if let Some(s) = secondary {
                div.tile-secondary { (s) }
            }
        }
    }
}

/// A small inline status dot with no label.
pub fn status_dot(level: StatusLevel) -> Markup {
    html! {
        span.status-dot .{ "status-" (level.class()) } title=(level.label()) {
            (PreEscaped("&#9679;"))
        }
    }
}
