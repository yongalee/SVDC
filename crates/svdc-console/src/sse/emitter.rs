/* SVDC Console SSE Emitter
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use crate::sse::{DashboardMetrics, SsePayload, WaveformSample};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use svdc_subscribe::{ChannelSet, Subscriber};
use tokio::sync::broadcast;
use tokio::time;

static EMITTER_TX: OnceLock<broadcast::Sender<String>> = OnceLock::new();

/// Retrieve the global broadcast transmitter, initializing it on first use
pub fn get_emitter_tx() -> &'static broadcast::Sender<String> {
    EMITTER_TX.get_or_init(|| {
        let (tx, _) = broadcast::channel(1024);

        // Spawn background simulation loop
        tokio::spawn(run_simulation(tx.clone()));

        tx
    })
}

/// Subscribe to the global real-time event broadcast stream
pub fn subscribe() -> broadcast::Receiver<String> {
    get_emitter_tx().subscribe()
}

/// Broadcast a telemetry event to all active console subscribers
pub fn broadcast_event(payload: &SsePayload) {
    if let Ok(json_str) = serde_json::to_string(payload) {
        let _ = get_emitter_tx().send(json_str);
    }
}

async fn run_simulation(tx: broadcast::Sender<String>) {
    let mut interval_10hz = time::interval(Duration::from_millis(100));
    let mut last_metrics_time = Instant::now();
    let mut angle: f32 = 0.0;

    // Simulated constants
    let v_peak = 110.0 * 1.414; // Peak voltage (110V RMS)
    let i_peak = 5.0 * 1.414; // Peak current (5A RMS)
    let pi_2_3 = 2.0 * std::f32::consts::PI / 3.0; // 120 degrees in radians

    let mut last_qse_time = Instant::now();
    let qse_operations = [
        (
            "WBS-9.3c",
            "Phase A Transient Correction",
            "QSE Estimator Core",
            "Substation QSE",
            "HEALED",
            "text-accent-green",
        ),
        (
            "WBS-9.3a",
            "Out-of-window Frame Rejected",
            "Circular Buffer",
            "svdc-ingest",
            "DROPPED",
            "text-accent-yellow",
        ),
        (
            "WBS-9.3c",
            "Residual Variance Warning",
            "Diagnostic Core",
            "Substation QSE",
            "WARN",
            "text-accent-yellow",
        ),
        (
            "WBS-9.1b",
            "Lock-Free Synchronization Adjust",
            "Time Aligner",
            "PTP Daemon",
            "SYNCED",
            "text-accent-blue",
        ),
        (
            "Gate G0",
            "Spec-Lock Integrity Verification",
            "SSIEC Node Settings",
            "claude-code",
            "LOCKED",
            "text-accent-green",
        ),
    ];
    let mut qse_index = 0;

    let pipeline = crate::dataplane::global();
    let mut sub = pipeline.subscriber.subscribe(ChannelSet::all());

    loop {
        interval_10hz.tick().await;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut has_real_data = false;
        if pipeline.has_external_feed() {
            let records = sub.read_since();
            if !records.is_empty() {
                has_real_data = true;
                // Throttle SSE: take up to 20 samples per 100ms
                let step = std::cmp::max(1, records.len() / 20);
                for r in records.iter().step_by(step) {
                    let wave_event = SsePayload::Waveform(WaveformSample {
                        mu_id: "MU-01".to_string(),
                        timestamp_ms: r.ts_utc_ns / 1_000_000,
                        i1: (r.samples[0].value_q as f32) / 1000.0,
                        i2: (r.samples[1].value_q as f32) / 1000.0,
                        i3: (r.samples[2].value_q as f32) / 1000.0,
                        i0: (r.samples[3].value_q as f32) / 1000.0,
                        v1: (r.samples[4].value_q as f32) / 100.0,
                        v2: (r.samples[5].value_q as f32) / 100.0,
                        v3: (r.samples[6].value_q as f32) / 100.0,
                        v0: (r.samples[7].value_q as f32) / 100.0,
                    });
                    if let Ok(json_str) = serde_json::to_string(&wave_event) {
                        let _ = tx.send(json_str);
                    }
                }
            }
        }

        if !has_real_data {
            // 1. Simulate 3-phase AC voltage and current waveforms at 10 Hz (WBS-9.3b)
            angle += 0.2; // increment angle for sine generation
            if angle > 2.0 * std::f32::consts::PI {
                angle -= 2.0 * std::f32::consts::PI;
            }

            let v1 = v_peak * angle.sin();
            let v2 = v_peak * (angle - pi_2_3).sin();
            let v3 = v_peak * (angle + pi_2_3).sin();
            let v0 = v1 + v2 + v3; // Neutral voltage should sum to nearly 0 in balanced state

            let i1 = i_peak * (angle - 0.1).sin(); // 0.1 rad lag for inductive load power factor
            let i2 = i_peak * (angle - pi_2_3 - 0.1).sin();
            let i3 = i_peak * (angle + pi_2_3 - 0.1).sin();
            let i0 = i1 + i2 + i3;

            let snapshot = crate::scd::registry::global().snapshot();
            let mut mu_ids: Vec<String> = snapshot.iter().map(|m| m.id.clone()).collect();
            if !mu_ids.contains(&"MU-SIM".to_string()) {
                mu_ids.push("MU-SIM".to_string());
            }

            for mu_id in mu_ids {
                let is_connected = mu_id == "MU-SIM"
                    || crate::routes::mu_detail::connected_mus()
                        .read()
                        .unwrap()
                        .contains(&mu_id);
                if is_connected {
                    let wave_event = SsePayload::Waveform(WaveformSample {
                        mu_id: mu_id.clone(),
                        timestamp_ms: now_ms,
                        v1,
                        v2,
                        v3,
                        v0,
                        i1,
                        i2,
                        i3,
                        i0,
                    });
                    if let Ok(json_str) = serde_json::to_string(&wave_event) {
                        let _ = tx.send(json_str);
                    }
                }
            }
        }

        // 2. Simulate dashboard telemetry updates at 1 Hz (WBS-9.2b)
        if last_metrics_time.elapsed() >= Duration::from_secs(1) {
            last_metrics_time = Instant::now();

            // Slightly fluctuate simulated metrics to look active
            let ptp_offset = 12 + (now_ms % 7) as i64; // varies between 12 and 18 ns
            let buffer_sat = 2.4 + ((now_ms % 5) as f64) * 0.1; // 2.4% - 2.8% saturation

            let metrics_event = SsePayload::Metrics(DashboardMetrics {
                ptp_sync_status: "Locked".to_string(),
                ptp_offset_ns: ptp_offset,
                buffer_saturation: buffer_sat,
                active_mus: 5,
                sps_rate: 20800,
                l1_opcua_active: true,
                l2_mqtt_active: false,
                l3_timescaledb_active: true,
            });

            if let Ok(json_str) = serde_json::to_string(&metrics_event) {
                let _ = tx.send(json_str);
            }

            // MuMetrics dummy data
            let mut jitter_histograms = Vec::new();
            for m_idx in 0..7 {
                let mut jitter = vec![0; 10];
                for i in 0..10 {
                    jitter[i] = ((now_ms / 100 + i as u64 * (m_idx + 1) * 3) % 40) as u32;
                }
                jitter_histograms.push(jitter);
            }

            let mu_metrics = vec![
                crate::sse::MuTelemetry {
                    mu_id: "MU-01".to_string(),
                    observed_sps: 4000,
                    missing_samples: 0,
                    interpolation_count: 0,
                    qse_corrections: 0,
                    jitter_histogram: jitter_histograms[0].clone(),
                    ptp_sync: format!("Locked ({} ns)", ptp_offset),
                    calibration: (1.0001, -0.02, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-02".to_string(),
                    observed_sps: 4000,
                    missing_samples: (now_ms % 100000 / 8000) as u32,
                    interpolation_count: (now_ms % 100000 / 6000) as u32,
                    qse_corrections: (now_ms % 100000 / 12000) as u32,
                    jitter_histogram: jitter_histograms[1].clone(),
                    ptp_sync: "Locked (15 ns)".to_string(),
                    calibration: (0.9998, 0.05, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-03".to_string(),
                    observed_sps: 0,
                    missing_samples: 8563 + (now_ms % 100 / 10) as u32,
                    interpolation_count: 0,
                    qse_corrections: 0,
                    jitter_histogram: vec![0; 10],
                    ptp_sync: "Disconnected".to_string(),
                    calibration: (1.0, 0.0, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-04".to_string(),
                    observed_sps: 4000,
                    missing_samples: 12,
                    interpolation_count: 2,
                    qse_corrections: 0,
                    jitter_histogram: jitter_histograms[3].clone(),
                    ptp_sync: "Locked (9 ns)".to_string(),
                    calibration: (1.0, 0.0, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-05".to_string(),
                    observed_sps: 4800,
                    missing_samples: 0,
                    interpolation_count: 0,
                    qse_corrections: 0,
                    jitter_histogram: jitter_histograms[4].clone(),
                    ptp_sync: "Locked (2 ns)".to_string(),
                    calibration: (1.0, 0.0, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-06".to_string(),
                    observed_sps: 4000,
                    missing_samples: 0,
                    interpolation_count: 0,
                    qse_corrections: 0,
                    jitter_histogram: jitter_histograms[5].clone(),
                    ptp_sync: "Locked (4 ns)".to_string(),
                    calibration: (1.0, 0.0, 1.0),
                },
                crate::sse::MuTelemetry {
                    mu_id: "MU-SIM".to_string(),
                    observed_sps: 4000,
                    missing_samples: 0,
                    interpolation_count: 0,
                    qse_corrections: 0,
                    jitter_histogram: jitter_histograms[6].clone(),
                    ptp_sync: "Locked (1 ns)".to_string(),
                    calibration: (1.0, 0.0, 1.0),
                },
            ];
            let mu_event = SsePayload::MuMetrics(mu_metrics);
            if let Ok(json_str) = serde_json::to_string(&mu_event) {
                let _ = tx.send(json_str);
            }
        }

        // 3. Simulate random QSE Audit Logs every few seconds
        if last_qse_time.elapsed() >= Duration::from_secs(3 + (now_ms % 4)) {
            last_qse_time = Instant::now();
            let op = qse_operations[qse_index % qse_operations.len()];
            qse_index += 1;

            // Format current time without chrono
            let secs = now_ms / 1000;
            let datetime = format!(
                "2026-05-22 {:02}:{:02}:{:02}",
                (secs / 3600) % 24,
                (secs / 60) % 60,
                secs % 60
            );

            let qse_event = SsePayload::Qse(crate::sse::QseLog {
                timestamp: datetime,
                wbs: op.0.to_string(),
                operation: op.1.to_string(),
                target: op.2.to_string(),
                operator: op.3.to_string(),
                result: op.4.to_string(),
                result_color: op.5.to_string(),
            });

            if let Ok(json_str) = serde_json::to_string(&qse_event) {
                let _ = tx.send(json_str);
            }
        }
    }
}

// Custom type-alias for Instant to ensure standard library compilation
type Instant = std::time::Instant;
