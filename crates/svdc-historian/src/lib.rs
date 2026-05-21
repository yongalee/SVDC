//! `svdc-historian` — append-only CSV historian.
//!
//! First concrete northbound consumer of the [`svdc_subscribe`] API.
//! The historian opens a CSV file, writes a header row on first
//! creation, then on each [`Historian::tick`] call drains the
//! attached [`Subscription`] via `read_since()` and appends one row
//! per [`TickRecord`].
//!
//! Phase 0 scope:
//! - one CSV file, append mode, no rotation,
//! - one row per tick with `tick_id, ts_utc_ns, n_channels, flags_hex`
//!   followed by triples of `(value_q, quality, origin)` for each
//!   populated channel (Phase 0 = 8 channels per the aligner's
//!   collapse rule),
//! - `BufWriter`; caller decides when to [`Historian::flush`].
//!
//! Phase 4 follow-ups: time-based and size-based rotation, Parquet
//! sidecar, TimescaleDB sidecar (WBS-3.9 in the IP). The Phase 0
//! `HistorianConfig::rotation` field already carries the placeholder
//! enum so Phase 4 only changes the writer, not the surface.
//!
//! See `docs/decisions/0011-historian-design.md`.
//!
//! OWNER: claude-code (scaffold + ADR-0011). Phase 4 rotation /
//! Parquet / TimescaleDB go to Antigravity.
//! NFR-10: English-only.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use svdc_core::TickRecord;
use svdc_subscribe::Subscription;

/// File-format choice. Phase 0 ships only [`Format::Csv`]; Phase 4
/// adds `Parquet` and `TimescaleDB`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    /// Plain CSV with a header row. Easy to plot in pandas /
    /// spreadsheets; convenient for the demo cycle.
    Csv,
}

/// File rotation policy. Phase 0 ships only [`RotationPolicy::None`];
/// Phase 4 wires `BySize` and `Daily` for production deployments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationPolicy {
    /// Never rotate. File grows unbounded.
    None,
}

/// Historian construction parameters.
#[derive(Debug, Clone)]
pub struct HistorianConfig {
    /// Path to the output file. Created if absent; appended to if
    /// present.
    pub path: PathBuf,
    /// Output format. Phase 0 supports CSV only.
    pub format: Format,
    /// Rotation policy. Phase 0 supports None only.
    pub rotation: RotationPolicy,
}

impl HistorianConfig {
    /// Quick-build for the common Phase 0 case: CSV at `path`, no
    /// rotation.
    pub fn csv_at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            format: Format::Csv,
            rotation: RotationPolicy::None,
        }
    }
}

/// Errors raised by the historian. Open / write failures bubble up;
/// per-record formatting cannot fail in Phase 0 (everything is
/// either an integer or a `:02X` flag word).
#[derive(Debug)]
pub enum HistorianError {
    /// I/O error opening or writing the historian file.
    Io(std::io::Error),
}

impl std::fmt::Display for HistorianError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HistorianError::Io(e) => write!(f, "historian I/O error: {e}"),
        }
    }
}

impl std::error::Error for HistorianError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HistorianError::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for HistorianError {
    fn from(e: std::io::Error) -> Self {
        HistorianError::Io(e)
    }
}

/// Append-only historian wrapping a buffered writer plus the
/// [`Subscription`] it drains.
pub struct Historian {
    writer: BufWriter<File>,
    subscription: Subscription,
    config: HistorianConfig,
    rows_written: u64,
}

