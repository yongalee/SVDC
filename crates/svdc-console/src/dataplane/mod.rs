//! Data-plane demo runner — the one place that drives every crate
//! built in PRs #42…#50 from inside the console process.
//!
//! Why this exists: today's data-plane crates (`svdc-ingress`,
//! `svdc-aligner`, `svdc-subscribe`, `svdc-historian`, `svdc-api`)
//! are wired against stable boundary types in `svdc-core`, but the
//! daemon (`svdc-bin`) does not yet instantiate them — daemon
//! wiring is deliberately deferred until the file is unlocked from
//! concurrent UI styling work.
//!
//! In the meantime, the operator can verify every data-plane
//! feature through the [`routes::dataplane`](crate::routes::dataplane)
//! UI screen, which:
//!
//! 1. spawns a tokio task that synthesises one `IngressFrame` per
//!    `tick_interval`, pushes it through an [`Aligner`], and lands
//!    the resulting `TickRecord` in a shared [`TickBuffer`];
//! 2. exposes an [`InProcessSubscriber`] so the historian can drain
//!    the buffer to a CSV;
//! 3. lets the operator tamper a record post-stamp so the integrity
//!    overlay (PR #49) fires;
//! 4. surfaces the live state for HTMX-polled status panels.
//!
//! This is **not** the production data plane — there is no
//! south-bound capture, no PTP timestamping, no lock-free SPSC ring.
//! It is the smallest end-to-end loop that touches every crate, run
//! from the same process as the operator console so a single
//! browser tab can verify the whole stack.
//!
//! OWNER: claude-code. NFR-10: English-only.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ssiec_sv_publisher::{ChannelSample, SampleData};
use svdc_aligner::{Aligner, TickBuffer};
use svdc_historian::{Historian, HistorianConfig};
use svdc_ingress::{DecodedSample, IngressFrame, IngressTimestamp};
use svdc_subscribe::{ChannelSet, InProcessSubscriber, Subscriber};

/// Tick buffer capacity for the demo pipeline. ~1 second at the
/// default tick interval below.
pub const DEMO_BUFFER_CAPACITY: usize = 256;

/// Period between synthesised ticks. 50 ms is slow enough to read in
/// the browser, fast enough to demonstrate the buffer rolling.
pub const DEFAULT_TICK_INTERVAL: Duration = Duration::from_millis(50);

/// Shared state read by every route handler in `routes::dataplane`.
pub struct DataPipeline {
    /// Shared tick buffer the background task writes into.
    pub buffer: Arc<TickBuffer>,
    /// Subscriber factory bound to `buffer`. Cheap to clone.
    pub subscriber: InProcessSubscriber,
    /// Historian CSV path. The historian itself is owned by the
    /// background task; the path is exposed so the UI can offer a
    /// download link.
    pub historian_path: PathBuf,
    /// Whether the background task is currently running. The task
    /// polls this and exits cleanly when set to `false`.
    running: AtomicBool,
    /// Background task handle so [`stop`] can join cleanly.
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// How many ticks the running task has emitted since it started.
    /// Reset to zero on each `start` call.
    ticks_emitted: AtomicU64,
    /// How many post-stamp tamper events the operator has triggered.
    tamper_count: AtomicU64,
    /// How many integrity violations the most recent `verify_all`
    /// sweep observed. Refreshed by `recent_status`.
    last_violations: AtomicU64,
    /// True when an external producer (the daemon's `--ingress-udp`
    /// task) is feeding ticks into the buffer. When set, the
    /// `/dataplane` synthetic loop refuses to start so the two
    /// feeds do not interleave (ADR-0015 §3).
    external_feed_active: AtomicBool,
    /// Distinct svIDs observed in the ingress stream (synthetic or
    /// live UDP). One entry per svID; updated on every frame.
    /// Populates the auto-observed section of `/south/mus` (PR D)
    /// and the `active_mus` count on the Dashboard.
    seen_mus: Mutex<BTreeMap<String, MuObservation>>,
    /// Currently selected vendor preset for the synthetic loop.
    /// `None` = the generic `DATAPLANE_DEMO` svID (default).
    /// Operator picks from the `/dataplane` vendor selector
    /// (PR F). Real UDP ingress ignores this — vendor identity
    /// there is whatever the simulator emits.
    selected_vendor: Mutex<Option<&'static ssiec_sv_publisher::VendorProfile>>,
    /// True when `svdc-bin` was started with `--enable-l0-demo` and
    /// the L0 reference subscriber task is running. Read by
    /// `/north` to badge L0 as "Wired (running)" vs "Wired
    /// (not started)" without forcing the UI to guess from log
    /// scraping. Set by `spawn_l0_demo` (PR H).
    l0_demo_active: AtomicBool,
    /// Highest tick_id the L0 demo has emitted via `read_since`.
    /// Surfaced on `/north/L0` as the cursor proof-of-life.
    l0_demo_last_tick_id: AtomicU64,
    /// Total ticks the L0 demo has drained since it started.
    /// Distinct from [`Self::ticks_emitted`] because that counter
    /// tracks the synthetic loop, not the L0 subscriber.
    l0_demo_total_ticks: AtomicU64,
}

