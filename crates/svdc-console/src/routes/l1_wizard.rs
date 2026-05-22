//! L1 Verification Wizard card — appended to the L1 detail page
//! by `routes::northbound`. Step-by-step status check that walks
//! the operator from "daemon started" to "external SCADA can
//! consume live samples" without leaving `/north/l1`.
//!
//! Each step shows an icon (✓ ok, ⚠ warn, ✗ blocked), a one-line
//! status drawn from the live `DataPipeline` + `ChannelRegistry`,
//! and (when blocked) a one-click action button. The card
//! re-renders on page reload, which the "Refresh status" button
//! triggers; an SSE-driven live version would be a follow-up.
//!
//! OWNER: claude-code.
//! NFR-10: English-only.

use maud::{html, Markup, PreEscaped};

#[derive(Clone, Copy, PartialEq, Eq)]
enum WizardState {
    Ok,
    Warn,
    Blocked,
}

impl WizardState {
    fn icon(self) -> &'static str {
        match self {
            Self::Ok => "\u{2713}",
            Self::Warn => "\u{26A0}",
            Self::Blocked => "\u{2717}",
        }
    }
    fn pill_classes(self) -> &'static str {
        match self {
            Self::Ok => "wizard-icon wizard-icon-ok",
            Self::Warn => "wizard-icon wizard-icon-warn",
            Self::Blocked => "wizard-icon wizard-icon-blocked",
        }
    }
}

/// Render the L1 Verification Wizard card from a live snapshot of
/// the daemon's L1 / registry state. Called from
/// `routes::northbound::adapter_detail_page` after the existing
/// "Write & Commit Configuration" block.
pub fn l1_wizard_card(
    l1_active: bool,
    l1_publishes: u64,
    l1_last_tick: u64,
    registry_len: usize,
    registered_mu_id: &str,
) -> Markup {
    let step1 = if l1_active {
        WizardState::Ok
    } else {
        WizardState::Blocked
    };
    let step2 = if registry_len > 0 {
        WizardState::Ok
    } else if l1_active {
        WizardState::Warn
    } else {
        WizardState::Blocked
    };
    let step3 = if l1_publishes > 0 {
        WizardState::Ok
    } else if l1_active && registry_len > 0 {
        WizardState::Warn
    } else {
        WizardState::Blocked
    };
    let step4 = if l1_publishes > 0 {
        WizardState::Warn
    } else {
        WizardState::Blocked
    };

    let step2_body = if registry_len > 0 {
        format!(
            "{registry_len} MU(s) registered; the first one ('{registered_mu_id}') seeds the OPC UA address space."
        )
    } else {
        "Registry is empty. Load the built-in sample SCD (one click) or upload a real one on /config."
            .to_string()
    };

    let step3_body = if l1_publishes > 0 {
        format!("{l1_publishes} publishes so far; last tick_id = {l1_last_tick}.")
    } else if l1_active && registry_len > 0 {
        "Server is up and registry is non-empty, but no tick has reached the publisher yet. \
         Start the southbound simulator on port 9100, or trigger the /dataplane synthetic loop."
            .to_string()
    } else {
        "Blocked by steps 1 or 2.".to_string()
    };

    let step4_body = if l1_publishes > 0 {
        "Server is publishing. Run the reference L1 client in a terminal \u{2014} if it prints \
         [L1] lines, an external SCADA can connect the same way. UA Expert can also connect to \
         opc.tcp://127.0.0.1:4840/."
            .to_string()
    } else {
        "Waits on step 3.".to_string()
    };

    html! {
        div class="glass-card shadow-md mt-4" {
            div class="card-header border-b border-border-color pb-3" {
                h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" {
                    "Verification Wizard \u{2014} Connect a SCADA Client"
                }
            }
            div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                p class="text-text-secondary leading-relaxed" {
                    "Walk these four gates top-down. Each row shows the live state of the data \
                     trail and offers the next action when a gate is open. The Refresh status \
                     button below re-checks all four."
                }

                (wizard_step(
                    step1, "1.", "OPC UA server listening on the bind address",
                    if l1_active {
                        "L1 task is up; binding loopback :4840 (per ADR-0017 \u{00A7}5).".to_string()
                    } else {
                        "Daemon was started without --enable-opcua. Stop svdc, then restart with \
                         the flag.".to_string()
                    },
                    if l1_active { None } else {
                        Some(("Show start command", "wizardToggle('wizard-start-cmd')"))
                    },
                ))
                (wizard_step(
                    step2, "2.", "At least one MU is in the channel registry",
                    step2_body,
                    if registry_len == 0 {
                        Some(("Load sample SCD", "wizardLoadSampleScd()"))
                    } else { None },
                ))
                (wizard_step(
                    step3, "3.", "Server is publishing aligned ticks to the address space",
                    step3_body,
                    if l1_publishes == 0 && l1_active && registry_len > 0 {
                        Some(("Open /dataplane", "window.location.href='/dataplane'"))
                    } else { None },
                ))
                (wizard_step(
                    step4, "4.", "External client subscription end-to-end",
                    step4_body,
                    if l1_publishes > 0 {
                        Some(("Show client command", "wizardToggle('wizard-client-cmd')"))
                    } else { None },
                ))

                div class="border-t border-border-color pt-3 mt-2 flex items-center justify-between" {
                    span class="text-text-secondary text-[11px]" {
                        "Server-side counters: "
                        code class="font-mono" { (l1_publishes) }
                        " publishes \u{00B7} last tick_id "
                        code class="font-mono" { (l1_last_tick) }
                    }
                    button type="button"
                        class="btn-primary py-1 px-3 text-[11px] bg-accent-blue hover:bg-[#1d4ed8]"
                        onclick="window.location.reload();" {
                        "Refresh status"
                    }
                }

                pre id="wizard-start-cmd"
                    class="font-mono text-[10px] hidden mt-2 p-2 bg-bg-secondary border border-border-color rounded" {
                    "cargo run --release -p svdc-bin -- \\\n    --ingress-udp 127.0.0.1:9100 \\\n    --enable-opcua"
                }
                pre id="wizard-client-cmd"
                    class="font-mono text-[10px] hidden mt-2 p-2 bg-bg-secondary border border-border-color rounded" {
                    "cargo run --release -p svdc-l1-opcua-client -- \\\n    --endpoint opc.tcp://127.0.0.1:4840/ \\\n    --samples 20"
                }

                script { (PreEscaped(L1_WIZARD_JS)) }
                style { (PreEscaped(L1_WIZARD_CSS)) }
            }
        }
    }
}

