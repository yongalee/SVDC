/* SVDC Console SSE Emitter
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tokio::time;
use crate::sse::{DashboardMetrics, WaveformSample, SsePayload};

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
    let i_peak = 5.0 * 1.414;   // Peak current (5A RMS)
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
                active_mus: 2,
                sps_rate: 4000,
                l1_opcua_active: true,
                l2_mqtt_active: false,
                l3_timescaledb_active: true,
            });

            if let Ok(json_str) = serde_json::to_string(&metrics_event) {
                let _ = tx.send(json_str);
            }
        }
    }
}

// Custom type-alias for Instant to ensure standard library compilation
type Instant = std::time::Instant;
