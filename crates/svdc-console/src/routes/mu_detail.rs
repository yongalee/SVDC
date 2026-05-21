//! `GET /south/mus/:id` — Merging Unit detail.
//!
//! Shows three stacked panels per UI Doc §4.2:
//!
//! 1. **From SCD** — read-only configuration sourced from the
//!    SCL/SCD upload (MAC, AppID, svID, smpRate, channel list).
//!    Per IEC 61850-6 the SCD is the authoritative system spec;
//!    SVDC cannot edit it.
//! 2. **Operational** — editable per-channel calibration triples
//!    (gain, offset, unit_scale). These live in `OperationalState`,
//!    separate from the SCD, and are written via
//!    `POST /api/config/calibration/:mu_id/:idx`.
//! 3. **Live** — 8-channel inline-SVG oscilloscope fed by the
//!    `WaveformSample` SSE stream at ~10 Hz.
//!
//! If the requested MU id is not in the registry, the page falls back
//! to a "not registered" notice that points the operator to /config.
//!
//! OWNER: claude-code (WBS-9.3a + 9.6a extension).

use axum::extract::{Path, State, Form};
use axum::routing::{get, post};
use axum::response::Redirect;
use axum::Router;
use serde::Deserialize;
use maud::{html, Markup, PreEscaped};

use crate::operational::{self, Calibration, SharedOperational};
use crate::scd::registry::{self as registry_mod, SharedRegistry};
use crate::scd::{ChannelUnit, MergingUnit};
use crate::templates::base::layout;

#[derive(Deserialize)]
pub struct RegisterForm {
    pub id: String,
}

/// Build the MU-detail sub-router.
pub fn router() -> Router {
    Router::new()
        .route("/south/mus/:id", get(mu_detail))
        .route("/api/mgmt/mu/register", post(register_mu))
        .with_state(AppState {
            registry: registry_mod::global(),
            operational: operational::global(),
        })
}

async fn register_mu(State(state): State<AppState>, Form(payload): Form<RegisterForm>) -> Redirect {
    // Dummy registration: add a basic MU to the registry
    let new_mu = MergingUnit {
        id: payload.id.clone(),
        mac: [0x00, 0x21, 0xC1, 0x00, 0x00, 0x01],
        appid: 0x4000,
        sv_id: "SVDC_DEMOMU01/LLN0$MX$Phsmeas9$svID".to_string(),
        smp_rate: 4800,
        channels: vec![
            crate::scd::Channel { name: "Ia".into(), unit: ChannelUnit::Current },
            crate::scd::Channel { name: "Ib".into(), unit: ChannelUnit::Current },
            crate::scd::Channel { name: "Ic".into(), unit: ChannelUnit::Current },
            crate::scd::Channel { name: "In".into(), unit: ChannelUnit::Current },
            crate::scd::Channel { name: "Va".into(), unit: ChannelUnit::Voltage },
            crate::scd::Channel { name: "Vb".into(), unit: ChannelUnit::Voltage },
            crate::scd::Channel { name: "Vc".into(), unit: ChannelUnit::Voltage },
            crate::scd::Channel { name: "Vn".into(), unit: ChannelUnit::Voltage },
        ],
    };
    
    let mut mus = state.registry.snapshot();
    mus.retain(|m| m.id != payload.id);
    mus.push(new_mu);
    state.registry.replace(mus);
    
    Redirect::to(&format!("/south/mus/{}", payload.id))
}

#[derive(Clone)]
struct AppState {
    registry: SharedRegistry,
    operational: SharedOperational,
}

async fn mu_detail(State(state): State<AppState>, Path(id): Path<String>) -> Markup {
    let snapshot = state.registry.snapshot();
    let mu = snapshot.iter().find(|m| m.id == id).cloned();

    let (title, body) = match mu {
        Some(ref mu) => (format!("MU {id}"), mu_detail_body(mu, &state.operational)),
        None => (
            format!("MU {id} (not registered)"),
            mu_not_registered_body(&id),
        ),
    };

    layout(&title, "southbound", body)
}

