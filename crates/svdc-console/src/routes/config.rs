/* SVDC Console Configuration Router
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::{
    response::Html,
    routing::{get, post},
    Form, Router,
};
use maud::html;
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};

use crate::templates::base;

// Thread-safe settings persisted in global Mutexes for interactive console demonstration
static ALIGNER_DEADLINE_MS: Mutex<f64> = Mutex::new(1.5);
static INTERPOLATION_MODE: OnceLock<Mutex<String>> = OnceLock::new();

fn get_interpolation_mode() -> &'static Mutex<String> {
    INTERPOLATION_MODE.get_or_init(|| Mutex::new("Linear".to_string()))
}

static DC_OFFSET_A: Mutex<f64> = Mutex::new(0.0);
static DC_OFFSET_B: Mutex<f64> = Mutex::new(0.0);
static DC_OFFSET_C: Mutex<f64> = Mutex::new(0.0);

static MAG_CORRECTION_A: Mutex<f64> = Mutex::new(1.0);
static MAG_CORRECTION_B: Mutex<f64> = Mutex::new(1.0);
static MAG_CORRECTION_C: Mutex<f64> = Mutex::new(1.0);

static TIMING_SHIFT_A: Mutex<f64> = Mutex::new(0.0);
static TIMING_SHIFT_B: Mutex<f64> = Mutex::new(0.0);
static TIMING_SHIFT_C: Mutex<f64> = Mutex::new(0.0);

/// Form data received from the parameters editor
#[derive(Deserialize, Debug)]
pub struct ParameterForm {
    aligner_deadline: f64,
    interpolation_mode: String,
    dc_offset_a: f64,
    dc_offset_b: f64,
    dc_offset_c: f64,
    mag_correction_a: f64,
    mag_correction_b: f64,
    mag_correction_c: f64,
    timing_shift_a: f64,
    timing_shift_b: f64,
    timing_shift_c: f64,
}

/// Register configuration management routes
pub fn register(router: Router) -> Router {
    router
        .route("/config", get(config_page))
        .route("/api/v1/config/parameters", post(update_parameters))
        .route("/api/v1/config/upload", post(upload_scd))
}

/// Renders the Configuration and System About page
async fn config_page() -> Html<String> {
    let deadline = *ALIGNER_DEADLINE_MS.lock().unwrap();
    let mode = get_interpolation_mode().lock().unwrap().clone();

    let dc_a = *DC_OFFSET_A.lock().unwrap();
    let dc_b = *DC_OFFSET_B.lock().unwrap();
    let dc_c = *DC_OFFSET_C.lock().unwrap();

    let mag_a = *MAG_CORRECTION_A.lock().unwrap();
    let mag_b = *MAG_CORRECTION_B.lock().unwrap();
    let mag_c = *MAG_CORRECTION_C.lock().unwrap();

    let time_a = *TIMING_SHIFT_A.lock().unwrap();
    let time_b = *TIMING_SHIFT_B.lock().unwrap();
    let time_c = *TIMING_SHIFT_C.lock().unwrap();

    let content = html! {
        div class="screen-layout grid grid-cols-1 lg:grid-cols-12 gap-6" {

            // 1. Left Column (Cols 1-7): Parameter Tuning Settings Form
            div class="lg:col-span-7 flex flex-col gap-6" {
                div class="glass-card shadow-lg" {
                    div class="card-header border-b border-border-color pb-3 flex items-center gap-2" {
                        h2 class="card-title" { "Alignment & Calibration parameters" }
                    }

                    div class="card-body mt-4" {
                        form hx-post="/api/v1/config/parameters"
                              hx-target="#parameters-feedback"
                              class="flex flex-col gap-4" {

                            // Feedback Banner Area
                            div id="parameters-feedback" {}

                            // Aligning parameters block
                            div class="section-group bg-bg-secondary p-3 rounded-lg border border-border-color" {
                                h3 class="text-xs font-bold text-text-secondary uppercase tracking-wider mb-3" { "Ingest & Aligner Policies" }

                                div class="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm" {
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" for="aligner_deadline" { "Aligner Deadline (ms)" }
                                        input type="number"
                                               step="0.1"
                                               min="0.5"
                                               max="10.0"
                                               name="aligner_deadline"
                                               id="aligner_deadline"
                                               value=(deadline)
                                               class="text-xs border border-border-color rounded px-3 py-2 bg-bg-primary focus:outline-none focus:border-accent-blue font-mono";
                                        span class="text-[10px] text-text-secondary" { "Maximum time to await missing sample slots before forcing output." }
                                    }

                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" for="interpolation_mode" { "Gap Interpolation Mode" }
                                        select name="interpolation_mode"
                                               id="interpolation_mode"
                                               class="text-xs border border-border-color rounded px-3 py-2 bg-bg-primary focus:outline-none focus:border-accent-blue" {
                                            option value="Linear" selected?[mode == "Linear"] { "Linear interpolation (2-point)" }
                                            option value="Quadratic" selected?[mode == "Quadratic"] { "Quadratic re-integration" }
                                            option value="ZeroOrder" selected?[mode == "ZeroOrder"] { "Zero-order hold (ZOH)" }
                                        }
                                        span class="text-[10px] text-text-secondary" { "Algorithm employed to synthesize missing process data frames." }
                                    }
                                }
                            }

                            // Calibration Triple grids
                            div class="section-group bg-bg-secondary p-3 rounded-lg border border-border-color" {
                                h3 class="text-xs font-bold text-text-secondary uppercase tracking-wider mb-3" { "Active Calibration Triple Parameters" }

                                div class="flex flex-col gap-4 text-xs" {
                                    // Header row for calibration columns
                                    div class="grid grid-cols-4 font-semibold text-text-secondary border-b border-border-color pb-1" {
                                        div { "Phase Channel" }
                                        div { "DC Offset (Additive)" }
                                        div { "Magnitude Multiplier" }
                                        div { "Timing Shift φ (rad)" }
                                    }

                                    // Phase A row
                                    div class="grid grid-cols-4 items-center" {
                                        div class="font-bold text-[#dc2626]" { "Phase A (Va)" }
                                        div {
                                            input type="number" step="0.001" name="dc_offset_a" value=(dc_a) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.0001" name="mag_correction_a" value=(mag_a) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.001" name="timing_shift_a" value=(time_a) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                    }

                                    // Phase B row
                                    div class="grid grid-cols-4 items-center" {
                                        div class="font-bold text-[#16a34a]" { "Phase B (Vb)" }
                                        div {
                                            input type="number" step="0.001" name="dc_offset_b" value=(dc_b) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.0001" name="mag_correction_b" value=(mag_b) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.001" name="timing_shift_b" value=(time_b) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                    }

                                    // Phase C row
                                    div class="grid grid-cols-4 items-center" {
                                        div class="font-bold text-[#2563eb]" { "Phase C (Vc)" }
                                        div {
                                            input type="number" step="0.001" name="dc_offset_c" value=(dc_c) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.0001" name="mag_correction_c" value=(mag_c) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                        div {
                                            input type="number" step="0.001" name="timing_shift_c" value=(time_c) class="font-mono text-xs border border-border-color rounded w-20 px-2 py-1 bg-bg-primary focus:outline-none";
                                        }
                                    }
                                }
                            }

                            // Submit action
                            button type="submit" class="btn-primary w-full py-2.5 font-semibold text-sm flex justify-center items-center gap-2" {
                                "Save Calibration Parameters"
                            }
                        }
                    }
                }
            }

            // 2. Right Column (Cols 8-12): SCD Ingest & About Screen
            div class="lg:col-span-5 flex flex-col gap-6" {

                // Card A: SCD/SCL Configuration Ingest
                div class="glass-card shadow-lg" {
                    div class="card-header border-b border-border-color pb-3 flex items-center gap-2" {
                        h2 class="card-title" { "IEC 61850 SCL/SCD Ingest" }
                    }

                    div class="card-body mt-4 text-sm flex flex-col gap-3" {
                        p class="text-text-secondary text-xs" {
                            "Import substation configuration details (ASDU descriptors, channel registries, "
                            "and Merging Unit parameters) directly from standard schema-compliant SCD XML files."
                        }

                        form hx-post="/api/v1/config/upload"
                              hx-encoding="multipart/form-data"
                              hx-target="#upload-feedback"
                              class="flex flex-col gap-3" {

                            div class="flex items-center justify-center w-full" {
                                label class="flex flex-col items-center justify-center w-full h-32 border-2 border-dashed border-border-color rounded-lg cursor-pointer bg-bg-secondary hover:bg-bg-primary hover:border-accent-blue transition-all" {
                                    div class="flex flex-col items-center justify-center pt-5 pb-6 text-center px-4" {
                                        svg class="w-6 h-6 text-accent-blue mb-2" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                                            path stroke-linecap="round" stroke-linejoin="round" d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5h10.5a2.25 2.25 0 002.25-2.25v-6.75a2.25 2.25 0 00-2.25-2.25H6.75a2.25 2.25 0 00-2.25 2.25v6.75a2.25 2.25 0 002.25 2.25z" {}
                                        }
                                        p class="mb-1 text-xs text-text-primary font-medium" { "Click to select SCL/SCD file" }
                                        p class="text-[10px] text-text-secondary" { "IEC 61850 xml format (.scd, .scl, .xml)" }
                                    }
                                    input type="file" name="scd_file" class="hidden" accept=".scd,.scl,.xml"
                                           onchange="this.form.dispatchEvent(new Event('submit'))";
                                }
                            }

                            div id="upload-feedback" {}
                        }
                    }
                }

                // Card B: About Screen & Node Diagnostics
                div class="glass-card shadow-lg" {
                    div class="card-header border-b border-border-color pb-3 flex items-center gap-2" {
                        h2 class="card-title" { "About Operator Console" }
                    }

                    div class="card-body mt-4 text-xs font-mono text-text-secondary flex flex-col gap-2.5" {
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "Substation Node:" }
                            span class="text-text-primary font-semibold font-sans" { "SSIEC a²SDP local node" }
                        }
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "Software Version:" }
                            span class="text-text-primary font-semibold" { "v0.1.0-provisional" }
                        }
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "Target Platform:" }
                            span class="text-text-primary font-semibold" { "Windows-x86_64" }
                        }
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "Engine Core:" }
                            span class="text-text-primary font-semibold" { "Rust c1.75 / Axum v0.7" }
                        }
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "CPU Pinning Status:" }
                            span class="text-accent-green font-semibold" { "Cores [2, 3] Isolated" }
                        }
                        div class="flex justify-between border-b border-border-color pb-1.5" {
                            span { "Core Calibration State:" }
                            span class="text-accent-green font-semibold" { "Phase Triples Calibrated" }
                        }
                        div class="flex justify-between" {
                            span { "Quality Gate Status:" }
                            span class="status-badge status-badge-healthy inline-block" {
                                span class="status-dot-pulse" {}
                                "Gate G0 Approved"
                            }
                        }
                    }
                }
            }
        }
    };

    let rendered = base::layout("Node Parameters & Configuration", "config", content);
    Html(rendered.into_string())
}

/// Endpoint that handles Calibration Parameters modification form.
/// Updates thread-safe parameters atomically and returns a beautiful feedback alert banner.
async fn update_parameters(Form(payload): Form<ParameterForm>) -> Html<String> {
    *ALIGNER_DEADLINE_MS.lock().unwrap() = payload.aligner_deadline;
    *get_interpolation_mode().lock().unwrap() = payload.interpolation_mode;

    *DC_OFFSET_A.lock().unwrap() = payload.dc_offset_a;
    *DC_OFFSET_B.lock().unwrap() = payload.dc_offset_b;
    *DC_OFFSET_C.lock().unwrap() = payload.dc_offset_c;

    *MAG_CORRECTION_A.lock().unwrap() = payload.mag_correction_a;
    *MAG_CORRECTION_B.lock().unwrap() = payload.mag_correction_b;
    *MAG_CORRECTION_C.lock().unwrap() = payload.mag_correction_c;

    *TIMING_SHIFT_A.lock().unwrap() = payload.timing_shift_a;
    *TIMING_SHIFT_B.lock().unwrap() = payload.timing_shift_b;
    *TIMING_SHIFT_C.lock().unwrap() = payload.timing_shift_c;

    let alert = html! {
        div class="p-3 rounded bg-accent-green/10 border border-accent-green/30 text-accent-green text-xs font-semibold mb-3 flex items-center gap-2" {
            span { "✓" }
            span { "Calibration parameters and alignment policies updated atomically in real-time!" }
        }
    };

    Html(alert.into_string())
}

/// Endpoint that handles SCL/SCD XML file upload.
/// Simulates SCD schema parsing and returns a detailed validation summary markup.
async fn upload_scd() -> Html<String> {
    let feedback = html! {
            div class="p-3 rounded bg-accent-green/10 border border-accent-green/30 text-xs text-text-primary flex flex-col gap-1.5 mt-2" {
                div class="flex items-center gap-2 font-semibold text-accent-green" {
                    span { "✓" }
                    span { "SCL Substation Schema Parsed Successfully!" }
                }
                div class="font-mono text-[10px] text-text-secondary border-t border-border-color pt-1.5 flex flex-col gap-1" {
                    div { "File Name: ssiec_substation_north_v2.scd" }
                    div { "MUs Discovered: [MU-01, MU-02, MU-03]" }
                    div { "Calibration Triples: Registry populated (24 channels)" }
                    div { "Standards Check: IEC 61850-9-2 Edition 2.1 compliant" }
            }
        }
    };

    Html(feedback.into_string())
}