/// One row in [`DataPipeline::seen_mus`]. Tracks first-seen,
/// last-seen, and the running frame count for a single svID.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MuObservation {
    /// The svID exactly as it appeared on the wire (ASN.1
    /// VisibleString from `SampledValueControl::smvID`).
    pub sv_id: String,
    /// Unix-ms when the daemon first observed this svID.
    pub first_seen_ms: u64,
    /// Unix-ms of the most recent frame carrying this svID.
    pub last_seen_ms: u64,
    /// Number of frames the daemon has seen carrying this svID.
    pub frame_count: u64,
}

impl DataPipeline {
    /// Build a fresh pipeline with an empty buffer and the default
    /// historian path. The task is not started here; call
    /// [`Self::start`] to spawn it.
    pub fn new() -> Self {
        let buffer = Arc::new(TickBuffer::new(DEMO_BUFFER_CAPACITY));
        let subscriber = InProcessSubscriber::new(Arc::clone(&buffer));
        let path = std::env::temp_dir().join("svdc-dataplane-demo.csv");
        Self {
            buffer,
            subscriber,
            historian_path: path,
            running: AtomicBool::new(false),
            handle: Mutex::new(None),
            ticks_emitted: AtomicU64::new(0),
            tamper_count: AtomicU64::new(0),
            last_violations: AtomicU64::new(0),
            external_feed_active: AtomicBool::new(false),
            seen_mus: Mutex::new(BTreeMap::new()),
            selected_vendor: Mutex::new(None),
            l0_demo_active: AtomicBool::new(false),
            l0_demo_last_tick_id: AtomicU64::new(0),
            l0_demo_total_ticks: AtomicU64::new(0),
        }
    }

    /// Mark the L0 reference subscriber as running (or stopped).
    /// Called once by `spawn_l0_demo` in `svdc-bin` after the task
    /// successfully subscribes. The flag drives the badge on
    /// `/north` and the "Wired (running)" state on `/north/L0`.
    pub fn mark_l0_demo_active(&self, active: bool) {
        self.l0_demo_active.store(active, Ordering::Relaxed);
    }

    /// Record one tick consumed by the L0 demo. Updates both the
    /// last-seen cursor and the running total. Called once per
    /// tick inside `spawn_l0_demo`'s `read_since` drain.
    pub fn record_l0_demo_tick(&self, tick_id: u64) {
        self.l0_demo_last_tick_id.store(tick_id, Ordering::Relaxed);
        self.l0_demo_total_ticks.fetch_add(1, Ordering::Relaxed);
    }