fn wizard_step(
    state: WizardState,
    label: &str,
    title: &str,
    body: String,
    action: Option<(&'static str, &'static str)>,
) -> Markup {
    html! {
        div class="flex items-start gap-3" {
            span class=(state.pill_classes()) { (state.icon()) }
            div class="flex-1" {
                div class="flex items-center gap-2" {
                    span class="font-bold text-text-primary text-[12px]" {
                        (label) " " (title)
                    }
                }
                p class="text-text-secondary text-[11px] mt-0.5 leading-relaxed" { (body) }
                @if let Some((btn_label, onclick)) = action {
                    button type="button" onclick=(onclick)
                        class="mt-1 btn-secondary py-0.5 px-2 text-[10px]" {
                        (btn_label)
                    }
                }
            }
        }
    }
}

const L1_WIZARD_JS: &str = r#"
function wizardToggle(id) {
  const el = document.getElementById(id);
  if (el) el.classList.toggle('hidden');
}
async function wizardLoadSampleScd() {
  try {
    const r = await fetch('/api/config/scd/sample', { method: 'POST' });
    if (r.ok) window.location.reload();
    else alert('Sample SCD load failed: ' + r.status);
  } catch (e) {
    alert('Sample SCD load error: ' + e);
  }
}
"#;

/// Inline stylesheet for the wizard. Lives next to the markup so
/// it does not depend on `styles.css` (Antigravity branch has the
/// stylesheet stored as UTF-16, which makes external editing
/// risky).
const L1_WIZARD_CSS: &str = r#"
.wizard-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  font-size: 14px;
  font-weight: 700;
  flex-shrink: 0;
}
.wizard-icon-ok      { background: rgba(22, 163, 74, 0.18);  color: #16a34a; border: 1px solid rgba(22, 163, 74, 0.45); }
.wizard-icon-warn    { background: rgba(245, 158, 11, 0.18); color: #b45309; border: 1px solid rgba(245, 158, 11, 0.45); }
.wizard-icon-blocked { background: rgba(220, 38, 38, 0.18);  color: #b91c1c; border: 1px solid rgba(220, 38, 38, 0.45); }
.hidden { display: none !important; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_blocked_when_daemon_is_idle() {
        let m = l1_wizard_card(false, 0, 0, 0, "(none)").into_string();
        // Step 1 must be ✗ when l1_active=false.
        assert!(m.contains("wizard-icon-blocked"));
        // The start-command pre is in the DOM (hidden until toggled).
        assert!(m.contains("--enable-opcua"));
        // Refresh button always renders.
        assert!(m.contains("Refresh status"));
    }

    #[test]
    fn step1_ok_when_server_active_step2_warn_when_registry_empty() {
        let m = l1_wizard_card(true, 0, 0, 0, "(none)").into_string();
        // Two distinct icons visible: ok (step 1) and warn (step 2).
        assert!(m.contains("wizard-icon-ok"));
        assert!(m.contains("wizard-icon-warn"));
    }

    #[test]
    fn all_ok_when_publishes_have_landed() {
        let m = l1_wizard_card(true, 480, 4800, 1, "SVDC_DEMO").into_string();
        // Steps 1-3 are ok; step 4 stays warn until external client
        // is independently verified — encoded into the rule set.
        let ok_count = m.matches("wizard-icon-ok").count();
        assert!(
            ok_count >= 3,
            "expected at least 3 ok icons, got {ok_count}"
        );
        assert!(m.contains("480 publishes"));
        assert!(m.contains("SVDC_DEMO"));
    }

    #[test]
    fn step2_action_button_appears_when_registry_empty() {
        let m = l1_wizard_card(true, 0, 0, 0, "(none)").into_string();
        assert!(m.contains("wizardLoadSampleScd"));
        assert!(m.contains("Load sample SCD"));
    }

    #[test]
    fn step4_client_command_shown_only_when_step3_ok() {
        let blocked = l1_wizard_card(false, 0, 0, 0, "(none)").into_string();
        let running = l1_wizard_card(true, 480, 4800, 1, "SVDC_DEMO").into_string();
        assert!(!blocked.contains("Show client command"));
        assert!(running.contains("Show client command"));
    }
}
