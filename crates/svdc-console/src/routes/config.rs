//! `GET /config` — Configuration screen.
//! `POST /api/config/scd` — SCD/SCL upload + validation.
//! `POST /api/config/scd/sample` — load the built-in sample SCD (one click).
//! `POST /api/config/mus` — register a single MU manually (JSON body).
//! `DELETE /api/config/mus` — clear the registry.
//! `GET /api/config/channels` — JSON snapshot of the channel registry.
//!
//! WBS-9.6a (Claude) authors the validator, the registry, the upload
//! API, the sample SCD shortcut, and the manual-register API. WBS-9.6b
//! (Antigravity) refines the upload form / About page on top.

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{html, Markup};
use serde::{Deserialize, Serialize};

use crate::scd::registry::{self as registry_mod, SharedRegistry};
use crate::scd::sample::SAMPLE_SCD_XML;
use crate::scd::{self, Channel, ChannelUnit, MergingUnit};
use crate::templates::base::{layout, Section};

/// Build the Configuration sub-router using the process-wide registry.
pub fn router() -> Router {
    Router::new()
        .route("/config", get(config_index))
        .route("/api/config/scd", post(api_upload_scd))
        .route("/api/config/scd/sample", post(api_load_sample_scd))
        .route(
            "/api/config/mus",
            post(api_register_mu).delete(api_clear_registry),
        )
        .route("/api/config/channels", get(api_channels))
        .with_state(registry_mod::global())
}

async fn config_index(State(registry): State<SharedRegistry>) -> Markup {
    let mus = registry.snapshot();
    layout(
        Section::Configuration,
        "Configuration",
        html! {
            section.config-section {
                div.config-section-head {
                    h2 { "Substation configuration (SCD)" }
                    p.muted {
                        "Upload an IEC 61850 SCL/SCD file. The Console parses "
                        "the Merging Units it describes (MAC, APPID, svID, "
                        "sample rate, channel list) and registers them for "
                        "the data plane."
                    }
                }
                form
                    id="scd-upload-form"
                    action="/api/config/scd"
                    method="post"
                    enctype="multipart/form-data" {
                    div.scd-form-row {
                        input
                            id="scd-file"
                            type="file"
                            name="scd"
                            accept=".cid,.scd,.icd,.xml"
                            required {}
                        button.btn-primary type="submit" { "Upload and validate" }
                        button.btn-secondary
                            type="button"
                            id="load-sample-scd"
                            title="Load the built-in SSIEC demo SCD with one MU and 8 channels" {
                            "Load built-in sample"
                        }
                    }
                    p.muted.small {
                        "WBS-9.6b will replace this minimal form with an HTMX-driven "
                        "live-feedback variant; the parse/validate endpoint already "
                        "returns a structured JSON result usable today via curl."
                    }
                }
            }
            section.config-section {
                div.config-section-head {
                    h2 { "Register a Merging Unit manually" }
                    p.muted {
                        "Use this when you do not have an SCD on hand and want "
                        "to register one MU directly. The fields map 1:1 to the "
                        "SCL ConnectedAP + SampledValueControl entries the "
                        "parser would otherwise extract from a file."
                    }
                }
                form
                    id="manual-mu-form"
                    action="/api/config/mus"
                    method="post" {
                    div.scd-form-grid {
                        label { "MU id"
                            input type="text" name="id" placeholder="MU-LAB-01"
                                  required pattern="[A-Za-z0-9_-]+" {}
                        }
                        label { "Source MAC"
                            input type="text" name="mac"
                                  placeholder="01:0C:CD:04:00:01"
                                  pattern="([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}"
                                  required {}
                        }
                        label { "APPID (hex)"
                            input type="text" name="appid"
                                  placeholder="4000"
                                  pattern="[0-9A-Fa-f]{1,4}"
                                  required {}
                        }
                        label { "svID"
                            input type="text" name="sv_id"
                                  placeholder="SVDC_LAB_01"
                                  required {}
                        }
                        label { "Sample rate (Hz)"
                            input type="number" name="smp_rate"
                                  min="1" max="100000" value="4800"
                                  required {}
                        }
                        label { "Channels (V count)"
                            input type="number" name="v_count" min="0" max="4" value="4" {}
                        }
                        label { "Channels (I count)"
                            input type="number" name="i_count" min="0" max="4" value="4" {}
                        }
                    }
                    div.scd-form-row {
                        button.btn-primary type="submit" { "Register MU" }
                        button.btn-secondary
                            type="button"
                            id="clear-registry"
                            title="Drop all registered MUs" {
                            "Clear registry"
                        }
                    }
                }
            }
            section.config-section {
                div.config-section-head {
                    h2 { "Currently registered Merging Units" }
                }
                div id="registry-state" {
                    @if mus.is_empty() {
                        p.muted { "(none — load the sample, upload an SCD, or register manually above)" }
                    } @else {
                        (mu_table(&mus))
                    }
                }
            }
            section.placeholder {
                p.muted {
                    "About page + per-parameter editor lands under WBS-9.6b."
                }
            }
            script type="module" {
                (maud::PreEscaped(CONFIG_PAGE_JS))
            }
        },
    )
}