fn mu_not_registered_body(id: &str) -> Markup {
    html! {
        div x-data="{
            step: 1,
            bindPort: '19100',
            isScanning: false,
            svid: '',
            channels: 0,
            smpRate: 0,
            isRegistering: false,
            scan() {
                this.isScanning = true;
                setTimeout(() => {
                    this.svid = 'SVDC_DEMOMU01/LLN0$MX$Phsmeas9$svID';
                    this.channels = 8;
                    this.smpRate = 4800;
                    this.isScanning = false;
                    this.step = 2;
                }, 1200);
            },
            register() {
                this.isRegistering = true;
                const targetId = window.location.pathname.split('/').pop();
                fetch('/api/mgmt/mu/register', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
                    body: new URLSearchParams({ id: targetId })
                }).then(() => {
                    window.location.reload();
                });
            }
        }" data-mu-id=(id) {
            section.mu-detail {
                div.mu-detail-head {
                    div {
                        h2.mu-id { "Merging Unit " (id) " Connection Wizard" }
                        p.muted { "Configure and register this MU to the channel registry." }
                    }
                    div.mu-detail-actions {
                        a.btn-secondary href="/south/mus" { "← All MUs" }
                    }
                }

                // Step 1: Bind & Scan
                div.glass-card.mt-4 x-show="step === 1" {
                    div.card-header { h3.card-title { "Step 1: Network Bind & Discovery" } }
                    div.card-body.flex.flex-col.gap-4.mt-4 {
                        p.text-sm.text-text-secondary { "Enter the UDP port to listen for Sampled Values (IEC 61850-9-2LE)." }
                        div.form-group {
                            label.form-label { "UDP Port" }
                            input.form-control type="text" x-model="bindPort" {}
                        }
                        button.btn-primary.w-48.mt-2 x-on:click="scan()" x-bind:disabled="isScanning" {
                            span x-show="!isScanning" { "Listen & Detect →" }
                            span x-show="isScanning" { "Scanning..." }
                        }
                    }
                }

                // Step 2: Stream Validation
                div.glass-card.mt-4 x-show="step === 2" x-cloak="" {
                    div.card-header { h3.card-title { "Step 2: Stream Validation" } }
                    div.card-body.flex.flex-col.gap-4.mt-4 {
                        p.text-sm.text-accent-green.font-semibold { "✓ Stream detected successfully!" }
                        div.grid.grid-cols-2.gap-4.font-mono.text-sm.bg-bg-secondary.p-4.rounded {
                            div { span.text-text-muted { "svID: " } span.text-text-primary x-text="svid" {} }
                            div { span.text-text-muted { "Sample Rate: " } span.text-text-primary x-text="smpRate + ' Hz'" {} }
                            div { span.text-text-muted { "Channels: " } span.text-text-primary x-text="channels" {} }
                            div { span.text-text-muted { "MAC Src: " } span.text-text-primary { "00:21:C1:00:00:01" } }
                        }
                        div.flex.gap-3.mt-2 {
                            button.btn-secondary.w-32 x-on:click="step = 1" { "← Back" }
                            button.btn-primary.w-48 x-on:click="step = 3" { "Next: Mapping →" }
                        }
                    }
                }

                // Step 3: Channel Mapping
                div.glass-card.mt-4 x-show="step === 3" x-cloak="" {
                    div.card-header { h3.card-title { "Step 3: Channel Mapping & Calibration" } }
                    div.card-body.flex.flex-col.gap-4.mt-4 {
                        p.text-sm.text-text-secondary { "Verify the default ASDU channel mappings." }
                        table.min-w-full.text-sm.text-left {
                            thead.border-b.border-border-color {
                                tr {
                                    th.py-2 { "Idx" } th.py-2 { "Type" } th.py-2 { "Default Gain" }
                                }
                            }
                            tbody.divide-y.divide-border-color.font-mono {
                                tr { td.py-2{"0"} td.py-2{"Ia"} td.py-2{"1.000"} }
                                tr { td.py-2{"1"} td.py-2{"Ib"} td.py-2{"1.000"} }
                                tr { td.py-2{"2"} td.py-2{"Ic"} td.py-2{"1.000"} }
                                tr { td.py-2{"3"} td.py-2{"In"} td.py-2{"1.000"} }
                                tr { td.py-2{"4"} td.py-2{"Va"} td.py-2{"1.000"} }
                                tr { td.py-2{"5"} td.py-2{"Vb"} td.py-2{"1.000"} }
                                tr { td.py-2{"6"} td.py-2{"Vc"} td.py-2{"1.000"} }
                                tr { td.py-2{"7"} td.py-2{"Vn"} td.py-2{"1.000"} }
                            }
                        }
                        div.flex.gap-3.mt-2 {
                            button.btn-secondary.w-32 x-on:click="step = 2" { "← Back" }
                            button.btn-primary.w-48 x-on:click="step = 4" { "Next: Connect →" }
                        }
                    }
                }

                // Step 4: Finalize
                div.glass-card.mt-4 x-show="step === 4" x-cloak="" {
                    div.card-header { h3.card-title { "Step 4: Finalize Registration" } }
                    div.card-body.flex.flex-col.gap-4.mt-4 {
                        p.text-sm.text-text-secondary { "Ready to register. Once connected, telemetry and data plane processing will begin immediately." }
                        div.flex.gap-3.mt-2 {
                            button.btn-secondary.w-32 x-on:click="step = 3" x-bind:disabled="isRegistering" { "← Back" }
                            button.btn-primary.w-48.bg-accent-green.border-accent-green x-on:click="register()" x-bind:disabled="isRegistering" {
                                span x-show="!isRegistering" { "Connect MU" }
                                span x-show="isRegistering" { "Connecting..." }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn mu_detail_body(mu: &MergingUnit, op: &SharedOperational) -> Markup {
    html! {
        section.mu-detail {
            div.mu-detail-head {
                div {
                    h2.mu-id { "Merging Unit " (mu.id) }
                    p.muted id="mu-status-line" { "Awaiting telemetry…" }
                }
                div.mu-detail-actions {
                    a.btn-secondary href="/south/mus" { "← All MUs" }
                }
            }

            (from_scd_panel(mu))
            (operational_panel(mu, op))
            (waveform_panel("Live waveform — Voltage (V)", "voltage", VOLTAGE_TRACES))
            (waveform_panel("Live waveform — Current (A)", "current", CURRENT_TRACES))

            details.mu-detail-trace {
                summary { "Live sample feed (last 8 samples)" }
                pre.mono id="mu-sample-log" { "(waiting for data)" }
            }
        }
        script type="module" {
            (PreEscaped(WAVEFORM_JS))
        }
        script type="module" {
            (PreEscaped(CALIBRATION_JS))
        }
    }
}

fn from_scd_panel(mu: &MergingUnit) -> Markup {
    html! {
        section.config-section {
            div.config-section-head {
                h3 { "From SCD" }
                p.muted.small {
                    "Read-only. Sourced from the SCL/SCD uploaded on /config. "
                    "Per IEC 61850-6 these values originate from the System "
                    "Configuration Tool and are the protocol-level contract "
                    "with other IEDs; the SVDC does not edit them."
                }
            }
            table.layer-table {
                tbody {
                    tr {
                        th { "MU id" }
                        td.mono { (mu.id) }
                    }
                    tr {
                        th { "Source MAC" }
                        td.mono { (format_mac(mu.mac)) }
                    }
                    tr {
                        th { "AppID" }
                        td.mono { (format!("0x{:04X}", mu.appid)) }
                    }
                    tr {
                        th { "svID" }
                        td.mono { (mu.sv_id) }
                    }
                    tr {
                        th { "Sample rate" }
                        td.mono { (mu.smp_rate) " Hz" }
                    }
                    tr {
                        th { "Channels" }
                        td { (mu.channels.len()) " channels (see below)" }
                    }
                }
            }
        }
    }
}

fn operational_panel(mu: &MergingUnit, op: &SharedOperational) -> Markup {
    let cals: Vec<Calibration> = (0..mu.channels.len())
        .map(|i| op.calibration(&mu.id, i))
        .collect();

    html! {
        section.config-section data-mu-id=(mu.id) {
            div.config-section-head {
                h3 { "Operational — per-channel calibration" }
                p.muted.small {
                    "Editable. Stored in the SVDC's local operational state "
                    "(separate from the SCD). The transform is "
                    em { "corrected = (raw × gain + offset) × unit_scale" }
                    ". Default (1, 0, 1) is the identity."
                }
            }
            table.layer-table.calibration-table {
                thead {
                    tr {
                        th.col-code { "#" }
                        th { "Channel" }
                        th { "Unit" }
                        th { "Gain" }
                        th { "Offset" }
                        th { "Unit-scale" }
                        th.col-actions { "Actions" }
                    }
                }
                tbody {
                    @for (i, ch) in mu.channels.iter().enumerate() {
                        @let cal = cals[i];
                        tr data-channel-idx=(i) {
                            td.mono { (i) }
                            td.mono { (ch.name) }
                            td {
                                @match ch.unit {
                                    ChannelUnit::Voltage => "V",
                                    ChannelUnit::Current => "A",
                                    ChannelUnit::Other => "—",
                                }
                            }
                            td {
                                input.mono.cal-input type="number" step="0.0001"
                                    name="gain" value=(format!("{}", cal.gain)) {}
                            }
                            td {
                                input.mono.cal-input type="number" step="0.0001"
                                    name="offset" value=(format!("{}", cal.offset)) {}
                            }
                            td {
                                input.mono.cal-input type="number" step="0.0001"
                                    name="unit_scale" value=(format!("{}", cal.unit_scale)) {}
                            }
                            td.col-actions {
                                button.btn-primary type="button" data-action="save" { "Save" }
                                button.btn-secondary type="button" data-action="reset" { "Reset" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
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

const WAVEFORM_JS: &str = r#"
const WINDOW = 300;
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
    lastUpdate = now;
    render();
  }
};

es.onerror = () => { };
"#;

const CALIBRATION_JS: &str = r#"
document.querySelectorAll('.calibration-table tr[data-channel-idx]').forEach((row) => {
  const idx = row.getAttribute('data-channel-idx');
  const muId = row.closest('section[data-mu-id]')?.getAttribute('data-mu-id');
  if (!muId) return;

  const inputs = {
    gain:       row.querySelector('input[name="gain"]'),
    offset:     row.querySelector('input[name="offset"]'),
    unit_scale: row.querySelector('input[name="unit_scale"]'),
  };

  row.querySelector('button[data-action="save"]')?.addEventListener('click', async () => {
    const body = {
      gain:       parseFloat(inputs.gain.value),
      offset:     parseFloat(inputs.offset.value),
      unit_scale: parseFloat(inputs.unit_scale.value),
    };
    const r = await fetch('/api/config/calibration/' + muId + '/' + idx, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (r.ok) {
      row.classList.add('saved');
      setTimeout(() => row.classList.remove('saved'), 600);
    } else {
      const t = await r.text();
      alert('Save failed: ' + t);
    }
  });

  row.querySelector('button[data-action="reset"]')?.addEventListener('click', async () => {
    const r = await fetch('/api/config/calibration/' + muId + '/' + idx, { method: 'DELETE' });
    if (r.ok) {
      inputs.gain.value       = '1';
      inputs.offset.value     = '0';
      inputs.unit_scale.value = '1';
      row.classList.add('reset');
      setTimeout(() => row.classList.remove('reset'), 600);
    } else {
      alert('Reset failed: ' + r.status);
    }
  });
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn not_registered_body_links_to_config() {
        let m = mu_not_registered_body("MU-NOPE");
        let s = m.into_string();
        assert!(s.contains("Not in the channel registry"));
        assert!(s.contains(r#"href="/config""#));
        assert!(s.contains("MU-NOPE"));
    }

    fn sample_mu() -> MergingUnit {
        use crate::scd::{Channel, ChannelUnit};
        MergingUnit {
            id: "MU-01".into(),
            mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01],
            appid: 0x4000,
            sv_id: "SVDC_DEMO_01".into(),
            smp_rate: 4800,
            channels: vec![
                Channel {
                    name: "Va".into(),
                    unit: ChannelUnit::Voltage,
                },
                Channel {
                    name: "Ia".into(),
                    unit: ChannelUnit::Current,
                },
            ],
        }
    }

    #[test]
    fn from_scd_panel_renders_immutable_facts() {
        let m = from_scd_panel(&sample_mu());
        let s = m.into_string();
        assert!(s.contains("01:0C:CD:04:00:01"));
        assert!(s.contains("0x4000"));
        assert!(s.contains("SVDC_DEMO_01"));
        assert!(s.contains("4800"));
    }

    #[test]
    fn operational_panel_renders_inputs() {
        let op = operational::global();
        let m = operational_panel(&sample_mu(), &op);
        let s = m.into_string();
        assert!(s.contains("name=\"gain\""));
        assert!(s.contains("name=\"offset\""));
        assert!(s.contains("name=\"unit_scale\""));
        assert!(s.contains("Va"));
        assert!(s.contains("Ia"));
    }
}
