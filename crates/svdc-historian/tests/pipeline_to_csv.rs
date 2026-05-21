//! Full-data-plane integration: publisher → ingress → aligner →
//! TickBuffer → InProcessSubscriber → Historian → CSV file → read
//! back and verify content.
//!
//! Six crates in one shot. This is the demo-grade regression guard
//! the rest of the workspace inherits when any boundary type
//! changes.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use ssiec_sv_publisher::{encode_frame, AsduFields, FrameParams, SampleData, MAX_FRAME_BYTES};
use svdc_aligner::{Aligner, TickBuffer};
use svdc_historian::{Historian, HistorianConfig};
use svdc_ingress::{
    Decoder, IngressFrame, IngressRing, IngressTimestamp, LoopbackSubscriber, Subscriber as InSub,
};
use svdc_subscribe::{ChannelSet, InProcessSubscriber, Subscriber};

fn unique_path(tag: &str) -> std::path::PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("svdc-historian-pipeline-{tag}-{ts}.csv"))
}

#[test]
fn full_data_plane_produces_csv_with_one_row_per_tick() {
    let path = unique_path("full");
    let _ = std::fs::remove_file(&path);

    // ---- M1: publisher → loopback subscriber ----
    let mut ingress_sub = LoopbackSubscriber::new();
    let n_frames: u16 = 6;
    let base_ns: u64 = 1_700_000_000_000_000_000;
    let period_ns: u64 = 208_333;
    for i in 0..n_frames {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let asdu = AsduFields {
            sv_id: "HIST_E2E",
            smp_cnt: i,
            conf_rev: 1,
            smp_synch: 2,
            smp_rate: 4800,
            samples: SampleData::NOMINAL_3PH,
        };
        let n = encode_frame(&FrameParams::DEMO, &asdu, &mut buf).unwrap();
        ingress_sub.push_frame(
            buf[..n].to_vec(),
            IngressTimestamp::from_unix_ns(base_ns + u64::from(i) * period_ns),
        );
    }

    // ---- M1: decode → ingress ring ----
    let decoder = Decoder;
    let ingress_ring = IngressRing::new(16);
    while let Ok((bytes, ts)) = ingress_sub.next_frame() {
        let samples = decoder.decode_frame(&bytes).unwrap();
        ingress_ring
            .push(IngressFrame {
                timestamp: ts,
                samples,
            })
            .expect("ingress ring not full");
    }

    // ---- M2: aligner → tick buffer ----
    let mut aligner = Aligner::new(period_ns);
    let tick_buffer = Arc::new(TickBuffer::new(64));
    while let Some(frame) = ingress_ring.pop() {
        for tick in aligner.process_frame(frame) {
            tick_buffer.push(tick);
        }
    }

    // ---- M3 → M4: subscriber + historian ----
    let subscriber = InProcessSubscriber::new(Arc::clone(&tick_buffer));
    let subscription = subscriber.subscribe(ChannelSet::all());
    let mut historian = Historian::new(HistorianConfig::csv_at(&path), subscription).unwrap();
    let n_written = historian.tick().unwrap();
    historian.flush().unwrap();
    assert_eq!(n_written, usize::from(n_frames));

    // ---- Verify the file on disk ----
    let body = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    // 1 header + n_frames data rows.
    assert_eq!(lines.len(), 1 + usize::from(n_frames));

    // Data rows in tick_id order, starting from 0.
    for (i, line) in lines.iter().enumerate().skip(1) {
        let tick_id = i - 1;
        let cols: Vec<&str> = line.split(',').collect();
        // tick_id, ts_utc_ns, n_channels, flags_hex, then 3 cols per channel.
        assert_eq!(cols.len(), 4 + 3 * svdc_core::MAX_CHANNELS);
        assert_eq!(cols[0].parse::<u64>().unwrap(), tick_id as u64);
        assert_eq!(cols[2].parse::<u16>().unwrap(), 8, "8 live channels");
        assert_eq!(cols[3], "0x0001", "COMPLETE flag only");
        // ch0 (Ia): publisher's NOMINAL_3PH puts 5000 there.
        assert_eq!(cols[4].parse::<i32>().unwrap(), 5000);
        // ch4 (Va) starts at column 4 + 3*4 = 16.
        assert_eq!(cols[16].parse::<i32>().unwrap(), 23000);
        // Origin of populated channels = 1 (Live).
        assert_eq!(cols[6], "1");
    }

    let _ = std::fs::remove_file(&path);
}

#[test]
fn streaming_writes_pick_up_new_ticks_across_multiple_tick_calls() {
    let path = unique_path("streaming");
    let _ = std::fs::remove_file(&path);

    let tick_buffer = Arc::new(TickBuffer::new(64));
    let subscriber = InProcessSubscriber::new(Arc::clone(&tick_buffer));
    let subscription = subscriber.subscribe(ChannelSet::all());
    let mut historian = Historian::new(HistorianConfig::csv_at(&path), subscription).unwrap();

    // First round: 3 ticks.
    for i in 0..3u64 {
        tick_buffer.push(svdc_core::TickRecord::empty(i, i * 1_000_000));
    }
    assert_eq!(historian.tick().unwrap(), 3);

    // Second round: 2 more ticks; historian picks up only the new ones.
    for i in 3..5u64 {
        tick_buffer.push(svdc_core::TickRecord::empty(i, i * 1_000_000));
    }
    assert_eq!(historian.tick().unwrap(), 2);

    // Third round: no new ticks.
    assert_eq!(historian.tick().unwrap(), 0);

    historian.flush().unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    // Header + 5 rows.
    assert_eq!(body.lines().count(), 6);
    assert_eq!(historian.rows_written(), 5);

    let _ = std::fs::remove_file(&path);
}
