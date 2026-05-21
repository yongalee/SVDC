//! `GET /config` — Configuration screen.
//! `POST /api/config/scd` — SCD/SCL upload + validation.
//! `GET /api/config/channels` — JSON snapshot of the channel registry.
//!
//! WBS-9.6a (Claude) authors the validator, the registry, and the API.
//! WBS-9.6b (Antigravity) refines the upload form / About page on top.

use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{html, Markup};
use serde::Serialize;

use crate::scd::registry::{ChannelRegistry, SharedRegistry};
use crate::scd::{self, MergingUnit};
use crate::templates::base::{layout, Section};

/// Build the Configuration sub-router with shared registry state.
pub fn router() -> Router {
    let state: SharedRegistry = Arc::new(ChannelRegistry::new());
    Router::new()
        .route("/config", get(config_index))
        .route("/api/config/scd", post(api_upload_scd))
        .route("/api/config/channels", get(api_channels))
        .with_state(state)
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
                    }
                    p.muted.small {
                        "WBS-9.6b will replace this minimal form with an HTMX-driven "
                        "live-feedback variant; the parse/validate endpoint already "
                        "returns a structured JSON result usable today via curl."
                    }
                }
                section.scd-state {
                    h3 { "Currently registered Merging Units" }
                    @if mus.is_empty() {
                        p.muted { "(none — upload an SCD)" }
                    } @else {
                        (mu_table(&mus))
                    }
                }
                section.placeholder {
                    p.muted {
                        "About page + per-parameter editor lands under WBS-9.6b."
                    }
                }
            }
        },
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scd::registry::ChannelRegistry;
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
        let registry: SharedRegistry = Arc::new(ChannelRegistry::new());
        let body = tokio_test_block_on(config_index(State(registry)));
        let s = body.into_string();
        assert!(s.contains("(none"));
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
