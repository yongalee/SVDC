//! Inline-SVG chart fragments used by /monitoring and /south/mus/:id.
//!
//! For Phase 0/3 the chart consumers are mock data sources; Phase 5
//! wires the daemon's HDR histogram and PTP/CB counters in. The
//! renderers here are pure (no I/O, no allocation tied to request
//! lifetime) so they are easy to unit-test and reuse.
//!
//! OWNER: claude-code (WBS-9.5a — latency histogram + percentiles).

use maud::{html, Markup};

/// Log-scale histogram of latency samples in nanoseconds.
///
/// Buckets are equally spaced on a `log10(ns)` axis between
/// `min_log10` and `max_log10`. For the SVDC's hot-path budget the
/// useful range is roughly 1 µs (`min_log10 = 3`) to 1 s
/// (`max_log10 = 9`), so 60 buckets is ~0.1 dB per bucket which is
/// enough resolution for the operator dashboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    /// log10 of the lower bound, in nanoseconds.
    pub min_log10: f32,
    /// log10 of the upper bound, in nanoseconds.
    pub max_log10: f32,
    /// Count per bucket. `buckets.len()` is the bucket count.
    pub buckets: Vec<u64>,
}

impl Histogram {
    /// Construct a fresh empty histogram with `n` buckets covering the
    /// given log10 range.
    pub fn new(min_log10: f32, max_log10: f32, n: usize) -> Self {
        assert!(n >= 1, "histogram needs at least 1 bucket");
        assert!(max_log10 > min_log10, "max must exceed min on log axis");
        Self {
            min_log10,
            max_log10,
            buckets: vec![0; n],
        }
    }

    /// Record one latency sample (nanoseconds). Samples outside the
    /// configured range clamp to the first/last bucket so the operator
    /// still sees that out-of-range samples exist.
    pub fn record_ns(&mut self, sample_ns: f64) {
        if sample_ns <= 0.0 || !sample_ns.is_finite() {
            return;
        }
        let log10 = (sample_ns.max(1.0)).log10() as f32;
        let span = self.max_log10 - self.min_log10;
        let mut idx =
            (((log10 - self.min_log10) / span) * self.buckets.len() as f32).floor() as i64;
        if idx < 0 {
            idx = 0;
        }
        if idx as usize >= self.buckets.len() {
            idx = self.buckets.len() as i64 - 1;
        }
        self.buckets[idx as usize] += 1;
    }

    /// Total sample count across all buckets.
    pub fn total(&self) -> u64 {
        self.buckets.iter().sum()
    }

    /// Bucket lower bound in nanoseconds.
    pub fn bound_ns(&self, idx: usize) -> f64 {
        let span = (self.max_log10 - self.min_log10) as f64;
        let frac = idx as f64 / self.buckets.len() as f64;
        10f64.powf(self.min_log10 as f64 + frac * span)
    }

    /// Approximate the `p` percentile in nanoseconds. `p` is in [0, 1].
    /// Linear interpolation within the containing bucket.
    pub fn percentile_ns(&self, p: f64) -> Option<f64> {
        let total = self.total();
        if total == 0 || !(0.0..=1.0).contains(&p) {
            return None;
        }
        let target = (p * total as f64).ceil() as u64;
        let mut cum: u64 = 0;
        for (i, &c) in self.buckets.iter().enumerate() {
            let next = cum + c;
            if next >= target.max(1) {
                let lo = self.bound_ns(i);
                let hi = self.bound_ns(i + 1);
                if c == 0 {
                    return Some(lo);
                }
                // Midpoint convention within the bucket: a single sample
                // lands at 0.5 of the bucket width on the log axis, so
                // p50 of one sample in a bucket is the bucket's log-midpoint.
                let within = ((target.saturating_sub(cum) as f64) - 0.5) / c as f64;
                let within = within.clamp(0.0, 1.0);
                // Geometric interpolation within the log-scaled bucket.
                let log_lo = lo.log10();
                let log_hi = hi.log10();
                let log_v = log_lo + within * (log_hi - log_lo);
                return Some(10f64.powf(log_v));
            }
            cum = next;
        }
        Some(self.bound_ns(self.buckets.len()))
    }

