//! WBS-2.4 — SPSC ring between M1 (ingress) and M2 (aligner).
//!
//! The architecturally important property of this ring is *single
//! producer, single consumer* with no allocation per push. Phase 1 will
//! land a true lock-free SPSC backed by `crossbeam-queue::ArrayQueue`
//! or `rtrb`. Phase 0 ships a `Mutex<VecDeque>` placeholder so the rest
//! of the ingress can be exercised end-to-end. The trait surface is
//! stable across both implementations so callers do not have to change
//! when the lock-free version lands.
//!
//! Behavioural promises that both implementations must satisfy:
//!  - `push` returns the rejected payload back to the caller when the
//!    ring is full (no silent drop, no blocking).
//!  - `pop` returns `None` rather than blocking when empty.
//!  - `capacity` is fixed at construction; no resize.

use std::sync::Mutex;

use crate::IngressFrame;

/// SPSC bounded ring carrying ingress frames from M1 to M2. Backed by
/// a `Mutex<VecDeque>` in Phase 0; the trait surface matches what a
/// lock-free queue exposes so the swap is invisible to callers.
#[derive(Debug)]
pub struct IngressRing {
    inner: Mutex<std::collections::VecDeque<IngressFrame>>,
    capacity: usize,
}

impl IngressRing {
    /// Construct a fixed-capacity ring. `capacity == 0` is rejected
    /// to keep the contract aligned with the lock-free replacement.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "IngressRing capacity must be > 0");
        Self {
            inner: Mutex::new(std::collections::VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push a frame. Returns `Ok(())` on success, or `Err(frame)`
    /// when the ring is full so the caller can record a drop metric
    /// and free the buffer. The Phase 1 lock-free version preserves
    /// this same signature.
    pub fn push(&self, frame: IngressFrame) -> Result<(), IngressFrame> {
        let mut g = self.inner.lock().expect("ingress ring poisoned");
        if g.len() >= self.capacity {
            return Err(frame);
        }
        g.push_back(frame);
        Ok(())
    }

    /// Pop the oldest frame, or `None` if empty.
    pub fn pop(&self) -> Option<IngressFrame> {
        self.inner
            .lock()
            .expect("ingress ring poisoned")
            .pop_front()
    }

    /// Current number of buffered frames.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fixed maximum size.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IngressTimestamp;
    use ssiec_sv_publisher::SampleData;

    fn dummy_frame(seq: u16) -> IngressFrame {
        IngressFrame {
            timestamp: IngressTimestamp::from_unix_ns(u64::from(seq) * 100),
            samples: vec![crate::DecodedSample {
                sv_id: "T".into(),
                smp_cnt: seq,
                conf_rev: 0,
                smp_synch: 0,
                smp_rate: 0,
                samples: SampleData::NOMINAL_3PH,
            }],
        }
    }

    #[test]
    fn fifo_order_is_preserved() {
        let ring = IngressRing::new(4);
        ring.push(dummy_frame(1)).unwrap();
        ring.push(dummy_frame(2)).unwrap();
        ring.push(dummy_frame(3)).unwrap();
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.pop().unwrap().samples[0].smp_cnt, 1);
        assert_eq!(ring.pop().unwrap().samples[0].smp_cnt, 2);
        assert_eq!(ring.pop().unwrap().samples[0].smp_cnt, 3);
        assert!(ring.pop().is_none());
        assert!(ring.is_empty());
    }

    #[test]
    fn full_ring_returns_rejected_frame() {
        let ring = IngressRing::new(2);
        ring.push(dummy_frame(1)).unwrap();
        ring.push(dummy_frame(2)).unwrap();
        let rejected = ring.push(dummy_frame(3)).unwrap_err();
        assert_eq!(rejected.samples[0].smp_cnt, 3);
        assert_eq!(ring.len(), 2);
    }

    #[test]
    #[should_panic(expected = "IngressRing capacity must be > 0")]
    fn zero_capacity_panics() {
        let _ = IngressRing::new(0);
    }
}