    /// Whether the L0 demo subscriber is running. False means the
    /// daemon was started without `--enable-l0-demo`; toggling at
    /// runtime is not supported (the flag is read once at boot).
    pub fn l0_demo_active(&self) -> bool {
        self.l0_demo_active.load(Ordering::Relaxed)
    }

    /// Highest tick_id the L0 demo has consumed since it started.
    /// Zero before the first read or when the demo is not running.
    pub fn l0_demo_last_tick_id(&self) -> u64 {
        self.l0_demo_last_tick_id.load(Ordering::Relaxed)
    }

    /// Total ticks drained by the L0 demo since process start.
    pub fn l0_demo_total_ticks(&self) -> u64 {
        self.l0_demo_total_ticks.load(Ordering::Relaxed)
    }

    /// Set the synthetic-loop vendor preset by name (case-insensitive
    /// match against `vendor::lookup`). Pass `None` to revert to the
    /// generic `DATAPLANE_DEMO` svID.
    pub fn set_vendor(&self, name: Option<&str>) -> Result<(), &'static str> {
        let resolved = match name {
            None | Some("") | Some("none") => None,
            Some(n) => Some(ssiec_sv_publisher::vendor::lookup(n).ok_or("unknown vendor preset")?),
        };
        if let Ok(mut g) = self.selected_vendor.lock() {
            *g = resolved;
            Ok(())
        } else {
            Err("vendor lock poisoned")
        }
    }

    /// Current vendor preset name (`None` for the generic demo).
    pub fn selected_vendor_name(&self) -> Option<&'static str> {
        self.selected_vendor
            .lock()
            .ok()
            .and_then(|g| g.map(|v| v.name))
    }

    /// The current vendor preset, used by the synthetic loop's
    /// frame builder to stamp the svID + rate.
    fn current_vendor(&self) -> Option<&'static ssiec_sv_publisher::VendorProfile> {
        self.selected_vendor.lock().ok().and_then(|g| *g)
    }

    /// Record one observation of `sv_id`. Called from the ingress
    /// hot path (synthetic loop or UDP receive task) just before
    /// the frame enters the aligner. The first call for a new svID
    /// records `first_seen_ms`; subsequent calls update
    /// `last_seen_ms` and increment `frame_count`.
    pub fn note_mu_observed(&self, sv_id: &str) {
        let now = now_unix_ms();
        let mut g = self.seen_mus.lock().expect("seen_mus lock poisoned");
        let entry = g.entry(sv_id.to_string()).or_insert_with(|| MuObservation {
            sv_id: sv_id.to_string(),
            first_seen_ms: now,
            last_seen_ms: now,
            frame_count: 0,
        });
        entry.last_seen_ms = now;
        entry.frame_count = entry.frame_count.saturating_add(1);
    }

    /// Snapshot of all observed MUs, sorted by svID. Cheap to call
    /// from a route handler.
    pub fn seen_mus(&self) -> Vec<MuObservation> {
        self.seen_mus
            .lock()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Distinct-svID count, used by the Dashboard `active_mus` tile.
    pub fn distinct_mu_count(&self) -> usize {
        self.seen_mus.lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Drop every observation. Test helper.
    pub fn clear_seen_mus(&self) {
        if let Ok(mut g) = self.seen_mus.lock() {
            g.clear();
        }
    }

    /// Mark that an external producer is feeding the buffer. Called
    /// by the daemon when `--ingress-udp` binds successfully. After
    /// this flag is set, [`Self::start`] returns `Err` so the
    /// in-process synthetic loop cannot fight the live feed for
    /// the same buffer.
    pub fn mark_external_feed(&self, active: bool) {
        self.external_feed_active.store(active, Ordering::SeqCst);
    }

    /// Whether an external (UDP) feed is currently driving the
    /// buffer. Read by the `/dataplane` UI to badge the panel.
    pub fn has_external_feed(&self) -> bool {
        self.external_feed_active.load(Ordering::Relaxed)
    }

    /// Increment the tick-emit counter; called by the daemon's
    /// external ingress task each time it pushes a tick into the
    /// shared buffer so the `/dataplane` status panel shows the
    /// same per-second rate it would for the synthetic loop.
    pub fn record_external_tick(&self) {
        self.ticks_emitted.fetch_add(1, Ordering::Relaxed);
    }

    /// Whether the background loop is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Number of ticks the current (or most recent) run has emitted.
    pub fn ticks_emitted(&self) -> u64 {
        self.ticks_emitted.load(Ordering::Relaxed)
    }

    /// Number of operator-triggered tampers.
    pub fn tamper_count(&self) -> u64 {
        self.tamper_count.load(Ordering::Relaxed)
    }

    /// Snapshot of the most recent status. Called by the status
    /// endpoint; also re-runs the integrity sweep so the operator
    /// sees a fresh verdict on every poll.
    pub fn snapshot(&self) -> DataPipelineSnapshot {
        let violations = self.buffer.verify_all();
        self.last_violations
            .store(violations.len() as u64, Ordering::Relaxed);

        let recent = self.buffer.recent(1);
        let newest = recent.into_iter().next();
        let (latest_tick_id, latest_ts_ns, latest_crc, latest_ch0_value) = match &newest {
            Some(r) => (
                Some(r.tick_id),
                Some(r.ts_utc_ns),
                Some(r.crc),
                r.live_samples().first().map(|s| s.value_q),
            ),
            None => (None, None, None, None),
        };
        let violation_tick_ids: Vec<u64> = violations.iter().map(|v| v.tick_id).collect();

        DataPipelineSnapshot {
            running: self.is_running(),
            buffer_len: self.buffer.len(),
            buffer_capacity: self.buffer.capacity(),
            ticks_emitted: self.ticks_emitted(),
            tamper_count: self.tamper_count(),
            integrity_violations: violations.len(),
            violation_tick_ids,
            latest_tick_id,
            latest_ts_ns,
            latest_crc,
            latest_ch0_value,
            historian_path: self.historian_path.display().to_string(),
            historian_exists: self.historian_path.exists(),
        }
    }

    /// Spawn the background tick generator. Returns `Err` if the
    /// pipeline is already running or if an external feed is
    /// currently driving the same buffer.
    pub fn start(self: &Arc<Self>) -> Result<(), &'static str> {
        if self.external_feed_active.load(Ordering::SeqCst) {
            return Err(
                "external --ingress-udp feed is active; restart the daemon without it to use the synthetic loop",
            );
        }
        if self.running.swap(true, Ordering::SeqCst) {
            // already running
            self.running.store(true, Ordering::SeqCst);
            return Err("data-plane pipeline is already running");
        }
        self.ticks_emitted.store(0, Ordering::Relaxed);

        let me = Arc::clone(self);
        let handle = tokio::spawn(async move {
            run_pipeline(me).await;
        });
        *self.handle.lock().expect("handle lock poisoned") = Some(handle);
        Ok(())
    }

    /// Signal the background task to stop and await its join.
    pub async fn stop(&self) -> Result<(), &'static str> {
        if !self.running.swap(false, Ordering::SeqCst) {
            return Err("data-plane pipeline is not running");
        }
        let handle = self.handle.lock().expect("handle lock poisoned").take();
        if let Some(h) = handle {
            let _ = h.await;
        }
        Ok(())
    }

    /// Inject a post-stamp tampered record so the integrity overlay
    /// fires. The buffer ends up with a record whose `crc` field does
    /// not match its samples.
    pub fn inject_tamper(&self) {
        let tick_id = self.ticks_emitted.fetch_add(1, Ordering::Relaxed) + 1_000_000_000;
        let mut bad = svdc_core::TickRecord::empty(tick_id, now_ns());
        bad.n_channels = 1;
        bad.set_flag(svdc_core::flags::COMPLETE);
        bad.samples[0] = svdc_core::Sample {
            value_q: 0xBAD_BAD,
            quality: 0,
            origin: svdc_core::SampleOrigin::Live.as_u8(),
            reserved: 0,
        };
        // Deliberately wrong CRC: stamp_crc() omitted on purpose.
        bad.crc = 0xDEAD_BEEF;
        self.buffer.push(bad);
        self.tamper_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Wipe the buffer and reset the counters. Pipeline may stay
    /// running; new ticks land in the now-empty buffer.
    pub fn reset(&self) {
        while self.buffer.pop().is_some() {}
        self.ticks_emitted.store(0, Ordering::Relaxed);
        self.tamper_count.store(0, Ordering::Relaxed);
        self.last_violations.store(0, Ordering::Relaxed);
        // Also drop the historian file so the next start writes a
        // fresh header. Best-effort.
        let _ = std::fs::remove_file(&self.historian_path);
    }
}

