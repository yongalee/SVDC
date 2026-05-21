//! Load a per-channel calibration table from a vendor-supplied CSV.
//!
//! When a vendor delivers a merging unit, the commissioning packet
//! always includes a per-channel scaling table — CT/PT primary,
//! CT/PT secondary, applied gain, applied offset, phase-angle
//! correction. Some vendors ship this as PDF (operator types it
//! into a CSV manually); others provide a CSV/XLSX export from
//! their engineering tool. Either way the SVDC needs the
//! `(gain, offset, unit_scale)` triple per channel before it can
//! report engineering-unit values on the northbound side.
//!
//! Expected CSV schema (header row required, column order
//! flexible):
//!
//! ```csv
//! channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale,notes
//! 0,A,current,1500/5,1.0,0.0,0.001,"Phase A current"
//! 1,B,current,1500/5,1.0,0.0,0.001,"Phase B current"
//! 2,C,current,1500/5,1.0,0.0,0.001,"Phase C current"
//! 3,N,current,1500/5,1.0,0.0,0.001,"Neutral current"
//! 4,A,voltage,138000/115,1.0,0.0,0.01,"Phase A voltage"
//! 5,B,voltage,138000/115,1.0,0.0,0.01,
//! 6,C,voltage,138000/115,1.0,0.0,0.01,
//! 7,N,voltage,138000/115,1.0,0.0,0.01,
//! ```
//!
//! The loader is intentionally permissive about quoting and
//! whitespace — vendor exports from Excel often quote everything
//! including numeric columns. Cells are trimmed, surrounding
//! double-quotes are stripped. Comma-inside-quotes is honoured.
//!
//! OWNER: claude-code. NFR-10: English-only.

use std::collections::HashMap;
use std::path::Path;

/// One row of a calibration table.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationRow {
    /// Channel index into `TickRecord::samples` (0..63).
    pub channel_id: u16,
    /// Phase identity: `A` / `B` / `C` / `N` / `G`.
    pub phase: String,
    /// `current` or `voltage`.
    pub quantity: String,
    /// CT or PT ratio printed as `"primary/secondary"`. Stored
    /// verbatim; the SVDC does not derive the triple from this
    /// — the vendor's calibration sheet is the authority.
    pub ct_pt_ratio: String,
    /// Multiplicative gain.
    pub gain: f32,
    /// Additive offset (raw units, before unit_scale).
    pub offset: f32,
    /// Scale factor from raw integer to engineering units
    /// (e.g. 0.001 for "0.001 A per LSB" on a current channel).
    pub unit_scale: f32,
    /// Free-form operator notes from the spreadsheet.
    pub notes: String,
}

