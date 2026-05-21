//! End-to-end happy path through the Phase 0 ingress scaffold:
//! ssiec-sv-publisher encodes a frame → LoopbackSubscriber yields it →
//! Decoder parses it → IngressRing buffers it → consumer drains it.
//!
//! This proves the M1→M2 boundary types fit together; the lock-free
//! ring and real AF_PACKET subscriber replace pieces under the same
//! surface in Phase 1 without breaking this test.

use ssiec_sv_publisher::{encode_frame, AsduFields, FrameParams, SampleData, MAX_FRAME_BYTES};
use svdc_ingress::{
    Decoder, IngressFrame, IngressRing, IngressTimestamp, LoopbackSubscriber, Subscriber,
};

fn publish_into(sub: &mut LoopbackSubscriber, smp_cnt: u16, ts_ns: u64) {
    let mut buf = [0u8; MAX_FRAME_BYTES];
    let asdu = AsduFields {
        sv_id: "PIPELINE",
        smp_cnt,
        conf_rev: 1,
        smp_synch: 2,
        smp_rate: 4800,
        samples: SampleData::NOMINAL_3PH,
    };
    let n = encode_frame(&FrameParams::DEMO, &asdu, &mut buf).unwrap();
    sub.push_frame(buf[..n].to_vec(), IngressTimestamp::from_unix_ns(ts_ns));
}

#[test]
fn loopback_pipeline_drains_in_order_with_timestamps_and_smp_cnts() {
    let mut sub = LoopbackSubscriber::new();
    for i in 0u16..4 {
        publish_into(&mut sub, i, 1_000_000_000 + u64::from(i) * 208_333); // 4800Hz spacing
    }

    let decoder = Decoder;
    let ring = IngressRing::new(8);

    // M1 producer loop.
    while let Ok((bytes, ts)) = sub.next_frame() {
        let samples = decoder.decode_frame(&bytes).unwrap();
        ring.push(IngressFrame {
            timestamp: ts,
            samples,
        })
        .expect("ring not full");
    }
    assert_eq!(ring.len(), 4);

    // M2 consumer drain.
    let mut drained: Vec<IngressFrame> = Vec::new();
    while let Some(f) = ring.pop() {
        drained.push(f);
    }
    assert_eq!(drained.len(), 4);
    for (i, f) in drained.iter().enumerate() {
        assert_eq!(f.samples[0].smp_cnt, i as u16);
        assert_eq!(f.samples[0].sv_id, "PIPELINE");
        assert_eq!(f.samples[0].samples, SampleData::NOMINAL_3PH);
        assert_eq!(f.timestamp.unix_ns(), 1_000_000_000 + (i as u64) * 208_333);
    }
}