    /// Highest bucket count, used for SVG y-axis normalisation.
    pub fn max_bucket(&self) -> u64 {
        self.buckets.iter().copied().max().unwrap_or(0)
    }
}

/// Render the histogram as an inline SVG bar chart with percentile
/// markers. Width 1000 × height 240 (matches the waveform panels).
pub fn latency_histogram_svg(hist: &Histogram) -> Markup {
    const VIEW_W: f32 = 1000.0;
    const VIEW_H: f32 = 240.0;
    const PAD_BOTTOM: f32 = 28.0;
    const PAD_TOP: f32 = 12.0;
    let chart_h = VIEW_H - PAD_TOP - PAD_BOTTOM;
    let n = hist.buckets.len() as f32;
    let bar_w = VIEW_W / n;
    let max = hist.max_bucket().max(1) as f32;

    let bars: Vec<(f32, f32, f32, f32)> = hist
        .buckets
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let h = chart_h * (*c as f32) / max;
            let x = i as f32 * bar_w;
            let y = PAD_TOP + (chart_h - h);
            (x, y, bar_w.max(0.6), h)
        })
        .collect();

    let p50 = hist.percentile_ns(0.5);
    let p99 = hist.percentile_ns(0.99);
    let p999 = hist.percentile_ns(0.999);

    let x_for_ns = |ns: f64| -> f32 {
        let log10 = (ns.max(1.0)).log10() as f32;
        let span = hist.max_log10 - hist.min_log10;
        let frac = ((log10 - hist.min_log10) / span).clamp(0.0, 1.0);
        frac * VIEW_W
    };

    // Decade tick labels (1µs, 10µs, ..., 1s).
    let ticks: Vec<(f32, f32, String)> = (hist.min_log10 as i32..=hist.max_log10 as i32)
        .map(|d| {
            let ns = 10f64.powi(d) as f32;
            let x = x_for_ns(ns as f64);
            let label = format_ns_short(ns as f64);
            (x, ns, label)
        })
        .collect();

    html! {
        svg.histogram-svg
            viewBox=(format!("0 0 {VIEW_W} {VIEW_H}"))
            preserveAspectRatio="none"
            role="img"
            aria-label="Latency histogram in nanoseconds, log scale" {
            rect.bg x="0" y="0" width=(VIEW_W) height=(VIEW_H) {}
            // Decade grid lines and labels
            @for (x, _ns, label) in &ticks {
                line.grid x1=(x) y1=(PAD_TOP) x2=(x) y2=(VIEW_H - PAD_BOTTOM) {}
                text.tick x=(x) y=(VIEW_H - 10.0) text-anchor="middle" { (label) }
            }
            // Bars
            @for (x, y, w, h) in &bars {
                rect.hist-bar x=(x) y=(y) width=(w) height=(h) {}
            }
            // Percentile markers
            @if let Some(v) = p50 {
                (percentile_marker(x_for_ns(v), "p50", v))
            }
            @if let Some(v) = p99 {
                (percentile_marker(x_for_ns(v), "p99", v))
            }
            @if let Some(v) = p999 {
                (percentile_marker(x_for_ns(v), "p999", v))
            }
        }
    }
}

fn percentile_marker(x: f32, label: &str, ns: f64) -> Markup {
    let y_top = 12.0;
    let y_bot = 212.0;
    html! {
        g.percentile-marker .{ "pm-" (label) } {
            line x1=(x) y1=(y_top) x2=(x) y2=(y_bot) {}
            text x=(x + 4.0) y=(y_top + 12.0) {
                (label) " " (format_ns_short(ns))
            }
        }
    }
}