/// Vanilla-JS handlers for the buttons on /config.
const CONFIG_PAGE_JS: &str = r#"
const reload = () => window.location.reload();

document.getElementById('load-sample-scd')?.addEventListener('click', async () => {
  const r = await fetch('/api/config/scd/sample', { method: 'POST' });
  if (!r.ok) {
    const t = await r.text();
    alert('Sample load failed: ' + t);
    return;
  }
  reload();
});

document.getElementById('clear-registry')?.addEventListener('click', async () => {
  if (!confirm('Drop all registered MUs?')) return;
  const r = await fetch('/api/config/mus', { method: 'DELETE' });
  if (!r.ok) {
    alert('Clear failed: ' + r.status);
    return;
  }
  reload();
});

document.getElementById('manual-mu-form')?.addEventListener('submit', async (e) => {
  e.preventDefault();
  const f = e.currentTarget;
  const body = {
    id:       f.id.value.trim(),
    mac:      f.mac.value.trim(),
    appid:    f.appid.value.trim(),
    sv_id:    f.sv_id.value.trim(),
    smp_rate: parseInt(f.smp_rate.value, 10),
    v_count:  parseInt(f.v_count.value || '0', 10),
    i_count:  parseInt(f.i_count.value || '0', 10),
  };
  const r = await fetch('/api/config/mus', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!r.ok) {
    const t = await r.text();
    alert('Register failed: ' + t);
    return;
  }
  reload();
});
"#;

