//! `svdc-subscribe` — northbound subscriber API (M3→M4).
//!
//! Implements the in-process Rust surface that maps onto the C ABI
//! sketched in SDD §8.2:
//!
//! ```c
//! SvdcCursor svdc_subscribe(const ChannelSet* cs);
//! int        svdc_read_latest(SvdcCursor, size_t k, TickRecord** out);
//! int        svdc_read_since (SvdcCursor, TickRecord** out, size_t* n_out);
//! void       svdc_release    (SvdcCursor, TickRecord**);
//! void       svdc_unsubscribe(SvdcCursor);
//! ```
//!
//! The Rust equivalent is:
//!
//! ```text
//! Subscriber::subscribe(ChannelSet) -> Subscription
//! Subscription::read_latest(k)        -> Vec<TickRecord>
//! Subscription::read_since()          -> Vec<TickRecord>   // advances cursor
//! drop(subscription)                   // ≡ svdc_unsubscribe
//! ```
//!
//! Phase 0 ships exactly one [`Subscriber`] implementation:
//! [`InProcessSubscriber`] wrapping `Arc<TickBuffer>`. It clones
//! records into the returned `Vec` (Phase 0 is allocation-friendly;
//! the zero-copy `&[TickRecord]` variant lands with the lock-free
//! SPSC buffer in Phase 4). The C ABI in `svdc-cabi` (also Phase 4)
//! will wrap this same surface; the UNIX-socket binding wraps it as
//! a separate transport without re-implementing the cursor logic.
//!
//! ADR-0010 documents the design.
//!
//! OWNER: claude-code (Phase 0 scaffold + ADR-0010). Phase 4 transport
//! wrappers (C ABI, UDS) and zero-copy reads are assigned to
//! Antigravity.
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod channels;

pub use channels::ChannelSet;

use std::sync::Arc;

use svdc_aligner::TickBuffer;
use svdc_core::TickRecord;

/// Factory for [`Subscription`]s. Pluggable so consumers can be
/// tested against a mock buffer; production wires
/// [`InProcessSubscriber`].
pub trait Subscriber {
    /// Open a new subscription. The returned [`Subscription`] holds
    /// its own cursor; concurrent subscribers do not interfere.
    fn subscribe(&self, channels: ChannelSet) -> Subscription;
}

/// One open subscription. Holds:
/// - the [`ChannelSet`] the consumer requested (advisory in Phase 0),
/// - the highest tick ID delivered so far (the cursor),
/// - a shared handle to the [`TickBuffer`] it reads from.
///
/// Dropping the value is the Rust equivalent of `svdc_unsubscribe`.
///
/// The cursor is `Option<u64>` internally so a fresh subscription
/// (nothing delivered yet) can be distinguished from "delivered up to
/// tick 0". The [`Self::cursor`] getter folds `None` to `0` for
/// callers that just want the last-delivered ID and treat the
/// pre-delivery state as "0".
#[derive(Debug)]
pub struct Subscription {
    cursor: Option<u64>,
    channels: ChannelSet,
    buffer: Arc<TickBuffer>,
}

impl Subscription {
    /// Read the newest `k` records (or fewer if the buffer has less).
    /// Does **not** advance the cursor — use [`Self::read_since`] for
    /// gap-free streaming reads.
    pub fn read_latest(&self, k: usize) -> Vec<TickRecord> {
        self.buffer.recent(k)
    }

    /// Read every record with `tick_id > cursor` (or all records, if
    /// nothing has been delivered yet). Returned oldest-first.
    /// Advances the cursor to the newest tick returned, so the next
    /// call only returns freshly emitted records.
    ///
    /// Phase 0 implementation walks `TickBuffer::recent(usize::MAX)`
    /// and filters; this is O(buffer.len()) per call but stays
    /// correct as long as the buffer's drop-oldest policy hasn't
    /// retired the cursor's tick. Phase 4's zero-copy implementation
    /// will track the cursor directly inside the lock-free buffer.
    pub fn read_since(&mut self) -> Vec<TickRecord> {
        let recent = self.buffer.recent(usize::MAX);
        // recent() is newest-first; sort ascending after filtering.
        let mut fresh: Vec<TickRecord> = recent
            .into_iter()
            .filter(|r| match self.cursor {
                None => true,
                Some(c) => r.tick_id > c,
            })
            .collect();
        fresh.sort_by_key(|r| r.tick_id);
        if let Some(last) = fresh.last() {
            self.cursor = Some(last.tick_id);
        }
        fresh
    }

    /// Highest tick ID this subscription has observed via
    /// [`Self::read_since`]. Returns `0` before the first delivery
    /// (callers that need to distinguish "fresh" from "delivered up
    /// to tick 0" should consult [`Self::has_started`]).
    pub fn cursor(&self) -> u64 {
        self.cursor.unwrap_or(0)
    }

    /// Whether [`Self::read_since`] has delivered at least one record.
    pub fn has_started(&self) -> bool {
        self.cursor.is_some()
    }

