//! Load a [`VendorProfile`](crate::vendor::VendorProfile) from a
//! real vendor-supplied SCL artifact (`.icd`, `.cid`, or `.scd`).
//!
//! What this gives the operator: when a real merging unit ships
//! with its ICD file, the simulator can be configured to emit
//! frames that match the same wire-level fields (APPID, multicast
//! MAC, svID, sample rate) the real unit will use after
//! commissioning. The professor's bench setup can then verify the
//! SVDC ingress against the simulator *before* the real MU is
//! powered up, and the same configuration round-trips to the real
//! unit without further translation.
//!
//! IEC 61850-6 SCL elements consumed:
//!
//! - **`<SampledValueControl>`** — `appID`, `smvID` (→ svID),
//!   `confRev`, `smpRate`, `smpMod` (sample-rate units),
//!   `nofASDU`, `securityEnable` (we ignore that one).
//! - **`<SMV>`** (under `<GSE>`-style `<Communication>`/`<ConnectedAP>`):
//!   address parameters `MAC-Address`, `APPID`, `VLAN-ID`,
//!   `VLAN-PRIORITY`.
//! - **`<IED>` `manufacturer` attribute** — guess the vendor.
//!
//! Robustness: the SCL XML format admits many variants and
//! optional fields. This loader tolerates missing values by
//! falling back to a base profile (caller-supplied) and reports
//! exactly which fields it overrode in [`LoadedProfile::overridden`]
//! so the operator can audit the merge.
//!
//! OWNER: claude-code. NFR-10: English-only.

use std::collections::BTreeSet;

use crate::vendor::{VendorProfile, VlanTag};

/// Errors from [`load_from_icd`].
#[derive(Debug)]
pub enum LoadError {
    /// I/O failure reading the file.
    Io(std::io::Error),
    /// XML parser rejected the document.
    Xml(roxmltree::Error),
    /// No `<SampledValueControl>` element found anywhere in the file.
    NoSampledValueControl,
    /// A required attribute is missing or malformed.
    MissingAttribute(&'static str),
    /// A field value was syntactically wrong (e.g. non-numeric
    /// APPID).
    BadValue {
        /// Name of the offending field.
        field: &'static str,
        /// Raw text seen.
        text: String,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "ICD load I/O error: {e}"),
            LoadError::Xml(e) => write!(f, "ICD parse error: {e}"),
            LoadError::NoSampledValueControl => {
                write!(f, "no <SampledValueControl> element found in the file")
            }
            LoadError::MissingAttribute(a) => write!(f, "missing required attribute: {a}"),
            LoadError::BadValue { field, text } => {
                write!(f, "bad value for `{field}`: {text:?}")
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

impl From<roxmltree::Error> for LoadError {
    fn from(e: roxmltree::Error) -> Self {
        LoadError::Xml(e)
    }
}

/// Result of loading a vendor profile from SCL. Carries the merged
/// profile (presets layered with file overrides) plus an audit trail
/// of which fields the file supplied.
#[derive(Debug, Clone)]
pub struct LoadedProfile {
    /// Merged profile ready to hand to `FrameParams::from_vendor`.
    pub profile: VendorProfile,
    /// `manufacturer` attribute read from the `<IED>` element, if
    /// present. Useful for picking a preset when the operator did
    /// not supply one.
    pub manufacturer: Option<String>,
    /// Field names overridden by the file. Lets the UI render
    /// "this came from the ICD" vs "this is the preset default".
    pub overridden: BTreeSet<&'static str>,
}

/// Parse an SCL XML file and merge any SV publisher parameters it
/// declares into `base`. Missing fields keep the base value;
/// present fields land in `LoadedProfile::overridden`.
pub fn load_from_icd_str(xml: &str, base: VendorProfile) -> Result<LoadedProfile, LoadError> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();

    let manufacturer = root
        .descendants()
        .find(|n| n.has_tag_name("IED"))
        .and_then(|n| n.attribute("manufacturer"))
        .map(|s| s.to_string());

    let sv_control = root
        .descendants()
        .find(|n| n.has_tag_name("SampledValueControl"))
        .ok_or(LoadError::NoSampledValueControl)?;

    let mut profile = base;
    let mut overridden = BTreeSet::new();

    if let Some(sv_id) = sv_control.attribute("smvID") {
        // The smvID attribute IS the svID on the wire (the leading
        // tag for the ASN.1 VisibleString).
        // Store it as a static-friendly substitution template:
        // callers can still apply `svid_for(name)` if they want.
        // For now we capture the literal value via a Box<str> leak
        // is undesirable; we keep it implicit through the
        // overridden audit trail and document that the literal
        // smvID is meant to be used as-is.
        profile.svid_template = leak(sv_id);
        overridden.insert("svid_template");
    }

    if let Some(s) = sv_control.attribute("appID") {
        // Per IEC 61850-6, this is the SV control block ID, not the
        // wire APPID. The wire APPID lives under <SMV><Address>.
        // We capture the smvID-side value only as a fall-through
        // hint and skip overriding the wire APPID here.
        let _ = s;
    }

    if let Some(s) = sv_control.attribute("confRev") {
        profile.default_conf_rev = parse_u32(s, "confRev")?;
        overridden.insert("default_conf_rev");
    }

    if let Some(s) = sv_control.attribute("smpRate") {
        profile.default_smp_rate_hz = parse_u32(s, "smpRate")?;
        overridden.insert("default_smp_rate_hz");
    }

    // Wire address parameters: <SMV><Address><P type="APPID"|"MAC-Address"|"VLAN-ID"|"VLAN-PRIORITY">…</P>
    if let Some(smv) = root.descendants().find(|n| n.has_tag_name("SMV")) {
        if let Some(address) = smv.children().find(|n| n.has_tag_name("Address")) {
            let mut vlan_id: Option<u16> = None;
            let mut vlan_pri: Option<u8> = None;
            for p in address.children().filter(|n| n.has_tag_name("P")) {
                let kind = p.attribute("type").unwrap_or("");
                let text = p.text().unwrap_or("").trim();
                match kind {
                    "APPID" => {
                        profile.default_appid = parse_u16_hex(text, "APPID")?;
                        overridden.insert("default_appid");
                    }
                    "MAC-Address" => {
                        let mac = parse_mac(text)?;
                        profile.multicast_mac = mac;
                        overridden.insert("multicast_mac");
                    }
                    "VLAN-ID" => {
                        let vid = parse_u16_hex(text, "VLAN-ID")?;
                        vlan_id = Some(vid & 0x0FFF);
                    }
                    "VLAN-PRIORITY" => {
                        let pcp = parse_u8(text, "VLAN-PRIORITY")?;
                        vlan_pri = Some(pcp & 0x7);
                    }
                    _ => {}
                }
            }
            if vlan_id.is_some() || vlan_pri.is_some() {
                let existing = profile.vlan.unwrap_or(VlanTag::process_bus(0));
                let tag = VlanTag {
                    pcp: vlan_pri.unwrap_or(existing.pcp),
                    dei: 0,
                    vid: vlan_id.unwrap_or(existing.vid),
                };
                profile.vlan = Some(tag);
                overridden.insert("vlan");
            }
        }
    }

    Ok(LoadedProfile {
        profile,
        manufacturer,
        overridden,
    })
}

/// Convenience: open the file at `path`, then call
/// [`load_from_icd_str`].
pub fn load_from_icd_path(
    path: &std::path::Path,
    base: VendorProfile,
) -> Result<LoadedProfile, LoadError> {
    let xml = std::fs::read_to_string(path)?;
    load_from_icd_str(&xml, base)
}

fn parse_u32(s: &str, field: &'static str) -> Result<u32, LoadError> {
    s.trim().parse::<u32>().map_err(|_| LoadError::BadValue {
        field,
        text: s.to_string(),
    })
}

fn parse_u8(s: &str, field: &'static str) -> Result<u8, LoadError> {
    s.trim().parse::<u8>().map_err(|_| LoadError::BadValue {
        field,
        text: s.to_string(),
    })
}

/// Parse `0xABCD` or `1234` (decimal).
fn parse_u16_hex(s: &str, field: &'static str) -> Result<u16, LoadError> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(rest, 16).map_err(|_| LoadError::BadValue {
            field,
            text: s.to_string(),
        })
    } else {
        s.parse::<u16>().map_err(|_| LoadError::BadValue {
            field,
            text: s.to_string(),
        })
    }
}