/// Format a nanosecond value with the most readable unit suffix.
fn format_ns_short(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{ns:.0} ns")
    } else if ns < 1_000_000.0 {
        format!("{:.1} µs", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.1} ms", ns / 1_000_000.0)
    } else {
        format!("{:.2} s", ns / 1_000_000_000.0)
    }
}

/// Render a small KPI strip — count / p50 / p99 / p999 as inline text
/// blocks, in the same row above the histogram.
pub fn histogram_kpis(hist: &Histogram) -> Markup {
    let p50 = hist
        .percentile_ns(0.5)
        .map(format_ns_short)
        .unwrap_or_else(|| "—".into());
    let p99 = hist
        .percentile_ns(0.99)
        .map(format_ns_short)
        .unwrap_or_else(|| "—".into());
    let p999 = hist
        .percentile_ns(0.999)
        .map(format_ns_short)
        .unwrap_or_else(|| "—".into());
    let total = hist.total();

    html! {
        div.kpi-strip {
            div.kpi { span.kpi-label { "samples" }       span.kpi-value.mono { (total) } }
            div.kpi { span.kpi-label { "p50" }           span.kpi-value.mono { (p50) } }
            div.kpi { span.kpi-label { "p99" }           span.kpi-value.mono { (p99) } }
            div.kpi { span.kpi-label.muted { "p99.9" }   span.kpi-value.mono { (p999) } }
        }
    }
}

/// Deterministic mock latency generator. Phase 0 stub — Phase 5 wires
/// the real daemon counters in.
///
/// Produces samples roughly distributed as log-normal with mean
/// `mean_ns` and a fat right tail. Pure function, no I/O, identical
/// output for identical seed.
pub fn mock_latency_samples(seed: u64, n: usize, mean_ns: f64) -> Vec<f64> {
    let mut rng = SplitMix64::new(seed);
    let mu = mean_ns.ln();
    let sigma: f64 = 0.7;
    (0..n)
        .map(|_| {
            let u1 = rng.next_f64_unit();
            let u2 = rng.next_f64_unit();
            // Box-Muller transform → standard normal.
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            (mu + sigma * z).exp()
        })
        .collect()
}

/// Build a populated histogram from a mock sample run. Convenience for
/// the route renderer.
pub fn mock_histogram(seed: u64, n: usize, mean_ns: f64) -> Histogram {
    let mut hist = Histogram::new(3.0, 9.0, 60);
    for s in mock_latency_samples(seed, n, mean_ns) {
        hist.record_ns(s);
    }
    hist
}

