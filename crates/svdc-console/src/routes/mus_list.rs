//! `GET /south/mus` — Merging Units list.
//!
//! Industrial-grade table view of the registered MUs. Each row links
//! to the MU detail page (`/south/mus/:id`) so the operator can drill
//! into per-MU configuration and live waveform.
//!
//! Source of truth is the global `ChannelRegistry` populated from the
//! SCD upload on `/config`. MUs that have not been registered do not
//! appear; the empty state guides the operator back to the upload
//! flow.
//!
//! Per ADR-0001 the lane assignment originally put this file in the
//! Antigravity lane (WBS-9.3b). It is implemented here by Claude per
//! the user's direct request that MU rows be clickable into the
//! detail page. Antigravity's WBS-9.9 industrial-grid styling is
//! orthogonal and can layer on top.

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use maud::{html, Markup};

use crate::scd::registry::{self as registry_mod, SharedRegistry};
use crate::scd::MergingUnit;
use crate::templates::base::{layout, Section};

/// Build the MU-list sub-router using the process-wide registry.
pub fn router() -> Router {
    Router::new()
        .route("/south/mus", get(mus_list))
        .with_state(registry_mod::global())
}

async fn mus_list(State(registry): State<SharedRegistry>) -> Markup {
    let mus = registry.snapshot();
    let observed = crate::dataplane::global().seen_mus();
    layout(
        Section::Southbound,
        "Merging Units",
        mus_list_body(&mus, &observed),
    )
}

fn mus_list_body(mus: &[MergingUnit], observed: &[crate::dataplane::MuObservation]) -> Markup {
    html! {
        section.config-section {
            div.config-section-head {
                h2 { "Merging Units" }
                p.muted {
                    "Registered Merging Units come from the SCD on /config. "
                    "Auto-observed Merging Units are discovered live from the "
                    "ingress stream (synthetic demo loop or `--ingress-udp`)."
                }
            }
            @if mus.is_empty() {
                (empty_state())
            } @else {
                (mu_table(mus))
            }
        }
        section.config-section {
            div.config-section-head {
                h2 { "Auto-observed (live ingress)" }
                p.muted {
                    "Each row is a distinct svID the daemon has decoded from "
                    "incoming frames. PR D — auto-registered before the SCD "
                    "channel registry catches up."
                }
            }
            @if observed.is_empty() {
                (auto_empty_state())
            } @else {
                (observed_table(observed))
            }
        }
    }
}

fn auto_empty_state() -> Markup {
    html! {
        section.placeholder {
            h3 { "No live svIDs observed yet" }
            p.muted {
                "Start the synthetic demo from "
                a href="/dataplane" { "Data plane" }
                " or run the simulator against "
                code { "--ingress-udp" }
                " — see "
                a href="/" { "Dashboard" }
                " for the live-feed badge."
            }
        }
    }
}

fn observed_table(rows: &[crate::dataplane::MuObservation]) -> Markup {
    html! {
        table.layer-table.mu-table {
            thead {
                tr {
                    th.col-mu-id { "svID" }
                    th.col-state { "First seen (UTC)" }
                    th.col-state { "Last seen (UTC)" }
                    th.col-rate  { "Frame count" }
                }
            }
            tbody {
                @for o in rows {
                    tr {
                        td.col-mu-id { code { (o.sv_id) } }
                        td.col-state { (format_ms(o.first_seen_ms)) }
                        td.col-state { (format_ms(o.last_seen_ms)) }
                        td.col-rate  { (o.frame_count) }
                    }
                }
            }
        }
    }
}

fn format_ms(ms: u64) -> String {
    // ISO-like rendering without dragging chrono in. Matches the
    // emitter's formatter style for consistency.
    let secs = (ms / 1000) as i64;
    let millis = (ms % 1000) as u32;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    let s_of_day = secs.rem_euclid(86_400) as u32;
    let h = s_of_day / 3600;
    let mi = (s_of_day / 60) % 60;
    let s = s_of_day % 60;
    format!("{year:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}.{millis:03}")
}

fn empty_state() -> Markup {
    html! {
        section.placeholder {
            h3 { "No Merging Units registered" }
            p.muted {
                "Visit "
                a href="/config" { "Configuration" }
                " to upload an SCD, load the built-in sample, or register an MU manually."
            }
        }
    }
}