impl Historian {
    /// Open the historian file and bind it to a subscription. Writes
    /// a CSV header row only if the file did not previously exist
    /// (so re-opening an existing file appends without duplicating
    /// the header).
    pub fn new(
        config: HistorianConfig,
        subscription: Subscription,
    ) -> Result<Self, HistorianError> {
        let pre_existing = config.path.exists();
        if let Some(parent) = config.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&config.path)?;
        let mut writer = BufWriter::new(file);
        if !pre_existing {
            write_csv_header(&mut writer)?;
            writer.flush()?;
        }
        Ok(Self {
            writer,
            subscription,
            config,
            rows_written: 0,
        })
    }

    /// Drain the subscription via `read_since()` and append one row
    /// per fresh record. Returns the number of rows written this
    /// call.
    pub fn tick(&mut self) -> Result<usize, HistorianError> {
        let fresh = self.subscription.read_since();
        for r in &fresh {
            write_csv_row(&mut self.writer, r)?;
        }
        self.rows_written += fresh.len() as u64;
        Ok(fresh.len())
    }

    /// Flush buffered output to disk.
    pub fn flush(&mut self) -> Result<(), HistorianError> {
        self.writer.flush()?;
        Ok(())
    }

    /// Total rows appended by this historian since construction
    /// (does not include rows written by a previous run of the
    /// daemon).
    pub fn rows_written(&self) -> u64 {
        self.rows_written
    }

    /// Path the historian is writing to.
    pub fn path(&self) -> &Path {
        &self.config.path
    }
}

fn write_csv_header<W: Write>(w: &mut W) -> std::io::Result<()> {
    write!(w, "tick_id,ts_utc_ns,n_channels,flags_hex")?;
    for i in 0..svdc_core::MAX_CHANNELS {
        write!(w, ",ch{i}_value,ch{i}_quality,ch{i}_origin")?;
    }
    writeln!(w)
}

