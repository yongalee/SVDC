//! WBS-2.8 / 2.9 — dual circular buffer.
//!
//! The aligner stages emitted [`TickRecord`]s in a buffer that the
//! northbound layers (svdc-api, svdc-opcua, historian) drain. Two
//! reasons it is "dual":
//!
//! 1. **Integrity (WBS-2.9)**: hashed checkpoints across the buffer
//!    allow corruption from a buggy producer to be caught before a
//!    consumer reads bad data. The current implementation owns the
//!    contract; the integrity overlay lives next to it in Phase 2.
//! 2. **Failover (WBS-2.9)**: a hot-spare buffer takes over without
//!    consumer-visible discontinuity if the primary is being checked,
//!    swapped, or invalidated.
//!
//! Phase 0 ships a single `Mutex<VecDeque<TickRecord>>` and exposes the
//! API shape the dual-CB will inherit. The Phase 2 owner replaces the
//! backing storage; consumers do not have to change.

use std::sync::Mutex;

use svdc_core::{IntegrityViolation, TickRecord};

/// Bounded FIFO of tick records. Drops the oldest on overflow rather
/// than rejecting new pushes, because dropping the *newest* tick would
/// stall the data plane in a way that's invisible to operators.
#[derive(Debug)]
pub struct TickBuffer {
    inner: Mutex<std::collections::VecDeque<TickRecord>>,
    capacity: usize,
}

impl TickBuffer {
    /// Construct a buffer with the given capacity. `capacity == 0` is
    /// rejected so the lock-free replacement does not need to handle
    /// a degenerate case.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "TickBuffer capacity must be > 0");
        Self {
            inner: Mutex::new(std::collections::VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Append one tick. If full, drops the oldest tick first.
    /// Returns whether a drop occurred so the caller can bump a
    /// metric.
    pub fn push(&self, tick: TickRecord) -> PushOutcome {
        let mut g = self.inner.lock().expect("tick buffer poisoned");
        let dropped = if g.len() >= self.capacity {
            g.pop_front();
            true
        } else {
            false
        };
        g.push_back(tick);
        if dropped {
            PushOutcome::DroppedOldest
        } else {
            PushOutcome::Appended
        }
    }

    /// Pop the oldest tick, or `None` if empty.
    pub fn pop(&self) -> Option<TickRecord> {
        self.inner.lock().expect("tick buffer poisoned").pop_front()
    }

    /// Snapshot the newest `n` ticks, newest first. Clones into the
    /// returned `Vec`; for hot-path callers, prefer [`Self::pop`].
    pub fn recent(&self, n: usize) -> Vec<TickRecord> {
        let g = self.inner.lock().expect("tick buffer poisoned");
        g.iter().rev().take(n).cloned().collect()
    }

    /// Current count.
    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Capacity (immutable after construction).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Verify every record's stored CRC against its recomputed value.
    /// Returns one [`IntegrityViolation`] per failure; empty `Vec`
    /// means the whole buffer is consistent.
    ///
    /// This is the diagnostic surface for WBS-2.9. Phase 2 will plug
    /// the dual-CB failover path into this method: if the primary
    /// returns any violation, swap to the hot spare and mark the
    /// surrounding tick window `DEGRADED`. Phase 0 just reports.
    ///
    /// Cost: O(n_channels × buffer.len()) — call from a slow path
    /// (operator-driven `/health` probe, periodic integrity sweep),
    /// not the hot read loop.
    pub fn verify_all(&self) -> Vec<IntegrityViolation> {
        let g = self.inner.lock().expect("tick buffer poisoned");
        g.iter().filter_map(|r| r.verify_crc().err()).collect()
    }
}

/// Outcome of a [`TickBuffer::push`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// New tick appended; no drop.
    Appended,
    /// Buffer was full; the oldest tick was dropped to make room.
    DroppedOldest,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(id: u64, ts: u64) -> TickRecord {
        TickRecord::empty(id, ts)
    }

    #[test]
    fn fifo_order_is_preserved() {
        let b = TickBuffer::new(4);
        for i in 0..3 {
            assert_eq!(b.push(tick(i, i * 1000)), PushOutcome::Appended);
        }
        assert_eq!(b.len(), 3);
        assert_eq!(b.pop().unwrap().tick_id, 0);
        assert_eq!(b.pop().unwrap().tick_id, 1);
        assert_eq!(b.pop().unwrap().tick_id, 2);
        assert!(b.pop().is_none());
    }

    #[test]
    fn push_on_full_drops_oldest_and_reports_outcome() {
        let b = TickBuffer::new(2);
        assert_eq!(b.push(tick(0, 0)), PushOutcome::Appended);
        assert_eq!(b.push(tick(1, 1)), PushOutcome::Appended);
        assert_eq!(b.push(tick(2, 2)), PushOutcome::DroppedOldest);
        assert_eq!(b.len(), 2);
        // tick 0 was dropped; remaining = [1, 2].
        assert_eq!(b.pop().unwrap().tick_id, 1);
        assert_eq!(b.pop().unwrap().tick_id, 2);
    }

    #[test]
    fn recent_returns_newest_first_bounded_by_n() {
        let b = TickBuffer::new(8);
        for i in 0..5 {
            b.push(tick(i, i));
        }
        let r = b.recent(3);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].tick_id, 4);
        assert_eq!(r[1].tick_id, 3);
        assert_eq!(r[2].tick_id, 2);
    }

    #[test]
    #[should_panic(expected = "TickBuffer capacity must be > 0")]
    fn zero_capacity_panics() {
        let _ = TickBuffer::new(0);
    }

    #[test]
    fn verify_all_passes_for_stamped_records() {
        let b = TickBuffer::new(8);
        for i in 0..4u64 {
            let mut t = tick(i, i * 1000);
            t.n_channels = 1;
            t.samples[0].value_q = i as i32;
            t.samples[0].origin = svdc_core::SampleOrigin::Live.as_u8();
            t.stamp_crc();
            b.push(t);
        }
        let violations = b.verify_all();
        assert!(violations.is_empty(), "stamped records must verify");
    }

    #[test]
    fn verify_all_flags_tampered_records() {
        let b = TickBuffer::new(8);
        // Push 3 records: stamp 2 properly, leave 1 with the wrong CRC.
        let mut good = tick(1, 100);
        good.n_channels = 1;
        good.samples[0].value_q = 10;
        good.stamp_crc();
        b.push(good);

        let mut tampered = tick(2, 200);
        tampered.n_channels = 1;
        tampered.samples[0].value_q = 20;
        tampered.stamp_crc();
        tampered.samples[0].value_q = 999; // post-stamp tamper
        b.push(tampered);

        let mut good2 = tick(3, 300);
        good2.n_channels = 1;
        good2.samples[0].value_q = 30;
        good2.stamp_crc();
        b.push(good2);

        let violations = b.verify_all();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].tick_id, 2);
    }
}
