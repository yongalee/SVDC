/* SVDC Console SSE Emitter
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use crate::sse::{DashboardMetrics, SsePayload, WaveformSample};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    let mut last_ticks_emitted: u64 = 0;
    let mut angle: f32 = 0.0;

    // Simulated constants
    let v_peak = 110.0 * 1.414; // Peak voltage (110V RMS)
    let i_peak = 5.0 * 1.414; // Peak current (5A RMS)
    let pi_2_3 = 2.0 * std::f32::consts::PI / 3.0; // 120 degrees in radians

    loop {
        interval_10hz.tick().await;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

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

        let wave_event = SsePayload::Waveform(WaveformSample {
            mu_id: "MU-01".to_string(),
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

        // 2. Dashboard telemetry update — once per second.
        //    Live counters come from the shared `DataPipeline`
        //    (PR #51) which is fed by either the in-process
        //    synthetic loop or the daemon's `--ingress-udp` task
        //    (PR #54). When neither is producing, the buffer is
        //    empty and all counters read zero — the dashboard
        //    shows that honestly instead of pretending.
        if last_metrics_time.elapsed() >= Duration::from_secs(1) {
            let elapsed = last_metrics_time.elapsed().as_secs_f64();
            last_metrics_time = Instant::now();

            let pipe = crate::dataplane::global();
            let buffer_len = pipe.buffer.len();
            let buffer_cap = pipe.buffer.capacity();
            let buffer_sat = if buffer_cap == 0 {
                0.0
            } else {
                (buffer_len as f64 / buffer_cap as f64) * 100.0
            };
            let now_ticks = pipe.ticks_emitted();
            let delta = now_ticks.saturating_sub(last_ticks_emitted);
            last_ticks_emitted = now_ticks;
            let sps_rate = if elapsed > 0.0 {
                (delta as f64 / elapsed).round() as u32
            } else {
                0
            };
            let live_feed_active = pipe.has_external_feed();
            // Phase 0 active-MU proxy: 1 when the buffer is
            // populated, 0 otherwise. PR D wires real
            // auto-registration via the incoming svIDs.
            let active_mus = if buffer_len > 0 { 1 } else { 0 };
            let integrity_violations = pipe.buffer.verify_all().len();

            // PTP stays mocked until Phase 5 wires linuxptp.
            let ptp_offset = 12 + (now_ms % 7) as i64;

            let metrics_event = SsePayload::Metrics(DashboardMetrics {
                ptp_sync_status: "Locked".to_string(),
                ptp_offset_ns: ptp_offset,
                buffer_saturation: buffer_sat,
                active_mus,
                sps_rate,
                l1_opcua_active: true,
                l2_mqtt_active: false,
                l3_timescaledb_active: true,
                integrity_violations,
                live_feed_active,
            });

            if let Ok(json_str) = serde_json::to_string(&metrics_event) {
                let _ = tx.send(json_str);
            }
        }
    }
}

// Custom type-alias for Instant to ensure standard library compilation
type Instant = std::time::Instant;