impl Default for DataPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON-friendly snapshot of the pipeline state. Rendered as both
/// JSON (for HTMX header polling) and HTML (for the status table).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DataPipelineSnapshot {
    /// Whether the background task is running.
    pub running: bool,
    /// Tick buffer length.
    pub buffer_len: usize,
    /// Tick buffer capacity (immutable for the demo).
    pub buffer_capacity: usize,
    /// Ticks emitted since the pipeline last started.
    pub ticks_emitted: u64,
    /// Operator-triggered tamper count.
    pub tamper_count: u64,
    /// `TickBuffer::verify_all()` result count.
    pub integrity_violations: usize,
    /// `tick_id` of every record currently in violation.
    pub violation_tick_ids: Vec<u64>,
    /// Newest record's `tick_id`, if any.
    pub latest_tick_id: Option<u64>,
    /// Newest record's `ts_utc_ns`, if any.
    pub latest_ts_ns: Option<u64>,
    /// Newest record's stamped `crc`, if any.
    pub latest_crc: Option<u32>,
    /// Newest record's channel-0 (Ia) value, if populated.
    pub latest_ch0_value: Option<i32>,
    /// Filesystem path of the historian CSV.
    pub historian_path: String,
    /// Whether the historian file exists on disk.
    pub historian_exists: bool,
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Synthetic IngressFrame producer + aligner + historian loop. Runs
/// inside the tokio runtime; exits when `pipe.running` becomes
/// `false`.
async fn run_pipeline(pipe: Arc<DataPipeline>) {
    // 80 SPC × 60 Hz reference; period_ns matches the publisher's
    // default sample rate so the binner math is consistent with the
    // unit-test data.
    let period_ns: u64 = 208_333;
    let mut aligner = Aligner::new(period_ns);
    let subscription = pipe.subscriber.subscribe(ChannelSet::all());
    let mut historian = match Historian::new(
        HistorianConfig::csv_at(pipe.historian_path.clone()),
        subscription,
    ) {
        Ok(h) => Some(h),
        Err(e) => {
            tracing::warn!(error = %e, "historian open failed; pipeline continues without CSV");
            None
        }
    };
    let mut ticker = tokio::time::interval(DEFAULT_TICK_INTERVAL);
    // The first tick fires immediately; subsequent ticks every
    // DEFAULT_TICK_INTERVAL. Skip delay so the first record appears
    // promptly.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut smp_cnt: u32 = 0;
    while pipe.running.load(Ordering::Relaxed) {
        ticker.tick().await;
        let frame = synth_frame(smp_cnt, period_ns, pipe.current_vendor());
        for asdu in &frame.samples {
            pipe.note_mu_observed(&asdu.sv_id);
        }
        for tick in aligner.process_frame(frame) {
            pipe.buffer.push(tick);
            pipe.ticks_emitted.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(h) = historian.as_mut() {
            if let Err(e) = h.tick() {
                tracing::warn!(error = %e, "historian tick failed; will retry next iteration");
            } else if smp_cnt % 20 == 0 {
                let _ = h.flush();
            }
        }
        smp_cnt = smp_cnt.wrapping_add(1);
    }
    if let Some(mut h) = historian {
        let _ = h.flush();
    }
}

fn synth_frame(
    smp_cnt: u32,
    period_ns: u64,
    vendor: Option<&'static ssiec_sv_publisher::VendorProfile>,
) -> IngressFrame {
    // Three-phase 60 Hz sinusoids in publisher-scale units. Same
    // amplitude conventions as `SampleData::NOMINAL_3PH` but
    // rotated through one cycle per second of demo time so the
    // operator sees the waveform breathe.
    let theta = (smp_cnt as f32) * core::f32::consts::TAU * 0.05_f32;
    let amp_v: f32 = 23_000.0;
    let amp_i: f32 = 5_000.0;
    let phase_b = -2.0 * core::f32::consts::PI / 3.0;
    let phase_c = 2.0 * core::f32::consts::PI / 3.0;
    let ia = (amp_i * theta.sin()).round() as i32;
    let ib = (amp_i * (theta + phase_b).sin()).round() as i32;
    let ic = (amp_i * (theta + phase_c).sin()).round() as i32;
    let in_ = -(ia + ib + ic);
    let va = (amp_v * theta.sin()).round() as i32;
    let vb = (amp_v * (theta + phase_b).sin()).round() as i32;
    let vc = (amp_v * (theta + phase_c).sin()).round() as i32;
    let vn = -(va + vb + vc);
    let samples = SampleData {
        channels: [
            ChannelSample::good(ia),
            ChannelSample::good(ib),
            ChannelSample::good(ic),
            ChannelSample::good(in_),
            ChannelSample::good(va),
            ChannelSample::good(vb),
            ChannelSample::good(vc),
            ChannelSample::good(vn),
        ],
    };
    let (sv_id, conf_rev, smp_rate) = match vendor {
        Some(v) => (
            v.svid_for("DATAPLANE_DEMO"),
            v.default_conf_rev,
            v.default_smp_rate_hz as u16,
        ),
        None => (
            "DATAPLANE_DEMO".into(),
            1,
            (1_000_000_000 / period_ns) as u16,
        ),
    };
    IngressFrame {
        timestamp: IngressTimestamp::from_unix_ns(now_ns()),
        samples: vec![DecodedSample {
            sv_id,
            smp_cnt: smp_cnt as u16,
            conf_rev,
            smp_synch: 2,
            smp_rate,
            samples,
        }],
    }
}

/// Process-wide handle. Lazily constructs one [`DataPipeline`] and
/// hands out cheap `Arc` clones.
pub fn global() -> Arc<DataPipeline> {
    static INSTANCE: OnceLock<Arc<DataPipeline>> = OnceLock::new();
    INSTANCE
        .get_or_init(|| Arc::new(DataPipeline::new()))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_pipeline_is_not_running_and_buffer_is_empty() {
        let p = DataPipeline::new();
        assert!(!p.is_running());
        assert_eq!(p.buffer.len(), 0);
        assert_eq!(p.buffer.capacity(), DEMO_BUFFER_CAPACITY);
        let snap = p.snapshot();
        assert!(!snap.running);
        assert_eq!(snap.buffer_len, 0);
        assert_eq!(snap.integrity_violations, 0);
        assert!(snap.latest_tick_id.is_none());
    }

    #[test]
    fn inject_tamper_pushes_record_with_mismatched_crc() {
        let p = DataPipeline::new();
        p.inject_tamper();
        assert_eq!(p.tamper_count(), 1);
        assert_eq!(p.buffer.len(), 1);
        let snap = p.snapshot();
        assert_eq!(snap.integrity_violations, 1);
        assert!(!snap.violation_tick_ids.is_empty());
    }

    #[test]
    fn reset_clears_buffer_counters_and_file() {
        let p = DataPipeline::new();
        p.inject_tamper();
        p.inject_tamper();
        assert!(!p.buffer.is_empty());
        p.reset();
        assert_eq!(p.buffer.len(), 0);
        assert_eq!(p.tamper_count(), 0);
        assert_eq!(p.ticks_emitted(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_then_stop_emits_at_least_one_tick() {
        let p = Arc::new(DataPipeline::new());
        // Override historian path to a per-test temp so concurrent
        // tests don't fight over the file. Need to drop the
        // pipeline and re-instantiate with a custom path; the
        // simplest path is to start once then stop quickly.
        p.start().unwrap();
        tokio::time::sleep(Duration::from_millis(220)).await;
        p.stop().await.unwrap();
        assert!(!p.is_running());
        // 220 ms / 50 ms = ~4 ticks expected. Allow ≥ 1 for slow CI.
        assert!(
            p.ticks_emitted() >= 1,
            "expected at least one tick, got {}",
            p.ticks_emitted()
        );
        assert!(!p.buffer.is_empty());
        let _ = std::fs::remove_file(&p.historian_path);
    }

    #[test]
    fn note_mu_observed_tracks_first_seen_last_seen_and_count() {
        let p = DataPipeline::new();
        assert_eq!(p.distinct_mu_count(), 0);

        p.note_mu_observed("SVDC_DEMO_PB_MU");
        let snap = p.seen_mus();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].sv_id, "SVDC_DEMO_PB_MU");
        assert_eq!(snap[0].frame_count, 1);
        assert_eq!(snap[0].first_seen_ms, snap[0].last_seen_ms);

        p.note_mu_observed("SVDC_DEMO_PB_MU");
        let snap = p.seen_mus();
        assert_eq!(snap[0].frame_count, 2);
        assert!(snap[0].last_seen_ms >= snap[0].first_seen_ms);
    }

    #[test]
    fn seen_mus_returns_one_entry_per_distinct_svid_sorted() {
        let p = DataPipeline::new();
        p.note_mu_observed("ZZ_LAST");
        p.note_mu_observed("AA_FIRST");
        p.note_mu_observed("MM_MID");
        p.note_mu_observed("AA_FIRST"); // duplicate — same entry
        assert_eq!(p.distinct_mu_count(), 3);
        let snap = p.seen_mus();
        let ids: Vec<&str> = snap.iter().map(|m| m.sv_id.as_str()).collect();
        assert_eq!(ids, vec!["AA_FIRST", "MM_MID", "ZZ_LAST"]);
    }

    #[test]
    fn clear_seen_mus_empties_the_map() {
        let p = DataPipeline::new();
        p.note_mu_observed("X");
        p.note_mu_observed("Y");
        assert_eq!(p.distinct_mu_count(), 2);
        p.clear_seen_mus();
        assert_eq!(p.distinct_mu_count(), 0);
    }

    #[test]
    fn set_vendor_resolves_preset_names_case_insensitively() {
        let p = DataPipeline::new();
        assert_eq!(p.selected_vendor_name(), None);
        p.set_vendor(Some("abb_relion_670")).unwrap();
        assert_eq!(p.selected_vendor_name(), Some("abb_relion_670"));
        p.set_vendor(Some("SEL_2240")).unwrap();
        assert_eq!(p.selected_vendor_name(), Some("sel_2240"));
        p.set_vendor(None).unwrap();
        assert_eq!(p.selected_vendor_name(), None);
        p.set_vendor(Some("none")).unwrap();
        assert_eq!(p.selected_vendor_name(), None);
    }

    #[test]
    fn set_vendor_rejects_unknown_preset() {
        let p = DataPipeline::new();
        assert!(p.set_vendor(Some("nonsense_vendor")).is_err());
        assert_eq!(p.selected_vendor_name(), None);
    }
}
