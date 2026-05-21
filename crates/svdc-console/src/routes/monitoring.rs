//! `GET /monitoring` — Performance and audit.
//!
//! Phase 0/3 renders a latency histogram fed from a deterministic
//! mock source (`templates::charts::mock_histogram`). Phase 5 replaces
//! the data source with the daemon's real HDR histogram, exposes a
//! 1 Hz SSE refresh, and adds the PTP/CB line charts + audit table
//! (those are WBS-9.5b, Antigravity lane).
//!
//! OWNER: claude-code (WBS-9.5a).

use axum::{routing::get, Router};
use maud::{html, Markup};

use crate::templates::base::{layout, Section};
use crate::templates::charts::{histogram_kpis, latency_histogram_svg, mock_histogram, Histogram};

/// Build the Monitoring sub-router.
pub fn router() -> Router {
    Router::new().route("/monitoring", get(monitoring))
}

async fn monitoring() -> Markup {
    // Deterministic seed → same picture on every refresh. Phase 5 will
    // swap this for the daemon's live counters.
    let hist = mock_histogram(20260521, 8_000, 50_000.0);

    layout(Section::Monitoring, "Monitoring", monitoring_body(&hist))
}

fn monitoring_body(hist: &Histogram) -> Markup {
    html! {
        section.config-section {
            div.config-section-head {
                h2 { "End-to-end latency" }
                p.muted {
                    "Distribution of per-frame protection-path latency in nanoseconds, "
                    "log-scale x-axis. Tile values are p50, p99, p99.9 percentiles. "
                    "Phase 0/3 shows a deterministic mock distribution; Phase 5 wires "
                    "the daemon's HDR histogram in."
                }
            }
            (histogram_kpis(hist))
            div.histogram-viewport {
                (latency_histogram_svg(hist))
            }
        }
        section.placeholder {
            p.muted {
                "PTP offset chart, circular-buffer occupancy chart, and audit-log "
                "table land under WBS-9.5b (Antigravity, Phase 5)."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn monitoring_body_contains_kpis_and_svg() {
        let hist = mock_histogram(1, 500, 50_000.0);
        let s = monitoring_body(&hist).into_string();
        assert!(s.contains("samples"));
        assert!(s.contains("p50"));
        assert!(s.contains("histogram-svg") || s.contains("class=\"histogram-svg\""));
    }
}
