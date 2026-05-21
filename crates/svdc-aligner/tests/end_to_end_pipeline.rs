//! M1 → M2 end-to-end happy path. Publisher emits 8 SV frames →
//! LoopbackSubscriber yields them → ingress Decoder parses them →
//! Aligner emits TickRecords → TickBuffer holds them → consumer drains.
//!
//! This proves the full Phase 0 data-plane skeleton ties together. The
//! Phase 1/2 owners can swap in the AF_PACKET subscriber, lock-free
//! ingress ring, real binner/interpolator/calibrator, and dual circular
//! buffer behind the same surface this test exercises.

use ssiec_sv_publisher::{encode_frame, AsduFields, FrameParams, SampleData, MAX_FRAME_BYTES};
use svdc_aligner::{Aligner, TickBuffer};
use svdc_ingress::{
    Decoder, IngressFrame, IngressRing, IngressTimestamp, LoopbackSubscriber, Subscriber,
};

#[test]
fn publisher_through_ingress_through_aligner_lands_in_tick_buffer() {
    // ---- M1: publisher → loopback subscriber ----
    let mut sub = LoopbackSubscriber::new();
    let n_frames: u16 = 8;
    let base_ns: u64 = 1_700_000_000_000_000_000;
    let period_ns: u64 = 208_333; // 1 / 4800 s
    for i in 0..n_frames {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let asdu = AsduFields {
            sv_id: "E2E",
            smp_cnt: i,
            conf_rev: 1,
            smp_synch: 2,
            smp_rate: 4800,
            samples: SampleData::NOMINAL_3PH,
        };
        let n = encode_frame(&FrameParams::DEMO, &asdu, &mut buf).unwrap();
        sub.push_frame(
            buf[..n].to_vec(),
            IngressTimestamp::from_unix_ns(base_ns + u64::from(i) * period_ns),
        );
    }

    // ---- M1: decode + push into ingress ring ----
    let decoder = Decoder;
    let ingress_ring = IngressRing::new(32);
    while let Ok((bytes, ts)) = sub.next_frame() {
        let samples = decoder.decode_frame(&bytes).unwrap();
        ingress_ring
            .push(IngressFrame {
                timestamp: ts,
                samples,
            })
            .expect("ingress ring not full");
    }

    // ---- M2: aligner drains the ring into the tick buffer ----
    let mut aligner = Aligner::new(period_ns);
    let tick_buffer = TickBuffer::new(64);
    while let Some(frame) = ingress_ring.pop() {
        for tick in aligner.process_frame(frame) {
            assert!(
                matches!(
                    tick_buffer.push(tick),
                    svdc_aligner::buffer::PushOutcome::Appended
                ),
                "tick buffer should have headroom"
            );
        }
    }

    // ---- M3 consumer: drain the buffer, verify the result ----
    assert_eq!(tick_buffer.len(), usize::from(n_frames));
    let mut last_tick_id: Option<u64> = None;
    let mut last_ts: Option<u64> = None;
    while let Some(tick) = tick_buffer.pop() {
        if let Some(prev) = last_tick_id {
            assert_eq!(
                tick.tick_id,
                prev + 1,
                "tick_id must increment by 1 across the data plane"
            );
        }
        if let Some(prev) = last_ts {
            assert_eq!(
                tick.ts_utc_ns - prev,
                period_ns,
                "tick spacing must match the bin period"
            );
        }
        last_tick_id = Some(tick.tick_id);
        last_ts = Some(tick.ts_utc_ns);
    }
    assert_eq!(last_tick_id, Some(u64::from(n_frames) - 1));
}