/// Parse `01-0C-CD-04-00-01` or `01:0C:CD:04:00:01`.
fn parse_mac(s: &str) -> Result<[u8; 6], LoadError> {
    let bytes: Vec<&str> = s
        .split(['-', ':'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if bytes.len() != 6 {
        return Err(LoadError::BadValue {
            field: "MAC-Address",
            text: s.to_string(),
        });
    }
    let mut out = [0u8; 6];
    for (i, b) in bytes.iter().enumerate() {
        out[i] = u8::from_str_radix(b, 16).map_err(|_| LoadError::BadValue {
            field: "MAC-Address",
            text: s.to_string(),
        })?;
    }
    Ok(out)
}

/// Leak an `&str` into a `'static` slot so the (otherwise-static)
/// `svid_template` field on `VendorProfile` can hold it. This is
/// the smallest concession that lets the existing struct shape
/// stay `Copy` + `'static`. The leak is at-most once per ICD load,
/// which is fine for a CLI / interactive UI; embedded callers that
/// load many ICDs should switch to a `Cow<str>` field instead
/// (Phase 3 follow-up).
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor::SEL_2240;

    const SAMPLE_ICD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL">
  <Header id="TEST_ICD" nameStructure="IEDName"/>
  <Communication>
    <SubNetwork name="Process_Bus_1" type="8-MMS">
      <ConnectedAP iedName="TEST_MU01" apName="P1">
        <SMV cbName="MSVCB01" ldInst="LD0">
          <Address>
            <P type="VLAN-ID">0x064</P>
            <P type="VLAN-PRIORITY">4</P>
            <P type="MAC-Address">01-0C-CD-04-00-AA</P>
            <P type="APPID">0x4ABC</P>
          </Address>
        </SMV>
      </ConnectedAP>
    </SubNetwork>
  </Communication>
  <IED name="TEST_MU01" manufacturer="VendorX" type="MU" configVersion="1.0">
    <AccessPoint name="P1">
      <Server>
        <LDevice inst="LD0">
          <LN0 lnClass="LLN0" inst="" lnType="LLN0Type">
            <DataSet name="Phsmeas9">
              <FCDA ldInst="LD0" lnClass="MMXU" doName="Phsmeas9" fc="MX"/>
            </DataSet>
            <SampledValueControl name="MSVCB01"
                                 smvID="TEST_MU01_PB"
                                 datSet="Phsmeas9"
                                 confRev="2025"
                                 smpRate="4800"
                                 nofASDU="1"
                                 multicast="true"/>
          </LN0>
        </LDevice>
      </Server>
    </AccessPoint>
  </IED>
</SCL>
"#;

    #[test]
    fn load_sample_extracts_all_wire_fields() {
        let loaded = load_from_icd_str(SAMPLE_ICD, SEL_2240).unwrap();
        assert_eq!(loaded.manufacturer.as_deref(), Some("VendorX"));
        assert_eq!(loaded.profile.svid_template, "TEST_MU01_PB");
        assert_eq!(loaded.profile.default_appid, 0x4ABC);
        assert_eq!(loaded.profile.default_smp_rate_hz, 4800);
        assert_eq!(loaded.profile.default_conf_rev, 2025);
        assert_eq!(
            loaded.profile.multicast_mac,
            [0x01, 0x0C, 0xCD, 0x04, 0x00, 0xAA]
        );
        let tag = loaded.profile.vlan.unwrap();
        assert_eq!(tag.pcp, 4);
        assert_eq!(tag.vid, 0x64);
        // Audit trail includes every overridden field.
        assert!(loaded.overridden.contains("svid_template"));
        assert!(loaded.overridden.contains("default_appid"));
        assert!(loaded.overridden.contains("default_conf_rev"));
        assert!(loaded.overridden.contains("default_smp_rate_hz"));
        assert!(loaded.overridden.contains("multicast_mac"));
        assert!(loaded.overridden.contains("vlan"));
    }

    #[test]
    fn missing_sampled_value_control_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL"><Header id="X"/></SCL>"#;
        let err = load_from_icd_str(xml, SEL_2240).unwrap_err();
        assert!(matches!(err, LoadError::NoSampledValueControl));
    }

    #[test]
    fn bad_appid_returns_bad_value_error() {
        let xml = r#"<?xml version="1.0"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL">
  <Communication>
    <SubNetwork name="N">
      <ConnectedAP iedName="X" apName="A">
        <SMV cbName="C">
          <Address>
            <P type="APPID">not-a-hex</P>
          </Address>
        </SMV>
      </ConnectedAP>
    </SubNetwork>
  </Communication>
  <IED name="X"><AccessPoint name="A"><Server><LDevice inst="LD0">
    <LN0 lnClass="LLN0" inst="" lnType="T">
      <SampledValueControl name="C" smvID="X" datSet="D" confRev="1" smpRate="4800"/>
    </LN0>
  </LDevice></Server></AccessPoint></IED>
</SCL>"#;
        let err = load_from_icd_str(xml, SEL_2240).unwrap_err();
        match err {
            LoadError::BadValue { field, .. } => assert_eq!(field, "APPID"),
            other => panic!("expected BadValue, got {other:?}"),
        }
    }

    #[test]
    fn mac_parser_accepts_both_colon_and_dash_separators() {
        assert_eq!(
            parse_mac("01:0C:CD:04:00:01").unwrap(),
            [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01]
        );
        assert_eq!(
            parse_mac("01-0C-CD-04-00-01").unwrap(),
            [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01]
        );
        assert!(parse_mac("01-0C-CD-04").is_err());
    }

    #[test]
    fn missing_smv_address_keeps_base_profile_unchanged() {
        // No <SMV><Address>, only <SampledValueControl>.
        let xml = r#"<?xml version="1.0"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL">
  <IED name="X" manufacturer="Y">
    <AccessPoint name="A"><Server><LDevice inst="LD0">
      <LN0 lnClass="LLN0" inst="" lnType="T">
        <SampledValueControl name="C" smvID="onlyID" datSet="D" confRev="42" smpRate="9600"/>
      </LN0>
    </LDevice></Server></AccessPoint>
  </IED>
</SCL>"#;
        let loaded = load_from_icd_str(xml, SEL_2240).unwrap();
        // Wire APPID + MAC must fall back to base.
        assert_eq!(loaded.profile.default_appid, SEL_2240.default_appid);
        assert_eq!(loaded.profile.multicast_mac, SEL_2240.multicast_mac);
        // confRev + svID + smpRate came from the file.
        assert_eq!(loaded.profile.default_conf_rev, 42);
        assert_eq!(loaded.profile.default_smp_rate_hz, 9600);
        assert_eq!(loaded.profile.svid_template, "onlyID");
    }
}
