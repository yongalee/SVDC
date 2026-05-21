//! `GET /south/mus/:id` — Merging Unit detail.
//!
//! Shows a live 8-channel inline-SVG oscilloscope (3-phase V + N,
//! 3-phase I + N) fed by the [`crate::sse`] WaveformSample stream at
//! ≤10 Hz. The downsampler that produces those samples lives on the
//! daemon side; the Console subscribes and draws.
//!
//! OWNER: claude-code (WBS-9.3a)

use axum::extract::Path;
use axum::{routing::get, Router};
use maud::{html, Markup, PreEscaped};

use crate::templates::base::{layout, Section};

/// Build the MU-detail sub-router.
pub fn router() -> Router {
    Router::new().route("/south/mus/:id", get(mu_detail))
}

async fn mu_detail(Path(id): Path<String>) -> Markup {
    layout(
        Section::Southbound,
        &format!("MU {id}"),
        mu_detail_body(&id),
    )
}

fn mu_detail_body(id: &str) -> Markup {
    html! {
        section.mu-detail {
            div.mu-detail-head {
                div {
                    h2.mu-id { "Merging Unit " (id) }
                    p.muted id="mu-status-line" { "Awaiting telemetry…" }
                }
                div.mu-detail-actions {
                    a.btn-secondary href="/south/mus" { "← All MUs" }
                }
            }

            (waveform_panel("Voltage (V)", "voltage", VOLTAGE_TRACES))
            (waveform_panel("Current (A)", "current", CURRENT_TRACES))

            details.mu-detail-trace {
                summary { "Live sample feed (last 8 samples)" }
                pre.mono id="mu-sample-log" { "(waiting for data)" }
            }
        }
        script type="module" {
            (PreEscaped(WAVEFORM_JS))
        }
    }
}

const VOLTAGE_TRACES: &[(&str, &str)] = &[("v1", "Va"), ("v2", "Vb"), ("v3", "Vc"), ("v0", "Vn")];

const CURRENT_TRACES: &[(&str, &str)] = &[("i1", "Ia"), ("i2", "Ib"), ("i3", "Ic"), ("i0", "In")];

fn waveform_panel(title: &str, kind: &str, traces: &[(&str, &str)]) -> Markup {
    html! {
        section.waveform-panel data-kind=(kind) {
            div.waveform-head {
                h3 { (title) }
                div.waveform-legend {
                    @for (key, label) in traces {
                        span.legend-item .{ "trace-" (key) } {
                            span.legend-swatch {}
                            (label)
                        }
                    }
                }
            }
            div.waveform-viewport {
                svg
                    id={ "svg-" (kind) }
                    viewBox="0 0 1000 240"
                    preserveAspectRatio="none"
                    role="img"
                    aria-label={ "8-channel " (title) " trace" } {
                    rect.bg x="0" y="0" width="1000" height="240" {}
                    line.grid x1="0" y1="120" x2="1000" y2="120" {}
                    @for (key, _) in traces {
                        path id={ "path-" (kind) "-" (key) } .{ "trace-" (key) } d="" fill="none" {}
                    }
                }
            }
        }
    }
}

/// Client-side waveform renderer.
///
/// Maintains a fixed-size ring per channel (last N samples, default 300
/// = 30 s at 10 Hz) and re-renders the SVG paths on each tick. Auto-
/// scales to the observed amplitude with a slow IIR (no jitter on the
/// y-axis when amplitude is steady).
const WAVEFORM_JS: &str = r#"
const WINDOW = 300;          /* ~30 s at 10 Hz */
const VIEW_W = 1000;
const VIEW_H = 240;
const VIEW_MID = VIEW_H / 2;
const PADDING_Y = 20;

const channels = {
  voltage: ['v1', 'v2', 'v3', 'v0'],
  current: ['i1', 'i2', 'i3', 'i0'],
};

const buffers = {};
for (const kind of Object.keys(channels)) {
  buffers[kind] = {};
  for (const key of channels[kind]) buffers[kind][key] = [];
}

const scale = { voltage: 1, current: 1 };
let lastUpdate = 0;

function pushSample(kind, key, value) {
  const buf = buffers[kind][key];
  buf.push(value);
  if (buf.length > WINDOW) buf.shift();
}