fn write_csv_row<W: Write>(w: &mut W, r: &TickRecord) -> std::io::Result<()> {
    write!(
        w,
        "{},{},{},0x{:04X}",
        r.tick_id, r.ts_utc_ns, r.n_channels, r.flags
    )?;
    for s in r.samples.iter() {
        write!(w, ",{},{},{}", s.value_q, s.quality, s.origin)?;
    }
    writeln!(w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use svdc_aligner::TickBuffer;
    use svdc_core::{flags, Sample, SampleOrigin};
    use svdc_subscribe::{ChannelSet, InProcessSubscriber, Subscriber};

    fn unique_path(tag: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("svdc-historian-{tag}-{ts}.csv"))
    }

    fn loaded_buffer(ticks: &[TickRecord]) -> Arc<TickBuffer> {
        let b = Arc::new(TickBuffer::new(64));
        for t in ticks {
            b.push(t.clone());
        }
        b
    }

    fn live_record(tick_id: u64, ts: u64, ch0_value: i32) -> TickRecord {
        let mut r = TickRecord::empty(tick_id, ts);
        r.n_channels = 1;
        r.set_flag(flags::COMPLETE);
        r.samples[0] = Sample {
            value_q: ch0_value,
            quality: 0,
            origin: SampleOrigin::Live.as_u8(),
            reserved: 0,
        };
        r
    }

    #[test]
    fn fresh_file_starts_with_header_row() {
        let path = unique_path("header");
        let _ = std::fs::remove_file(&path);

        let buffer = Arc::new(TickBuffer::new(8));
        let subscriber = InProcessSubscriber::new(buffer);
        let subscription = subscriber.subscribe(ChannelSet::all());
        let mut h = Historian::new(HistorianConfig::csv_at(&path), subscription).unwrap();
        h.flush().unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        let first_line = body.lines().next().unwrap();
        assert!(first_line.starts_with("tick_id,ts_utc_ns,n_channels,flags_hex,"));
        // Header should reference every channel slot.
        assert!(first_line.contains("ch0_value"));
        assert!(first_line.contains(&format!("ch{}_origin", svdc_core::MAX_CHANNELS - 1)));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tick_writes_one_row_per_fresh_record() {
        let path = unique_path("rows");
        let _ = std::fs::remove_file(&path);

        let buffer = loaded_buffer(&[
            live_record(0, 1_000_000_000, 100),
            live_record(1, 1_000_208_333, 200),
            live_record(2, 1_000_416_666, 300),
        ]);
        let subscriber = InProcessSubscriber::new(buffer);
        let subscription = subscriber.subscribe(ChannelSet::all());

        let mut h = Historian::new(HistorianConfig::csv_at(&path), subscription).unwrap();
        let n = h.tick().unwrap();
        h.flush().unwrap();

        assert_eq!(n, 3);
        assert_eq!(h.rows_written(), 3);

        let body = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        // 1 header + 3 data rows.
        assert_eq!(lines.len(), 4);
        // Data rows start with the tick_id.
        assert!(lines[1].starts_with("0,1000000000,1,0x0001,100,0,1,"));
        assert!(lines[2].starts_with("1,1000208333,1,0x0001,200,0,1,"));
        assert!(lines[3].starts_with("2,1000416666,1,0x0001,300,0,1,"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn second_tick_returns_zero_when_no_new_records() {
        let path = unique_path("idempotent");
        let _ = std::fs::remove_file(&path);

        let buffer = loaded_buffer(&[live_record(0, 0, 1)]);
        let subscriber = InProcessSubscriber::new(buffer);
        let subscription = subscriber.subscribe(ChannelSet::all());

        let mut h = Historian::new(HistorianConfig::csv_at(&path), subscription).unwrap();
        assert_eq!(h.tick().unwrap(), 1);
        assert_eq!(h.tick().unwrap(), 0);
        h.flush().unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        // Header + 1 data row only.
        assert_eq!(body.lines().count(), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn re_opening_existing_file_appends_without_writing_header() {
        let path = unique_path("reopen");
        let _ = std::fs::remove_file(&path);

        // First run: write the header + 1 row.
        {
            let buf = loaded_buffer(&[live_record(0, 0, 1)]);
            let sub = InProcessSubscriber::new(buf);
            let s = sub.subscribe(ChannelSet::all());
            let mut h = Historian::new(HistorianConfig::csv_at(&path), s).unwrap();
            h.tick().unwrap();
            h.flush().unwrap();
        }
        let after_run_1 = std::fs::read_to_string(&path).unwrap();
        assert_eq!(after_run_1.lines().count(), 2);

        // Second run: append 2 more rows; header should NOT be re-written.
        {
            let buf = loaded_buffer(&[live_record(1, 10, 2), live_record(2, 20, 3)]);
            let sub = InProcessSubscriber::new(buf);
            let s = sub.subscribe(ChannelSet::all());
            let mut h = Historian::new(HistorianConfig::csv_at(&path), s).unwrap();
            h.tick().unwrap();
            h.flush().unwrap();
        }
        let after_run_2 = std::fs::read_to_string(&path).unwrap();
        // 1 header + 1 row + 2 rows = 4 lines; only ONE header row.
        assert_eq!(after_run_2.lines().count(), 4);
        let header_lines = after_run_2
            .lines()
            .filter(|l| l.starts_with("tick_id,"))
            .count();
        assert_eq!(header_lines, 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flags_are_serialised_as_4_digit_hex() {
        let path = unique_path("flagshex");
        let _ = std::fs::remove_file(&path);

        let mut r = TickRecord::empty(7, 0);
        r.n_channels = 0;
        r.set_flag(flags::COMPLETE);
        r.set_flag(flags::DEGRADED);
        // 0x0001 | 0x0008 = 0x0009.

        let buf = loaded_buffer(&[r]);
        let sub = InProcessSubscriber::new(buf);
        let s = sub.subscribe(ChannelSet::all());
        let mut h = Historian::new(HistorianConfig::csv_at(&path), s).unwrap();
        h.tick().unwrap();
        h.flush().unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        let data_row = body.lines().nth(1).unwrap();
        assert!(data_row.starts_with("7,0,0,0x0009,"));

        let _ = std::fs::remove_file(&path);
    }
}
