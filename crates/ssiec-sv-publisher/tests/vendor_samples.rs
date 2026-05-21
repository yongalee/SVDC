//! Per-vendor regression: load each checked-in sample ICD and
//! assert that the resulting `VendorProfile` matches the preset
//! the simulator built in by hand. This is the drift detector
//! ADR-0014 §4 calls out — if the preset constants change without
//! the ICD changing in lockstep, this test fails.

use std::path::PathBuf;

use ssiec_sv_publisher::vendor::{ABB_RELION_670, GE_UR_SERIES, SEL_2240, SIEMENS_SIPROTEC_5};
use ssiec_sv_publisher::vendor_loader::load_from_icd_path;
use ssiec_sv_publisher::VendorProfile;

fn samples_dir() -> PathBuf {
    // Cargo runs integration tests from the crate root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("docs")
        .join("vendor-samples")
}

fn assert_sample_matches_preset(
    filename: &str,
    preset: VendorProfile,
    expected_manufacturer: &str,
) {
    let path = samples_dir().join(filename);
    let loaded = load_from_icd_path(&path, preset).unwrap_or_else(|e| {
        panic!("failed to load {}: {e}", path.display());
    });
    assert_eq!(
        loaded.profile.default_appid, preset.default_appid,
        "{filename}: APPID mismatch (preset {:#06X}, ICD {:#06X})",
        preset.default_appid, loaded.profile.default_appid
    );
    assert_eq!(
        loaded.profile.multicast_mac, preset.multicast_mac,
        "{filename}: multicast MAC mismatch"
    );
    assert_eq!(
        loaded.profile.default_smp_rate_hz, preset.default_smp_rate_hz,
        "{filename}: smpRate mismatch"
    );
    assert_eq!(
        loaded.profile.default_conf_rev, preset.default_conf_rev,
        "{filename}: confRev mismatch"
    );
    let preset_tag = preset.vlan.expect("preset has VLAN");
    let icd_tag = loaded.profile.vlan.expect("ICD has VLAN");
    assert_eq!(preset_tag.pcp, icd_tag.pcp, "{filename}: VLAN PCP mismatch");
    assert_eq!(preset_tag.vid, icd_tag.vid, "{filename}: VLAN VID mismatch");
    assert_eq!(
        loaded.manufacturer.as_deref(),
        Some(expected_manufacturer),
        "{filename}: <IED manufacturer=...>",
    );
}

#[test]
fn abb_sample_icd_matches_preset() {
    assert_sample_matches_preset("abb_relion_670.icd", ABB_RELION_670, "ABB");
}

#[test]
fn siemens_sample_icd_matches_preset() {
    assert_sample_matches_preset("siemens_siprotec_5.icd", SIEMENS_SIPROTEC_5, "Siemens");
}

#[test]
fn ge_sample_icd_matches_preset() {
    assert_sample_matches_preset("ge_ur_series.icd", GE_UR_SERIES, "GE Vernova");
}

#[test]
fn sel_sample_icd_matches_preset() {
    assert_sample_matches_preset(
        "sel_2240.icd",
        SEL_2240,
        "Schweitzer Engineering Laboratories",
    );
}

#[test]
fn all_four_calibration_csvs_parse_eight_rows_each() {
    use ssiec_sv_publisher::calibration_loader::load_csv_path;
    for filename in [
        "abb_relion_670.csv",
        "siemens_siprotec_5.csv",
        "ge_ur_series.csv",
        "sel_2240.csv",
    ] {
        let path = samples_dir().join(filename);
        let rows = load_csv_path(&path).unwrap_or_else(|e| panic!("{filename}: {e}"));
        assert_eq!(rows.len(), 8, "{filename}: expected 8 channels");
        // Channel ids 0..=7 each present exactly once.
        let mut ids: Vec<u16> = rows.iter().map(|r| r.channel_id).collect();
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7], "{filename}");
        // First four are current, next four are voltage (Ia Ib Ic In Va Vb Vc Vn).
        for r in rows.iter().take(4) {
            assert_eq!(
                r.quantity, "current",
                "{filename} ch{}: quantity",
                r.channel_id
            );
        }
        for r in rows.iter().skip(4).take(4) {
            assert_eq!(
                r.quantity, "voltage",
                "{filename} ch{}: quantity",
                r.channel_id
            );
        }
    }
}
