/* SVDC Console SSE Emitter
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use crate::sse::{DashboardMetrics, QseLog, SsePayload, WaveformSample};
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
    let mut last_qse_time = Instant::now();
    let mut last_ticks_emitted: u64 = 0;
    let mut qse_seq: u64 = 0;
    let mut angle: f32 = 0.0;

    // Simulated constants (used as a fallback when no live tick is
    // available — see `build_waveform_event`).
    let v_peak = 110.0 * 1.414; // Peak voltage (110V RMS)
    let i_peak = 5.0 * 1.414; // Peak current (5A RMS)
    let pi_2_3 = 2.0 * std::f32::consts::PI / 3.0; // 120 degrees in radians

    loop {
        interval_10hz.tick().await;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // 1. Waveform event at 10 Hz. PR E: when the data plane
        //    has a recent TickRecord, derive the per-phase values
        //    from `live_samples()` (channel order Ia Ib Ic In Va
        //    Vb Vc Vn) and tag the event with the latest observed
        //    svID. Otherwise fall back to the synthetic sinusoid
        //    so the polar diagram never goes blank during
        //    pre-pipeline startup.
        angle += 0.2;
        if angle > 2.0 * std::f32::consts::PI {
            angle -= 2.0 * std::f32::consts::PI;
        }
        let wave_event = build_waveform_event(
            &crate::dataplane::global(),
            now_ms,
            angle,
            v_peak,
            i_peak,
            pi_2_3,
        );
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
            // PR D: live distinct-svID count from the pipeline's
            // observation map. Replaces the Phase-0 "1 if buffer
            // non-empty" proxy.
            let active_mus = pipe.distinct_mu_count();
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

        // 3. Mock QSE audit-log row every ~7 s. Phase 0 keeps
        //    /monitoring's "QSE Write-Back Action Audit Logs"
        //    table non-empty during demos. Real wiring lands
        //    when the QSE write-back path is built (ADR-0020).
        if last_qse_time.elapsed() >= Duration::from_secs(7) {
            last_qse_time = Instant::now();
            qse_seq = qse_seq.wrapping_add(1);
            let qse_event = SsePayload::Qse(mock_qse_log(qse_seq, now_ms));
            if let Ok(json_str) = serde_json::to_string(&qse_event) {
                let _ = tx.send(json_str);
            }
        }
    }
}

/// Deterministic per-sequence mock QSE log row. Cycles through a
/// small fixture of operations + results so the demo table shows
/// variety. Replace with the real event source when QSE write-back
/// lands (ADR-0020).
fn mock_qse_log(seq: u64, now_ms: u64) -> QseLog {
    let fixtures: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "WBS-9.6a",
            "set_calibration",
            "MU-01 / ch4",
            "console:127.0.0.1",
            "applied",
            "green",
        ),
        (
            "WBS-2.9",
            "tamper_injected",
            "tick_buffer / synthetic",
            "operator:demo",
            "degraded",
            "amber",
        ),
        (
            "WBS-9.4a",
            "northbound_state_change",
            "Layer L1 (OPC UA)",
            "console:127.0.0.1",
            "applied",
            "green",
        ),
        (
            "WBS-2.9",
            "integrity_sweep",
            "tick_buffer",
            "scheduler",
            "applied",
            "green",
        ),
        (
            "WBS-9.6a",
            "scd_upload",
            "AA1J1Q01A1 sample SCD",
            "console:127.0.0.1",
            "applied",
            "green",
        ),
        (
            "ADR-0020",
            "qse_writeback",
            "MU-01 / ch4 — placeholder",
            "qse:planned",
            "rejected",
            "red",
        ),
    ];
    let pick = fixtures[(seq as usize) % fixtures.len()];
    QseLog {
        timestamp: iso8601_from_unix_ms(now_ms),
        wbs: pick.0.to_string(),
        operation: pick.1.to_string(),
        target: pick.2.to_string(),
        operator: pick.3.to_string(),
        result: pick.4.to_string(),
        result_color: pick.5.to_string(),
    }
}

/// Cheap ISO-8601 (UTC) formatter, no `chrono` dependency: just
/// good enough for the mock audit-log row.
fn iso8601_from_unix_ms(unix_ms: u64) -> String {
    let secs = (unix_ms / 1000) as i64;
    let millis = (unix_ms % 1000) as u32;
    // Convert seconds since epoch to (y, m, d, h, m, s) using the
    // civil-from-days algorithm.
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
    let secs_of_day = secs.rem_euclid(86_400) as u32;
    let h = secs_of_day / 3600;
    let mi = (secs_of_day / 60) % 60;
    let s = secs_of_day % 60;
    format!("{year:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}

/// Build the next `Waveform` event. PR E: prefer the latest
/// `TickRecord` from the shared `DataPipeline` buffer; fall back
/// to a synthetic 60 Hz sinusoid when the buffer is empty (no
/// producer attached).
///
/// Channel mapping per SDD §7.1 + the publisher's order in
/// `SampleData::NOMINAL_3PH`:
///
///   samples[0]=Ia samples[1]=Ib samples[2]=Ic samples[3]=In
///   samples[4]=Va samples[5]=Vb samples[6]=Vc samples[7]=Vn
///
/// The publisher emits raw integers in 9-2 LE scaled units
/// (currents = 0.001 A / LSB, voltages = 0.01 V / LSB); we
/// divide by the scale to produce f32 engineering units the UI's
/// existing polar diagram expects.
fn build_waveform_event(
    pipe: &crate::dataplane::DataPipeline,
    now_ms: u64,
    angle: f32,
    v_peak: f32,
    i_peak: f32,
    pi_2_3: f32,
) -> SsePayload {
    let recent = pipe.buffer.recent(1);
    if let Some(tick) = recent.into_iter().next() {
        let live = tick.live_samples();
        if live.len() >= 8 {
            let mu_id = pipe
                .seen_mus()
                .first()
                .map(|m| m.sv_id.clone())
                .unwrap_or_else(|| "MU-01".to_string());
            // Currents: 0.001 A per LSB. Voltages: 0.01 V per LSB.
            let i_scale = 0.001_f32;
            let v_scale = 0.01_f32;
            return SsePayload::Waveform(WaveformSample {
                mu_id,
                timestamp_ms: now_ms,
                i1: live[0].value_q as f32 * i_scale,
                i2: live[1].value_q as f32 * i_scale,
                i3: live[2].value_q as f32 * i_scale,
                i0: live[3].value_q as f32 * i_scale,
                v1: live[4].value_q as f32 * v_scale,
                v2: live[5].value_q as f32 * v_scale,
                v3: live[6].value_q as f32 * v_scale,
                v0: live[7].value_q as f32 * v_scale,
            });
        }
    }
    // Fallback: synthetic sinusoid so the polar diagram never
    // sits empty before the producer attaches.
    let v1 = v_peak * angle.sin();
    let v2 = v_peak * (angle - pi_2_3).sin();
    let v3 = v_peak * (angle + pi_2_3).sin();
    let v0 = v1 + v2 + v3;
    let i1 = i_peak * (angle - 0.1).sin();
    let i2 = i_peak * (angle - pi_2_3 - 0.1).sin();
    let i3 = i_peak * (angle + pi_2_3 - 0.1).sin();
    let i0 = i1 + i2 + i3;
    SsePayload::Waveform(WaveformSample {
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
    })
}

// Custom type-alias for Instant to ensure standard library compilation
type Instant = std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_formatter_matches_known_epoch() {
        // Unix epoch + 0 ms = 1970-01-01T00:00:00.000Z
        assert_eq!(iso8601_from_unix_ms(0), "1970-01-01T00:00:00.000Z");
        // 86_400_000 ms = exactly one day after epoch.
        assert_eq!(iso8601_from_unix_ms(86_400_000), "1970-01-02T00:00:00.000Z");
        // 1_700_000_000_000 ms = 2023-11-14T22:13:20.000 UTC (well-known
        // round number used in many docs as the "modern" timestamp).
        assert_eq!(
            iso8601_from_unix_ms(1_700_000_000_000),
            "2023-11-14T22:13:20.000Z"
        );
        // Sanity: every output ends with the Z + 3-digit millis.
        let s = iso8601_from_unix_ms(1_700_000_000_123);
        assert!(s.ends_with(".123Z"), "got {s}");
    }

    #[test]
    fn mock_qse_log_cycles_through_fixtures() {
        let a = mock_qse_log(0, 1_700_000_000_000);
        let b = mock_qse_log(1, 1_700_000_000_000);
        assert_ne!(a.operation, b.operation);
        // Same seq + same time → identical (determinism)
        let c = mock_qse_log(0, 1_700_000_000_000);
        assert_eq!(a.operation, c.operation);
    }

    #[test]
    fn qse_log_round_trips_as_json_with_event_type_tag() {
        let log = mock_qse_log(0, 1_700_000_000_000);
        let payload = SsePayload::Qse(log);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""event_type":"Qse""#));
        assert!(json.contains(r#""result_color":"green""#));
        assert!(json.contains(r#""operation":"set_calibration""#));
    }

    #[test]
    fn waveform_falls_back_to_synthetic_when_buffer_is_empty() {
        // Construct a fresh local pipeline so the test does not
        // race other tests on the global singleton.
        let pipe = crate::dataplane::DataPipeline::new();
        let event = build_waveform_event(
            &pipe,
            1_700_000_000_000,
            0.0,
            155.5,
            7.0,
            2.0 * std::f32::consts::PI / 3.0,
        );
        match event {
            SsePayload::Waveform(w) => {
                assert_eq!(w.mu_id, "MU-01");
                // Synthetic v1 = v_peak * sin(0) = 0
                assert!(w.v1.abs() < 1e-3, "v1 ≈ 0 at angle 0, got {}", w.v1);
            }
            _ => panic!("expected Waveform variant"),
        }
    }

    #[test]
    fn waveform_reads_live_samples_when_buffer_non_empty() {
        use svdc_core::{Sample, SampleOrigin, TickRecord};
        let pipe = crate::dataplane::DataPipeline::new();
        pipe.note_mu_observed("E2E_TEST_MU");
        let mut tick = TickRecord::empty(1, 1_700_000_000_000_000_000);
        tick.n_channels = 8;
        for i in 0..8 {
            tick.samples[i] = Sample {
                value_q: (i as i32 + 1) * 100,
                quality: 0,
                origin: SampleOrigin::Live.as_u8(),
                reserved: 0,
            };
        }
        tick.stamp_crc();
        pipe.buffer.push(tick);

        let event = build_waveform_event(
            &pipe,
            1_700_000_000_000,
            0.0,
            155.5,
            7.0,
            2.0 * std::f32::consts::PI / 3.0,
        );
        match event {
            SsePayload::Waveform(w) => {
                assert_eq!(w.mu_id, "E2E_TEST_MU");
                // i1 = ch0.value_q (100) * 0.001 = 0.1 A
                assert!((w.i1 - 0.1).abs() < 1e-4, "i1 = {}", w.i1);
                // v1 = ch4.value_q (500) * 0.01 = 5.0 V
                assert!((w.v1 - 5.0).abs() < 1e-4, "v1 = {}", w.v1);
            }
            _ => panic!("expected Waveform variant"),
        }
        pipe.reset();
        pipe.clear_seen_mus();
    }
}