/// Errors from [`load_csv_str`] / [`load_csv_path`].
#[derive(Debug)]
pub enum LoadError {
    /// File I/O failure.
    Io(std::io::Error),
    /// CSV did not have a header row.
    NoHeader,
    /// Header is missing a required column.
    MissingColumn(&'static str),
    /// A row had fewer columns than the header.
    RowTooShort {
        /// 1-based row number (excluding the header).
        row: usize,
        /// Number of columns the row supplied.
        got: usize,
        /// Number of columns the header declared.
        expected: usize,
    },
    /// A numeric cell did not parse.
    BadCell {
        /// 1-based row number.
        row: usize,
        /// Column name from the header.
        column: &'static str,
        /// Raw text the cell carried.
        text: String,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "calibration CSV I/O error: {e}"),
            LoadError::NoHeader => write!(f, "calibration CSV is empty (no header row)"),
            LoadError::MissingColumn(c) => write!(f, "calibration CSV missing column: {c}"),
            LoadError::RowTooShort { row, got, expected } => write!(
                f,
                "calibration CSV row {row} has {got} columns; expected {expected}"
            ),
            LoadError::BadCell { row, column, text } => {
                write!(
                    f,
                    "calibration CSV row {row} column `{column}`: bad value {text:?}"
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        LoadError::Io(e)
    }
}

const REQUIRED: &[&str] = &[
    "channel_id",
    "phase",
    "quantity",
    "ct_pt_ratio",
    "gain",
    "offset",
    "unit_scale",
];

/// Parse a calibration CSV from an in-memory string. Returns the
/// rows in input order; `channel_id` collisions are *not* checked
/// here — the caller decides whether to error or last-write-wins.
pub fn load_csv_str(text: &str) -> Result<Vec<CalibrationRow>, LoadError> {
    let mut lines = text.lines();
    let header_line = loop {
        match lines.next() {
            Some(l) if l.trim().is_empty() => continue,
            Some(l) if l.trim().starts_with('#') => continue,
            Some(l) => break l,
            None => return Err(LoadError::NoHeader),
        }
    };
    let header = parse_row(header_line);
    let mut col_idx: HashMap<&str, usize> = HashMap::new();
    for (i, c) in header.iter().enumerate() {
        col_idx.insert(c.as_str(), i);
    }
    for required in REQUIRED {
        if !col_idx.contains_key(required) {
            return Err(LoadError::MissingColumn(required));
        }
    }
    let notes_idx = col_idx.get("notes").copied();

    let mut rows = Vec::new();
    for (row_num, line) in lines.enumerate() {
        let row_num = row_num + 1;
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let cells = parse_row(line);
        if cells.len() < header.len() {
            return Err(LoadError::RowTooShort {
                row: row_num,
                got: cells.len(),
                expected: header.len(),
            });
        }
        let channel_id = parse_u16(&cells[col_idx["channel_id"]], row_num, "channel_id")?;
        let gain = parse_f32(&cells[col_idx["gain"]], row_num, "gain")?;
        let offset = parse_f32(&cells[col_idx["offset"]], row_num, "offset")?;
        let unit_scale = parse_f32(&cells[col_idx["unit_scale"]], row_num, "unit_scale")?;
        if !gain.is_finite() || gain == 0.0 {
            return Err(LoadError::BadCell {
                row: row_num,
                column: "gain",
                text: cells[col_idx["gain"]].clone(),
            });
        }
        if !offset.is_finite() {
            return Err(LoadError::BadCell {
                row: row_num,
                column: "offset",
                text: cells[col_idx["offset"]].clone(),
            });
        }
        if !unit_scale.is_finite() || unit_scale == 0.0 {
            return Err(LoadError::BadCell {
                row: row_num,
                column: "unit_scale",
                text: cells[col_idx["unit_scale"]].clone(),
            });
        }
        let notes = notes_idx
            .and_then(|i| cells.get(i))
            .cloned()
            .unwrap_or_default();
        rows.push(CalibrationRow {
            channel_id,
            phase: cells[col_idx["phase"]].clone(),
            quantity: cells[col_idx["quantity"]].clone(),
            ct_pt_ratio: cells[col_idx["ct_pt_ratio"]].clone(),
            gain,
            offset,
            unit_scale,
            notes,
        });
    }
    Ok(rows)
}

/// Convenience: read `path` and call [`load_csv_str`].
pub fn load_csv_path(path: &Path) -> Result<Vec<CalibrationRow>, LoadError> {
    let text = std::fs::read_to_string(path)?;
    load_csv_str(&text)
}

fn parse_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match (ch, in_quotes) {
            ('"', _) => in_quotes = !in_quotes,
            (',', false) => {
                cells.push(current.trim().to_string());
                current = String::new();
            }
            (c, _) => current.push(c),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn parse_u16(s: &str, row: usize, column: &'static str) -> Result<u16, LoadError> {
    s.trim().parse::<u16>().map_err(|_| LoadError::BadCell {
        row,
        column,
        text: s.to_string(),
    })
}

fn parse_f32(s: &str, row: usize, column: &'static str) -> Result<f32, LoadError> {
    s.trim().parse::<f32>().map_err(|_| LoadError::BadCell {
        row,
        column,
        text: s.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSV: &str = "\
channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale,notes
0,A,current,1500/5,1.0,0.0,0.001,\"Phase A current\"
1,B,current,1500/5,1.0,0.0,0.001,\"Phase B current\"
4,A,voltage,138000/115,1.05,-50.0,0.01,\"primary -50 V offset per cal sheet\"
";

    #[test]
    fn happy_path_parses_three_rows_with_notes() {
        let rows = load_csv_str(SAMPLE_CSV).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].channel_id, 0);
        assert_eq!(rows[0].quantity, "current");
        assert!((rows[2].gain - 1.05).abs() < 1e-6);
        assert!((rows[2].offset - -50.0).abs() < 1e-6);
        assert!(rows[2].notes.contains("-50 V offset"));
    }

    #[test]
    fn header_in_any_column_order() {
        let csv = "\
notes,unit_scale,offset,gain,ct_pt_ratio,quantity,phase,channel_id
\"Phase A\",0.001,0.0,1.0,1500/5,current,A,0
";
        let rows = load_csv_str(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].channel_id, 0);
        assert!((rows[0].unit_scale - 0.001).abs() < 1e-6);
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = "channel_id,phase,quantity,ct_pt_ratio,gain,offset\n0,A,current,1/1,1.0,0.0\n";
        let err = load_csv_str(csv).unwrap_err();
        assert!(matches!(err, LoadError::MissingColumn("unit_scale")));
    }

    #[test]
    fn comments_and_blanks_are_skipped() {
        let csv = "\
# Vendor: ACME Co.
# Date: 2025-03-15

channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale
0,A,current,1/1,1.0,0.0,0.001
# mid-file comment
1,B,current,1/1,1.0,0.0,0.001
";
        let rows = load_csv_str(csv).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn zero_gain_is_rejected() {
        let csv = "channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale\n0,A,current,1/1,0.0,0.0,0.001\n";
        let err = load_csv_str(csv).unwrap_err();
        match err {
            LoadError::BadCell { column, .. } => assert_eq!(column, "gain"),
            other => panic!("expected BadCell, got {other:?}"),
        }
    }

    #[test]
    fn row_too_short_reports_indices() {
        let csv = "channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale\n0,A,current\n";
        let err = load_csv_str(csv).unwrap_err();
        match err {
            LoadError::RowTooShort { row, got, expected } => {
                assert_eq!(row, 1);
                assert_eq!(got, 3);
                assert_eq!(expected, 7);
            }
            other => panic!("expected RowTooShort, got {other:?}"),
        }
    }

    #[test]
    fn quoted_comma_inside_notes_is_honoured() {
        let csv = "\
channel_id,phase,quantity,ct_pt_ratio,gain,offset,unit_scale,notes
0,A,current,1/1,1.0,0.0,0.001,\"includes, commas, in the value\"
";
        let rows = load_csv_str(csv).unwrap();
        // The parser keeps commas inside quoted cells; only commas
        // outside quotes split fields.
        assert_eq!(rows[0].notes, "includes, commas, in the value");
    }
}