fn mu_table(mus: &[MergingUnit]) -> Markup {
    html! {
        table.layer-table.mu-table {
            thead {
                tr {
                    th.col-mu-id { "MU id" }
                    th.col-mac    { "Source MAC" }
                    th.col-appid  { "AppID" }
                    th.col-svid   { "svID" }
                    th.col-rate   { "Sample rate" }
                    th.col-chans  { "Channels" }
                    th.col-status { "Status" }
                }
            }
            tbody {
                @for mu in mus {
                    @let href = format!("/south/mus/{}", mu.id);
                    tr.mu-row data-mu-id=(mu.id) data-href=(href) {
                        td.col-mu-id {
                            a.mu-link href=(href) { (mu.id) }
                            div.muted.small { (channel_summary(mu)) }
                        }
                        td.mono { (format_mac(mu.mac)) }
                        td.mono { (format!("0x{:04X}", mu.appid)) }
                        td.mono { (mu.sv_id) }
                        td.mono { (mu.smp_rate) " Hz" }
                        td.mono { (mu.channels.len()) }
                        td {
                            // Phase 0/4: live status comes from the SSE
                            // waveform feed. Until per-MU streaming is
                            // wired, "Registered" is the most we can
                            // assert about the MU here.
                            span.state-badge.state-on { "Registered" }
                        }
                    }
                }
            }
        }
        script type="module" { (maud::PreEscaped(ROW_CLICK_JS)) }
    }
}

fn channel_summary(mu: &MergingUnit) -> String {
    use crate::scd::ChannelUnit;
    let v = mu
        .channels
        .iter()
        .filter(|c| c.unit == ChannelUnit::Voltage)
        .count();
    let i = mu
        .channels
        .iter()
        .filter(|c| c.unit == ChannelUnit::Current)
        .count();
    let other = mu.channels.len() - v - i;
    let mut parts: Vec<String> = Vec::new();
    if v > 0 {
        parts.push(format!("{v} V"));
    }
    if i > 0 {
        parts.push(format!("{i} I"));
    }
    if other > 0 {
        parts.push(format!("{other} other"));
    }
    parts.join(" · ")
}

fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

/// Vanilla-JS row click handler: clicking anywhere in the row (except
/// the existing link / interactive element) navigates to the MU
/// detail page. The link itself still works for keyboard / a11y.
const ROW_CLICK_JS: &str = r#"
document.querySelectorAll('tr.mu-row[data-href]').forEach((row) => {
  row.addEventListener('click', (e) => {
    if (e.target.closest('a, button, input, label')) return;
    const href = row.getAttribute('data-href');
    if (href) window.location.href = href;
  });
  row.setAttribute('role', 'link');
  row.setAttribute('tabindex', '0');
  row.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const href = row.getAttribute('data-href');
      if (href) window.location.href = href;
    }
  });
});
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scd::{Channel, ChannelUnit};

    fn make_mu(id: &str, n_v: usize, n_i: usize) -> MergingUnit {
        let mut channels = Vec::new();
        for i in 0..n_v {
            channels.push(Channel {
                name: format!("V{i}"),
                unit: ChannelUnit::Voltage,
            });
        }
        for i in 0..n_i {
            channels.push(Channel {
                name: format!("I{i}"),
                unit: ChannelUnit::Current,
            });
        }
        MergingUnit {
            id: id.to_string(),
            mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01],
            appid: 0x4000,
            sv_id: "SVDC_DEMO".to_string(),
            smp_rate: 4800,
            channels,
        }
    }

    #[test]
    fn router_constructs() {
        let _ = router();
    }

    #[test]
    fn empty_state_links_to_config() {
        let s = empty_state().into_string();
        assert!(s.contains("No Merging Units registered"));
        assert!(s.contains(r#"href="/config""#));
    }

    #[test]
    fn mu_table_renders_one_row_per_mu_with_detail_link() {
        let s = mu_table(&[make_mu("MU-01", 4, 4), make_mu("MU-02", 3, 3)]).into_string();
        assert!(s.contains("MU-01"));
        assert!(s.contains("MU-02"));
        assert!(s.contains(r#"href="/south/mus/MU-01""#));
        assert!(s.contains(r#"href="/south/mus/MU-02""#));
        assert!(s.contains(r#"data-href="/south/mus/MU-01""#));
        assert!(s.contains("mu-row"));
    }

    #[test]
    fn channel_summary_counts_v_and_i() {
        assert_eq!(channel_summary(&make_mu("X", 4, 4)), "4 V · 4 I");
        assert_eq!(channel_summary(&make_mu("X", 3, 0)), "3 V");
        assert_eq!(channel_summary(&make_mu("X", 0, 0)), "");
    }

    #[test]
    fn row_click_js_provides_keyboard_and_link_safety() {
        assert!(ROW_CLICK_JS.contains("a, button, input, label"));
        assert!(ROW_CLICK_JS.contains("tabindex"));
    }
}