/// SplitMix64 PRNG — small, deterministic, used only for mock data
/// generation. Not cryptographically secure (and does not need to be).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    fn next_f64_unit(&mut self) -> f64 {
        // 53-bit mantissa-worth of entropy in [0, 1).
        let r = self.next_u64() >> 11;
        let f = (r as f64) * (1.0_f64 / ((1u64 << 53) as f64));
        // Clamp the lower end so ln(u) downstream is finite.
        f.max(1.0e-12)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_of_uniform_bucket_returns_geometric_midpoint() {
        // Single bucket [10, 100) with one sample → p50 is roughly the
        // geometric midpoint of the bucket (≈31.6).
        let mut h = Histogram::new(1.0, 2.0, 1);
        h.record_ns(20.0);
        let p = h.percentile_ns(0.5).unwrap();
        // Inside the single bucket, p50 → log-midpoint, ≈ sqrt(10*100).
        assert!(p > 25.0 && p < 40.0, "got {p}");
    }

    #[test]
    fn percentile_monotone_in_p() {
        // Build a known histogram: 100 samples at ~10µs, 10 at ~100µs,
        // 1 at ~10ms.
        let mut h = Histogram::new(3.0, 9.0, 60);
        for _ in 0..100 {
            h.record_ns(10_000.0);
        }
        for _ in 0..10 {
            h.record_ns(100_000.0);
        }
        h.record_ns(10_000_000.0);

        let p50 = h.percentile_ns(0.5).unwrap();
        let p90 = h.percentile_ns(0.9).unwrap();
        let p99 = h.percentile_ns(0.99).unwrap();

        assert!(p50 < p90, "p50 {p50} should be < p90 {p90}");
        assert!(p90 < p99, "p90 {p90} should be < p99 {p99}");
        // p50 should land somewhere in the 10µs cluster.
        assert!(p50 > 5_000.0 && p50 < 50_000.0, "p50 {p50}");
        // p99 must reach the ms cluster.
        assert!(p99 > 100_000.0, "p99 {p99}");
    }

    #[test]
    fn empty_histogram_has_no_percentile() {
        let h = Histogram::new(3.0, 9.0, 60);
        assert!(h.percentile_ns(0.5).is_none());
        assert_eq!(h.total(), 0);
        assert_eq!(h.max_bucket(), 0);
    }

    #[test]
    fn out_of_range_samples_clamp_into_extremes() {
        let mut h = Histogram::new(3.0, 9.0, 4);
        h.record_ns(0.5); // below min, clamps to bucket 0
        h.record_ns(1.0e15); // above max, clamps to last bucket
        assert_eq!(h.buckets[0], 1);
        assert_eq!(*h.buckets.last().unwrap(), 1);
        assert_eq!(h.total(), 2);
    }

    #[test]
    fn record_ignores_nan_and_negative() {
        let mut h = Histogram::new(3.0, 9.0, 4);
        h.record_ns(f64::NAN);
        h.record_ns(f64::NEG_INFINITY);
        h.record_ns(-10.0);
        assert_eq!(h.total(), 0);
    }

    #[test]
    fn mock_generator_is_deterministic() {
        let a = mock_latency_samples(42, 100, 50_000.0);
        let b = mock_latency_samples(42, 100, 50_000.0);
        assert_eq!(a, b);
        let c = mock_latency_samples(43, 100, 50_000.0);
        assert_ne!(a, c);
    }

    #[test]
    fn mock_histogram_produces_plausible_percentiles() {
        let h = mock_histogram(2026, 5_000, 50_000.0); // 50 µs mean
        let p50 = h.percentile_ns(0.5).unwrap();
        let p99 = h.percentile_ns(0.99).unwrap();
        // Log-normal with σ=0.7 → p50 near the mean of the underlying
        // normal in log space, i.e., ~ mean. Wide tolerance because
        // 5000 samples is finite.
        assert!(p50 > 10_000.0 && p50 < 200_000.0, "p50 {p50}");
        assert!(p99 > p50);
    }

    #[test]
    fn svg_renders_with_decade_ticks_and_bars() {
        let h = mock_histogram(1, 500, 50_000.0);
        let svg = latency_histogram_svg(&h).into_string();
        assert!(svg.contains("viewBox=\"0 0 1000 240\""));
        assert!(svg.contains("hist-bar"));
        // Decade ticks in the 1µs..1s range include "µs" and "ms" labels.
        assert!(
            svg.contains("µs") || svg.contains("ms"),
            "expected unit label in tick text"
        );
        assert!(svg.contains("pm-p50"));
    }

    #[test]
    fn kpi_strip_shows_sample_count_and_percentiles() {
        let h = mock_histogram(7, 200, 50_000.0);
        let s = histogram_kpis(&h).into_string();
        assert!(s.contains("samples"));
        assert!(s.contains("p50"));
        assert!(s.contains("p99"));
        assert!(s.contains("200"));
    }

    #[test]
    fn format_ns_short_picks_readable_unit() {
        assert_eq!(format_ns_short(500.0), "500 ns");
        assert_eq!(format_ns_short(1_500.0), "1.5 µs");
        assert_eq!(format_ns_short(2_500_000.0), "2.5 ms");
        assert_eq!(format_ns_short(3_500_000_000.0), "3.50 s");
    }
}
