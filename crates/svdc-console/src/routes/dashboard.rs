//! `GET /` — Dashboard (system overview).
//!
//! Four primary tiles per UI Doc §4.1: PTP, Circular Buffer, MU count,
//! Throughput. Each tile id matches an SSE event field so the
//! client-side updater (see [`SSE_UPDATER_JS`]) can swap content
//! without a full reload.
//!
//! OWNER: claude-code (WBS-9.2a)

use axum::{routing::get, Router};
use maud::{html, Markup, PreEscaped};

use crate::templates::base::{layout, Section};
use crate::templates::components::{tile, StatusLevel};

/// Build the Dashboard sub-router.
pub fn router() -> Router {
    Router::new().route("/", get(dashboard))
}

async fn dashboard() -> Markup {
    layout(Section::Dashboard, "Dashboard", dashboard_body())
}

fn dashboard_body() -> Markup {
    html! {
        section.dashboard-grid hx-ext="sse" sse-connect="/sse/dashboard" {
            (tile(
                "PTP synchronization",
                "—",
                Some("offset 0 ns · 0 ms path delay"),
                StatusLevel::Unknown,
                Some("tile-ptp"),
            ))
            (tile(
                "Circular buffer",
                "—%",
                Some("0 / 5,120 records · dual CB"),
                StatusLevel::Unknown,
                Some("tile-buffer"),
            ))
            (tile(
                "Merging Units",
                "0",
                Some("0 healthy · 0 degraded · 0 faulted"),
                StatusLevel::Unknown,
                Some("tile-mus"),
            ))
            (tile(
                "Throughput",
                "0 Hz",
                Some("p99 latency — ns"),
                StatusLevel::Unknown,
                Some("tile-throughput"),
            ))
        }
        section.dashboard-aux {
            p.muted {
                "Live tiles update from the daemon over Server-Sent Events. "
                "When no daemon data is flowing, mock telemetry is shown."
            }
        }
        script type="module" {
            (PreEscaped(SSE_UPDATER_JS))
        }
    }
}

/// Client-side SSE consumer that updates the four tiles in place.
///
/// Subscribes to `/sse/dashboard` and routes events by `event_type`
/// to the matching tile element. Implemented in vanilla JS to avoid
/// a framework dependency per ADR-0004.
const SSE_UPDATER_JS: &str = r#"
const es = new EventSource('/sse/dashboard');
const tile = (id) => document.getElementById(id);
const set = (id, primary, secondary, level) => {
  const el = tile(id);
  if (!el) return;
  const primaryEl = el.querySelector('.tile-primary');
  const secondaryEl = el.querySelector('.tile-secondary');
  const pill = el.querySelector('.status-pill');
  if (primaryEl) primaryEl.textContent = primary;
  if (secondaryEl && secondary != null) secondaryEl.textContent = secondary;
  if (pill && level) {
    pill.className = 'status-pill status-' + level;
    pill.textContent = level.charAt(0).toUpperCase() + level.slice(1);
  }
};

const ptpLevel = (offset_ns) => {
  if (offset_ns == null) return 'unknown';
  const a = Math.abs(offset_ns);
  if (a <= 100) return 'healthy';
  if (a <= 1000) return 'degraded';
  return 'fault';
};

es.onmessage = (evt) => {
  let p;
  try { p = JSON.parse(evt.data); } catch (_) { return; }
  if (p.event_type === 'Metrics') {
    const m = p.data;
    set('tile-ptp',
        (m.ptp_offset_ns ?? 0) + ' ns',
        m.ptp_sync_status + ' · mock until Phase 5',
        ptpLevel(m.ptp_offset_ns));
    // Buffer tile carries the live integrity verdict in its
    // secondary line so the operator sees CRC failures at a
    // glance.
    const violations = m.integrity_violations ?? 0;
    const bufferSecondary = (violations === 0)
        ? 'integrity ok · CRC verified'
        : ('degraded · ' + violations + ' integrity violation(s)');
    const bufferLevel = (violations > 0)
        ? 'fault'
        : ((m.buffer_saturation < 70) ? 'healthy'
            : (m.buffer_saturation < 90 ? 'degraded' : 'fault'));
    set('tile-buffer',
        (m.buffer_saturation ?? 0).toFixed(1) + '%',
        bufferSecondary,
        bufferLevel);
    const muSecondary = m.live_feed_active
        ? 'live UDP feed · auto-registration in PR D'
        : ((m.active_mus > 0) ? 'in-process demo loop active'
                              : 'no producer attached');
    set('tile-mus',
        String(m.active_mus ?? 0),
        muSecondary,
        (m.active_mus > 0) ? 'healthy' : 'unknown');
    set('tile-throughput',
        (m.sps_rate ?? 0).toLocaleString() + ' Hz',
        (m.sps_rate > 0)
            ? ('live ' + (m.sps_rate ?? 0).toLocaleString() + ' ticks/s')
            : 'idle · waiting for producer',
        (m.sps_rate > 0) ? 'healthy' : 'unknown');
  }
};
es.onerror = () => {
  /* let the browser auto-reconnect; nothing to surface in v0.1 */
};
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn sse_updater_handles_metrics_shape() {
        // The JS string must reference the event_type tag used by
        // `SsePayload::Metrics` so a typo here is caught at review.
        assert!(SSE_UPDATER_JS.contains("event_type"));
        assert!(SSE_UPDATER_JS.contains("Metrics"));
        assert!(SSE_UPDATER_JS.contains("tile-ptp"));
    }
}
