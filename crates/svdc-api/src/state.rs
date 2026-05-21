//! Shared daemon state that every management-API handler reads.
//!
//! Phase 0 holds a startup [`Instant`] (for uptime) and an
//! `Arc<TickBuffer>` (for liveness / integrity / size metrics).
//! Phase 2 will add an `Arc<ChannelRegistry>` and Phase 3 a write
//! handle into `OperationalState` so the calibration POST has
//! somewhere to land. The struct is intentionally non-`Clone` for
//! the inner fields — wrap the whole thing in `Arc` and clone the
//! Arc.

use std::sync::Arc;
use std::time::Instant;

use svdc_aligner::TickBuffer;

/// Shared context handed to every management-API handler.
pub struct ManagementContext {
    /// When the daemon started. `Instant::elapsed()` gives uptime.
    pub started_at: Instant,
    /// Live tick buffer the data plane writes into. Handlers read
    /// length, drop counts, integrity verdicts.
    pub tick_buffer: Arc<TickBuffer>,
}

impl ManagementContext {
    /// Construct from a tick buffer; sets `started_at` to now.
    pub fn new(tick_buffer: Arc<TickBuffer>) -> Self {
        Self {
            started_at: Instant::now(),
            tick_buffer,
        }
    }

    /// Daemon uptime in milliseconds. Used by `/health` and the
    /// process-up gauge in `/metrics`.
    pub fn uptime_ms(&self) -> u128 {
        self.started_at.elapsed().as_millis()
    }
}