    /// Channel filter requested at subscribe time. Advisory in
    /// Phase 0; honoured by Phase 1's channel-registry mapping.
    pub fn channel_set(&self) -> &ChannelSet {
        &self.channels
    }
}

/// In-process subscriber: wraps `Arc<TickBuffer>` directly. The
/// daemon hands out one instance from `svdc-bin` and every
/// node-local consumer constructs subscriptions through it.
#[derive(Debug, Clone)]
pub struct InProcessSubscriber {
    buffer: Arc<TickBuffer>,
}

impl InProcessSubscriber {
    /// Wrap a `TickBuffer`. The same buffer can back any number of
    /// subscribers; subscriptions get cheap `Arc` clones.
    pub fn new(buffer: Arc<TickBuffer>) -> Self {
        Self { buffer }
    }
}

impl Subscriber for InProcessSubscriber {
    fn subscribe(&self, channels: ChannelSet) -> Subscription {
        Subscription {
            cursor: None,
            channels,
            buffer: Arc::clone(&self.buffer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svdc_core::TickRecord;

    fn buffer_with(ticks: &[(u64, u64)]) -> Arc<TickBuffer> {
        let b = Arc::new(TickBuffer::new(64));
        for (id, ts) in ticks {
            b.push(TickRecord::empty(*id, *ts));
        }
        b
    }

    #[test]
    fn fresh_subscription_has_zero_cursor() {
        let sub = InProcessSubscriber::new(buffer_with(&[]));
        let s = sub.subscribe(ChannelSet::all());
        assert_eq!(s.cursor(), 0);
        assert!(!s.has_started());
        assert_eq!(s.channel_set(), &ChannelSet::all());
    }

    #[test]
    fn has_started_distinguishes_pre_delivery_from_delivered_zero() {
        // Pre-delivery: cursor() == 0 but has_started() == false.
        let sub = InProcessSubscriber::new(buffer_with(&[(0, 10)]));
        let mut s = sub.subscribe(ChannelSet::all());
        assert_eq!(s.cursor(), 0);
        assert!(!s.has_started());

        // After delivering tick_id 0: cursor() still == 0, but has_started true.
        let r = s.read_since();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].tick_id, 0);
        assert_eq!(s.cursor(), 0);
        assert!(s.has_started());
    }

    #[test]
    fn read_latest_returns_newest_first_bounded_by_k() {
        let sub = InProcessSubscriber::new(buffer_with(&[(1, 10), (2, 20), (3, 30), (4, 40)]));
        let s = sub.subscribe(ChannelSet::all());
        let r = s.read_latest(2);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].tick_id, 4);
        assert_eq!(r[1].tick_id, 3);
        // Cursor is NOT advanced by read_latest.
        assert_eq!(s.cursor(), 0);
    }

    #[test]
    fn read_since_returns_oldest_first_and_advances_cursor() {
        let sub = InProcessSubscriber::new(buffer_with(&[(1, 10), (2, 20), (3, 30)]));
        let mut s = sub.subscribe(ChannelSet::all());
        let r = s.read_since();
        assert_eq!(r.len(), 3);
        // Returned oldest-first → callers can stream into a sink.
        assert_eq!(r[0].tick_id, 1);
        assert_eq!(r[2].tick_id, 3);
        assert_eq!(s.cursor(), 3);

        // Second call is empty until more arrive.
        assert!(s.read_since().is_empty());
        assert_eq!(s.cursor(), 3, "cursor unchanged on empty read");
    }

    #[test]
    fn read_since_picks_up_only_new_records() {
        let buf = buffer_with(&[(1, 10), (2, 20)]);
        let sub = InProcessSubscriber::new(Arc::clone(&buf));
        let mut s = sub.subscribe(ChannelSet::all());
        let r1 = s.read_since();
        assert_eq!(r1.len(), 2);
        assert_eq!(s.cursor(), 2);

        // New records arrive.
        buf.push(TickRecord::empty(3, 30));
        buf.push(TickRecord::empty(4, 40));

        let r2 = s.read_since();
        assert_eq!(r2.len(), 2);
        assert_eq!(r2[0].tick_id, 3);
        assert_eq!(r2[1].tick_id, 4);
        assert_eq!(s.cursor(), 4);
    }

    #[test]
    fn concurrent_subscriptions_have_independent_cursors() {
        let buf = buffer_with(&[(1, 10), (2, 20), (3, 30)]);
        let sub = InProcessSubscriber::new(buf);
        let mut a = sub.subscribe(ChannelSet::all());
        let mut b = sub.subscribe(ChannelSet::specific([4]));
        // a drains everything.
        let ra = a.read_since();
        assert_eq!(ra.len(), 3);
        // b also drains everything (channel set is advisory in Phase 0).
        let rb = b.read_since();
        assert_eq!(rb.len(), 3);
        assert_eq!(a.cursor(), 3);
        assert_eq!(b.cursor(), 3);
        assert_eq!(b.channel_set(), &ChannelSet::specific([4]));
    }
}