function autoScale(kind) {
  let m = 0;
  for (const key of channels[kind]) {
    const buf = buffers[kind][key];
    for (let i = 0; i < buf.length; i++) {
      const a = Math.abs(buf[i]);
      if (a > m) m = a;
    }
  }
  if (m === 0) return;
  /* Slow IIR toward observed peak so the y-axis does not jitter. */
  scale[kind] = scale[kind] * 0.85 + m * 0.15;
}

function buildPath(buf, kind) {
  if (buf.length === 0) return '';
  const s = scale[kind] || 1;
  const halfH = (VIEW_H - 2 * PADDING_Y) / 2;
  const stepX = VIEW_W / Math.max(1, WINDOW - 1);
  let d = '';
  for (let i = 0; i < buf.length; i++) {
    const x = i * stepX;
    const y = VIEW_MID - (buf[i] / s) * halfH;
    d += (i === 0 ? 'M ' : 'L ') + x.toFixed(1) + ' ' + y.toFixed(1) + ' ';
  }
  return d;
}

function render() {
  for (const kind of Object.keys(channels)) {
    autoScale(kind);
    for (const key of channels[kind]) {
      const el = document.getElementById('path-' + kind + '-' + key);
      if (el) el.setAttribute('d', buildPath(buffers[kind][key], kind));
    }
  }
}

const es = new EventSource('/sse/dashboard');
const statusLine = document.getElementById('mu-status-line');
const sampleLog = document.getElementById('mu-sample-log');
const sampleLogRing = [];

es.onmessage = (evt) => {
  let p;
  try { p = JSON.parse(evt.data); } catch (_) { return; }
  if (p.event_type !== 'Waveform') return;
  const w = p.data;

  pushSample('voltage', 'v1', w.v1);
  pushSample('voltage', 'v2', w.v2);
  pushSample('voltage', 'v3', w.v3);
  pushSample('voltage', 'v0', w.v0);
  pushSample('current', 'i1', w.i1);
  pushSample('current', 'i2', w.i2);
  pushSample('current', 'i3', w.i3);
  pushSample('current', 'i0', w.i0);

  if (statusLine) {
    statusLine.textContent =
      'MU ' + (w.mu_id || '?') + ' · ' +
      new Date(w.timestamp_ms).toISOString() + ' · ' +
      buffers.voltage.v1.length + ' samples buffered';
  }
  if (sampleLog) {
    sampleLogRing.push(
      new Date(w.timestamp_ms).toISOString().substring(11, 23) + '  ' +
      'V[' + w.v1.toFixed(1) + ' ' + w.v2.toFixed(1) + ' ' + w.v3.toFixed(1) + ' ' + w.v0.toFixed(1) + ']  ' +
      'I[' + w.i1.toFixed(2) + ' ' + w.i2.toFixed(2) + ' ' + w.i3.toFixed(2) + ' ' + w.i0.toFixed(2) + ']'
    );
    while (sampleLogRing.length > 8) sampleLogRing.shift();
    sampleLog.textContent = sampleLogRing.join('\n');
  }

  const now = performance.now();
  if (now - lastUpdate > 80) {
    /* Cap render at ~12 fps to keep the CPU quiet. */
    lastUpdate = now;
    render();
  }
};

es.onerror = () => { /* let the browser auto-reconnect */ };
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn renders_three_phase_legend() {
        let m = mu_detail_body("MU-01");
        let html = m.into_string();
        for label in ["Va", "Vb", "Vc", "Vn", "Ia", "Ib", "Ic", "In"] {
            assert!(html.contains(label), "missing legend label {label}");
        }
    }

    #[test]
    fn renders_svg_viewports() {
        let html = mu_detail_body("MU-01").into_string();
        assert!(html.contains(r#"id="svg-voltage""#));
        assert!(html.contains(r#"id="svg-current""#));
        assert!(html.contains(r#"viewBox="0 0 1000 240""#));
    }

    #[test]
    fn js_subscribes_to_waveform_events() {
        assert!(WAVEFORM_JS.contains("'Waveform'"));
        assert!(WAVEFORM_JS.contains("/sse/dashboard"));
        for ch in ["v1", "v2", "v3", "v0", "i1", "i2", "i3", "i0"] {
            assert!(
                WAVEFORM_JS.contains(&format!("'{ch}'")),
                "JS missing channel {ch}"
            );
        }
    }
}