fn mu_table(mus: &[MergingUnit]) -> Markup {
    html! {
        table.layer-table {
            thead {
                tr {
                    th { "MU id" }
                    th { "MAC" }
                    th { "APPID" }
                    th { "svID" }
                    th { "smpRate" }
                    th { "Channels" }
                }
            }
            tbody {
                @for mu in mus {
                    tr {
                        td.mono { (mu.id) }
                        td.mono { (format_mac(mu.mac)) }
                        td.mono { (format!("0x{:04X}", mu.appid)) }
                        td.mono { (mu.sv_id) }
                        td.mono { (mu.smp_rate) " Hz" }
                        td { (mu.channels.len()) }
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

/// JSON response body returned from `POST /api/config/scd`.
#[derive(Debug, Serialize)]
pub struct ScdUploadResponse {
    /// Whether the upload was accepted and the registry replaced.
    pub ok: bool,
    /// Number of MUs registered (0 on failure).
    pub mu_count: usize,
    /// Human-readable status message.
    pub message: String,
}

async fn api_upload_scd(
    State(registry): State<SharedRegistry>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut xml_text: Option<String> = None;

    loop {
        let next = multipart.next_field().await;
        match next {
            Ok(Some(field)) => {
                let field_name = field.name().unwrap_or("").to_string();
                if field_name == "scd" {
                    match field.bytes().await {
                        Ok(bytes) => match String::from_utf8(bytes.to_vec()) {
                            Ok(s) => xml_text = Some(s),
                            Err(_) => {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(ScdUploadResponse {
                                        ok: false,
                                        mu_count: 0,
                                        message: "uploaded file is not valid UTF-8".into(),
                                    }),
                                )
                                    .into_response();
                            }
                        },
                        Err(e) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(ScdUploadResponse {
                                    ok: false,
                                    mu_count: 0,
                                    message: format!("failed to read upload bytes: {e}"),
                                }),
                            )
                                .into_response();
                        }
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ScdUploadResponse {
                        ok: false,
                        mu_count: 0,
                        message: format!("malformed multipart body: {e}"),
                    }),
                )
                    .into_response();
            }
        }
    }

    let Some(xml) = xml_text else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScdUploadResponse {
                ok: false,
                mu_count: 0,
                message: "no file part named 'scd' in upload".into(),
            }),
        )
            .into_response();
    };

    match scd::parse_scd(&xml) {
        Ok(doc) => {
            let n = registry.replace(doc.merging_units);
            tracing::info!(
                audit.event = "scd_upload",
                audit.mu_count = n,
                "operator SCD registered"
            );
            (
                StatusCode::OK,
                Json(ScdUploadResponse {
                    ok: true,
                    mu_count: n,
                    message: format!("SCD parsed; {n} Merging Unit(s) registered"),
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ScdUploadResponse {
                ok: false,
                mu_count: 0,
                message: format!("SCD validation failed: {e}"),
            }),
        )
            .into_response(),
    }
}

async fn api_channels(State(registry): State<SharedRegistry>) -> Json<Vec<MergingUnit>> {
    Json(registry.snapshot())
}

/// Body for `POST /api/config/mus` — manually register one MU.
#[derive(Debug, Deserialize)]
pub struct ManualMuRequest {
    /// IED-style id used everywhere in the system.
    pub id: String,
    /// MAC string (colons or hyphens accepted, 12 hex digits).
    pub mac: String,
    /// APPID, hex (e.g. "4000").
    pub appid: String,
    /// SV identifier the MU publishes in each ASDU.
    pub sv_id: String,
    /// Sample rate in Hz.
    pub smp_rate: u32,
    /// Number of voltage channels (0..=4).
    #[serde(default = "default_v_count")]
    pub v_count: u8,
    /// Number of current channels (0..=4).
    #[serde(default = "default_i_count")]
    pub i_count: u8,
}

fn default_v_count() -> u8 {
    4
}

fn default_i_count() -> u8 {
    4
}

async fn api_load_sample_scd(State(registry): State<SharedRegistry>) -> impl IntoResponse {
    match scd::parse_scd(SAMPLE_SCD_XML) {
        Ok(doc) => {
            let n = registry.replace(doc.merging_units);
            tracing::info!(
                audit.event = "scd_load_sample",
                audit.mu_count = n,
                "operator loaded built-in sample SCD"
            );
            (
                StatusCode::OK,
                Json(ScdUploadResponse {
                    ok: true,
                    mu_count: n,
                    message: format!("Sample SCD loaded; {n} Merging Unit(s) registered"),
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScdUploadResponse {
                ok: false,
                mu_count: 0,
                message: format!("built-in sample SCD failed to parse: {e}"),
            }),
        )
            .into_response(),
    }
}

async fn api_clear_registry(State(registry): State<SharedRegistry>) -> impl IntoResponse {
    let prev = registry.len();
    registry.replace(Vec::new());
    tracing::info!(
        audit.event = "registry_cleared",
        audit.previous_count = prev,
        "operator cleared the channel registry"
    );
    (
        StatusCode::OK,
        Json(ScdUploadResponse {
            ok: true,
            mu_count: 0,
            message: format!("Registry cleared ({prev} MU(s) removed)"),
        }),
    )
}

async fn api_register_mu(
    State(registry): State<SharedRegistry>,
    Json(req): Json<ManualMuRequest>,
) -> impl IntoResponse {
    let mu = match build_mu(req) {
        Ok(m) => m,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ScdUploadResponse {
                    ok: false,
                    mu_count: 0,
                    message: msg,
                }),
            )
                .into_response();
        }
    };

    // Append to existing registry rather than replacing — manual
    // registration is additive by design.
    let mut snap = registry.snapshot();
    if snap.iter().any(|m| m.id == mu.id) {
        return (
            StatusCode::CONFLICT,
            Json(ScdUploadResponse {
                ok: false,
                mu_count: snap.len(),
                message: format!("MU id '{}' already registered", mu.id),
            }),
        )
            .into_response();
    }
    let mu_id = mu.id.clone();
    snap.push(mu);
    let n = registry.replace(snap);
    tracing::info!(
        audit.event = "mu_manual_register",
        audit.mu_id = %mu_id,
        audit.total = n,
        "operator registered MU manually"
    );
    (
        StatusCode::OK,
        Json(ScdUploadResponse {
            ok: true,
            mu_count: n,
            message: format!("MU '{mu_id}' registered ({n} total)"),
        }),
    )
        .into_response()
}

fn build_mu(req: ManualMuRequest) -> Result<MergingUnit, String> {
    let mac = parse_mac_string(&req.mac).ok_or_else(|| format!("invalid MAC: {:?}", req.mac))?;
    let appid = u16::from_str_radix(req.appid.trim_start_matches("0x"), 16)
        .map_err(|_| format!("invalid APPID hex: {:?}", req.appid))?;
    if req.smp_rate == 0 {
        return Err("smp_rate must be > 0".to_string());
    }
    let v = req.v_count.min(4);
    let i = req.i_count.min(4);
    let mut channels = Vec::with_capacity((v + i) as usize);
    const PHASE_LABELS: [&str; 4] = ["phsA", "phsB", "phsC", "neut"];
    for label in PHASE_LABELS.iter().take(v as usize) {
        channels.push(Channel {
            name: format!("VPhMMXU1.PhV.{label}.MX"),
            unit: ChannelUnit::Voltage,
        });
    }
    for label in PHASE_LABELS.iter().take(i as usize) {
        channels.push(Channel {
            name: format!("IPhMMXU1.A.{label}.MX"),
            unit: ChannelUnit::Current,
        });
    }
    Ok(MergingUnit {
        id: req.id,
        mac,
        appid,
        sv_id: req.sv_id,
        smp_rate: req.smp_rate,
        channels,
    })
}

fn parse_mac_string(s: &str) -> Option<[u8; 6]> {
    let cleaned: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() != 12 {
        return None;
    }
    let mut out = [0u8; 6];
    for i in 0..6 {
        out[i] = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scd::{Channel, ChannelUnit};

    fn make_mu(id: &str) -> MergingUnit {
        MergingUnit {
            id: id.to_string(),
            mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01],
            appid: 0x4000,
            sv_id: "SVDC_DEMO".into(),
            smp_rate: 4800,
            channels: vec![Channel {
                name: "VPhMMXU1.PhV.phsA.MX".into(),
                unit: ChannelUnit::Voltage,
            }],
        }
    }

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn format_mac_pads_hex() {
        assert_eq!(
            format_mac([0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01]),
            "01:0C:CD:04:00:01"
        );
    }

    #[test]
    fn mu_table_renders_one_row_per_mu() {
        let html = mu_table(&[make_mu("MU-A"), make_mu("MU-B")]).into_string();
        assert!(html.contains("MU-A"));
        assert!(html.contains("MU-B"));
        assert!(html.contains("01:0C:CD:04:00:01"));
        assert!(html.contains("0x4000"));
    }

    #[test]
    fn config_index_shows_empty_state_when_no_mus() {
        let registry: SharedRegistry =
            std::sync::Arc::new(crate::scd::registry::ChannelRegistry::new());
        let body = tokio_test_block_on(config_index(State(registry)));
        let s = body.into_string();
        assert!(s.contains("(none"));
    }

    #[test]
    fn registry_global_is_a_singleton() {
        let a = registry_mod::global();
        let b = registry_mod::global();
        // Both handles point at the same allocation.
        assert!(std::sync::Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn parse_mac_accepts_colons_and_hyphens() {
        assert_eq!(
            parse_mac_string("01:0C:CD:04:00:01"),
            Some([0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01])
        );
        assert_eq!(
            parse_mac_string("01-0C-CD-04-00-01"),
            Some([0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01])
        );
        assert_eq!(
            parse_mac_string("010CCD040001"),
            Some([0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01])
        );
    }

    #[test]
    fn parse_mac_rejects_short_or_garbage() {
        assert_eq!(parse_mac_string(""), None);
        assert_eq!(parse_mac_string("01:0C:CD:04:00"), None);
        assert_eq!(parse_mac_string("not a mac at all"), None);
    }

    #[test]
    fn build_mu_synthesizes_v_and_i_channels() {
        let mu = build_mu(ManualMuRequest {
            id: "MU-LAB".into(),
            mac: "01-0C-CD-04-00-09".into(),
            appid: "4001".into(),
            sv_id: "SVDC_LAB".into(),
            smp_rate: 4800,
            v_count: 3,
            i_count: 4,
        })
        .unwrap();
        assert_eq!(mu.id, "MU-LAB");
        assert_eq!(mu.mac, [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x09]);
        assert_eq!(mu.appid, 0x4001);
        assert_eq!(mu.channels.len(), 7);
        let voltages = mu
            .channels
            .iter()
            .filter(|c| c.unit == ChannelUnit::Voltage)
            .count();
        let currents = mu
            .channels
            .iter()
            .filter(|c| c.unit == ChannelUnit::Current)
            .count();
        assert_eq!(voltages, 3);
        assert_eq!(currents, 4);
    }

    #[test]
    fn build_mu_rejects_bad_mac_and_bad_appid() {
        let req = ManualMuRequest {
            id: "X".into(),
            mac: "not-a-mac".into(),
            appid: "4000".into(),
            sv_id: "S".into(),
            smp_rate: 4800,
            v_count: 1,
            i_count: 1,
        };
        assert!(build_mu(req).is_err());

        let req2 = ManualMuRequest {
            id: "X".into(),
            mac: "01-02-03-04-05-06".into(),
            appid: "ZZZZ".into(),
            sv_id: "S".into(),
            smp_rate: 4800,
            v_count: 1,
            i_count: 1,
        };
        assert!(build_mu(req2).is_err());
    }

    #[test]
    fn build_mu_rejects_zero_sample_rate() {
        let req = ManualMuRequest {
            id: "X".into(),
            mac: "01-02-03-04-05-06".into(),
            appid: "4000".into(),
            sv_id: "S".into(),
            smp_rate: 0,
            v_count: 1,
            i_count: 1,
        };
        assert!(build_mu(req).is_err());
    }

    fn tokio_test_block_on<F: std::future::Future>(f: F) -> F::Output {
        // Tiny single-threaded runtime so the handler `async fn` can be
        // awaited inside synchronous test functions.
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }
}
