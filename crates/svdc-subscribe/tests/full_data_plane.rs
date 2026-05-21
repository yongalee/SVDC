//! End-to-end through every M-stage:
//! publisher → ingress subscriber → decoder → IngressRing → Aligner →
//! TickBuffer → InProcessSubscriber.
//!
//! This is the first test in the workspace that drives all five
//! crates of the data plane in one shot. It is the regression guard
//! the Phase 4 C ABI and the Phase 4 UNIX-socket binding will land
//! behind: both wrap [`InProcessSubscriber`], so as long as the
//! cursor / read_since semantics hold here, the transport wrappers
//! inherit the same correctness.

use std::sync::Arc;

use ssiec_sv_publisher::{encode_frame, AsduFields, FrameParams, SampleData, MAX_FRAME_BYTES};
use svdc_aligner::{Aligner, TickBuffer};
use svdc_core::{flags, SampleOrigin};
use svdc_ingress::{
    Decoder, IngressFrame, IngressRing, IngressTimestamp, LoopbackSubscriber, Subscriber as InSub,
};
use svdc_subscribe::{ChannelSet, InProcessSubscriber, Subscriber};

#[test]
fn one_subscriber_drains_the_full_data_plane() {
    // ---- M1: publisher → loopback subscriber ----
    let mut ingress_sub = LoopbackSubscriber::new();
    let n_frames: u16 = 4;
    let base_ns: u64 = 1_700_000_000_000_000_000;
    let period_ns: u64 = 208_333;
    for i in 0..n_frames {
        let mut buf = [0u8; MAX_FRAME_BYTES];
        let asdu = AsduFields {
            sv_id: "FULL_E2E",
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
    assert_eq!(tick_buffer.len(), usize::from(n_frames));

    // ---- M3→M4: in-process subscriber drains the buffer ----
    let subscriber = InProcessSubscriber::new(Arc::clone(&tick_buffer));
    let mut subscription = subscriber.subscribe(ChannelSet::all());

    // First read_since returns every record, oldest-first, advancing the cursor.
    let records = subscription.read_since();
    assert_eq!(records.len(), usize::from(n_frames));
    assert_eq!(records.first().unwrap().tick_id, 0);
    assert_eq!(
        records.last().unwrap().tick_id,
        u64::from(n_frames) - 1,
        "cursor should sit at the last delivered tick"
    );
    assert_eq!(subscription.cursor(), u64::from(n_frames) - 1);

    // Second call returns nothing — the buffer is fully drained from this
    // subscriber's point of view.
    assert!(subscription.read_since().is_empty());

    // Every record carries 8 live channels with the COMPLETE flag.
    for r in &records {
        assert_eq!(r.n_channels, 8, "publisher emits 8 channels per frame");
        assert!(r.has_flag(flags::COMPLETE));
        for s in r.live_samples() {
            assert_eq!(s.origin, SampleOrigin::Live.as_u8());
        }
    }
}

#[test]
fn two_subscribers_drain_independently_from_the_same_buffer() {
    let tick_buffer = Arc::new(TickBuffer::new(16));
    for i in 0..5u64 {
        tick_buffer.push(svdc_core::TickRecord::empty(i, i * 1_000_000));
    }

    let subscriber = InProcessSubscriber::new(Arc::clone(&tick_buffer));
    let mut a = subscriber.subscribe(ChannelSet::all());
    let mut b = subscriber.subscribe(ChannelSet::specific([4, 5, 6]));

    // a reads everything.
    assert_eq!(a.read_since().len(), 5);
    assert_eq!(a.cursor(), 4);

    // b reads the same 5 records (channel filter is advisory in Phase 0).
    let b_recs = b.read_since();
    assert_eq!(b_recs.len(), 5);
    assert_eq!(b.cursor(), 4);

    // New tick lands; only future reads pick it up.
    tick_buffer.push(svdc_core::TickRecord::empty(5, 5_000_000));
    assert_eq!(a.read_since().len(), 1);
    assert_eq!(b.read_since().len(), 1);
    assert_eq!(a.cursor(), 5);
    assert_eq!(b.cursor(), 5);
}

#[test]
fn read_latest_does_not_advance_cursor() {
    let tick_buffer = Arc::new(TickBuffer::new(8));
    for i in 0..4u64 {
        tick_buffer.push(svdc_core::TickRecord::empty(i, i));
    }
    let subscriber = InProcessSubscriber::new(tick_buffer);
    let subscription = subscriber.subscribe(ChannelSet::all());

    let snapshot = subscription.read_latest(2);
    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].tick_id, 3);
    assert_eq!(snapshot[1].tick_id, 2);
    assert_eq!(
        subscription.cursor(),
        0,
        "read_latest is a snapshot, not a stream"
    );
}
