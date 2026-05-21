//! IEC 61850 SCL (Substation Configuration Language) — minimal parser
//! and in-memory channel registry.
//!
//! Phase 0/4 scope per ADR-0006 (SSIEC default for spec-lock Q5):
//! parse just enough SCL to extract, per Merging Unit, its identifier,
//! Ethernet MAC, APPID, svID, sample rate, and the list of FCDA
//! entries that the SV stream carries. Phase 4+ will widen the parser
//! when the L1 OPC UA AddressSpace builder needs deeper SCL metadata.
//!
//! OWNER: claude-code (WBS-9.6a).
//! NFR-10: English-only.

pub mod registry;
pub mod sample;

use serde::{Deserialize, Serialize};

/// A single signal channel from an SCL DataSet's FCDA entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Fully qualified channel name (`<prefix><lnClass><lnInst>.<doName>.<fc>`),
    /// matching IEC 61850-7-4 reference style.
    pub name: String,
    /// Coarse classification used by the UI for grouping and tile colour.
    pub unit: ChannelUnit,
}

/// Coarse unit classification. Heuristic per `doName` prefix per IEC 61850-7-4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelUnit {
    /// Voltage (PhV.*, Vol.* etc.).
    Voltage,
    /// Current (A.*, PhA.* etc.).
    Current,
    /// Anything else.
    Other,
}

impl ChannelUnit {
    fn from_do_name(do_name: &str) -> Self {
        let lc = do_name.to_ascii_lowercase();
        if lc.starts_with("phv") || lc.contains("vol") || lc.starts_with('v') {
            ChannelUnit::Voltage
        } else if lc.starts_with('a') || lc.contains("amp") || lc.contains("cur") {
            ChannelUnit::Current
        } else {
            ChannelUnit::Other
        }
    }
}

/// A Merging Unit parsed from an SCL/SCD document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergingUnit {
    /// IED name (used as the MU id throughout the system).
    pub id: String,
    /// Ethernet MAC (multicast for 9-2 SV streams), 6 bytes.
    pub mac: [u8; 6],
    /// 9-2 LE APPID (16-bit, hex-encoded in SCL).
    pub appid: u16,
    /// `svID` published in each SV ASDU.
    pub sv_id: String,
    /// Sample rate per second (e.g. 4800 = 80 SPC × 60 Hz).
    pub smp_rate: u32,
    /// Ordered list of channels carried in this MU's SV stream.
    pub channels: Vec<Channel>,
}

/// Result of parsing an SCD document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScdDocument {
    /// All Merging Units found in the document.
    pub merging_units: Vec<MergingUnit>,
}

/// Reasons SCD parsing can fail.
#[derive(Debug)]
pub enum ScdError {
    /// XML parse error reported by the underlying parser.
    Xml(roxmltree::Error),
    /// SCL document was syntactically valid XML but missing a required
    /// element. The string identifies what was missing.
    Missing(&'static str),
    /// A required field (MAC, APPID, smpRate) was present but
    /// malformed (e.g. non-hex APPID, MAC with wrong byte count).
    Malformed {
        /// Name of the field that failed validation.
        field: &'static str,
        /// Raw value as it appeared in the SCD document.
        value: String,
    },
}

impl core::fmt::Display for ScdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScdError::Xml(e) => write!(f, "XML parse error: {e}"),
            ScdError::Missing(m) => write!(f, "missing required element: {m}"),
            ScdError::Malformed { field, value } => {
                write!(f, "malformed {field}: {value:?}")
            }
        }
    }
}

impl std::error::Error for ScdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScdError::Xml(e) => Some(e),
            _ => None,
        }
    }
}

impl From<roxmltree::Error> for ScdError {
    fn from(e: roxmltree::Error) -> Self {
        ScdError::Xml(e)
    }
}

/// Parse an SCL/SCD XML document and return the per-MU summary.
///
/// The parser is permissive about missing optional metadata but
/// strict about MAC/APPID byte structure: a Communication block with
/// an unparseable MAC fails the whole document rather than silently
/// producing wrong configuration.
pub fn parse_scd(xml: &str) -> Result<ScdDocument, ScdError> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();
    if !eq_local_name(root, "SCL") {
        return Err(ScdError::Missing("SCL root element"));
    }

    // 1. Gather IED -> (svID, smpRate, channels) from DataSet entries.
    let mut mus: Vec<MergingUnit> = Vec::new();

    for ied in root.children().filter(|n| eq_local_name(*n, "IED")) {
        let Some(ied_name) = ied.attribute("name") else {
            continue;
        };

        // Find SampledValueControl + DataSet inside any LN0.
        let mut sv_id = String::new();
        let mut smp_rate: u32 = 0;
        let mut dataset_name: Option<&str> = None;

        if let Some(n) = ied
            .descendants()
            .find(|n| eq_local_name(*n, "SampledValueControl"))
        {
            sv_id = n.attribute("smvID").unwrap_or("").to_string();
            smp_rate = n
                .attribute("smpRate")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            dataset_name = n.attribute("datSet");
        }

        if sv_id.is_empty() || smp_rate == 0 {
            // No SV publisher on this IED — skip rather than fail.
            continue;
        }

        let mut channels: Vec<Channel> = Vec::new();
        if let Some(ds_name) = dataset_name {
            if let Some(ds) = ied
                .descendants()
                .filter(|n| eq_local_name(*n, "DataSet"))
                .find(|n| n.attribute("name") == Some(ds_name))
            {
                for fcda in ds.children().filter(|n| eq_local_name(*n, "FCDA")) {
                    channels.push(parse_fcda(fcda));
                }
            }
        }

        mus.push(MergingUnit {
            id: ied_name.to_string(),
            mac: [0; 6],
            appid: 0,
            sv_id,
            smp_rate,
            channels,
        });
    }

    // 2. Walk Communication / SubNetwork / ConnectedAP for MAC + APPID.
    for cap in root
        .descendants()
        .filter(|n| eq_local_name(*n, "ConnectedAP"))
    {
        let Some(ied_name) = cap.attribute("iedName") else {
            continue;
        };
        let Some(mu) = mus.iter_mut().find(|m| m.id == ied_name) else {
            continue;
        };

        for p in cap.descendants().filter(|n| eq_local_name(*n, "P")) {
            let Some(p_type) = p.attribute("type") else {
                continue;
            };
            let value = p.text().unwrap_or("").trim();
            match p_type {
                "MAC-Address" => {
                    mu.mac = parse_mac(value)?;
                }
                "APPID" => {
                    let v = u16::from_str_radix(value, 16).map_err(|_| ScdError::Malformed {
                        field: "APPID",
                        value: value.to_string(),
                    })?;
                    mu.appid = v;
                }
                _ => {}
            }
        }
    }

    Ok(ScdDocument { merging_units: mus })
}

fn eq_local_name(n: roxmltree::Node<'_, '_>, name: &str) -> bool {
    n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name)
}

fn parse_fcda(n: roxmltree::Node<'_, '_>) -> Channel {
    let prefix = n.attribute("prefix").unwrap_or("");
    let ln_class = n.attribute("lnClass").unwrap_or("");
    let ln_inst = n.attribute("lnInst").unwrap_or("");
    let do_name = n.attribute("doName").unwrap_or("");
    let fc = n.attribute("fc").unwrap_or("");
    let name = format!("{prefix}{ln_class}{ln_inst}.{do_name}.{fc}");
    Channel {
        name,
        unit: ChannelUnit::from_do_name(do_name),
    }
}

fn parse_mac(s: &str) -> Result<[u8; 6], ScdError> {
    let cleaned: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.len() != 12 {
        return Err(ScdError::Malformed {
            field: "MAC-Address",
            value: s.to_string(),
        });
    }
    let mut out = [0u8; 6];
    for i in 0..6 {
        out[i] = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).map_err(|_| {
            ScdError::Malformed {
                field: "MAC-Address",
                value: s.to_string(),
            }
        })?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SCD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL">
  <IED name="MU-SSIEC-01">
    <AccessPoint name="AP1">
      <Server>
        <LDevice inst="LD0">
          <LN0 lnClass="LLN0" inst="" lnType="LLN0">
            <SampledValueControl name="MSVCB01" smvID="SVDC_DEMO_01"
                                 datSet="dsSV01" smpRate="4800" nofASDU="1"
                                 confRev="1" />
            <DataSet name="dsSV01">
              <FCDA ldInst="LD0" prefix="VPh" lnClass="MMXU" lnInst="1"
                    doName="PhV.phsA" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="VPh" lnClass="MMXU" lnInst="1"
                    doName="PhV.phsB" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="VPh" lnClass="MMXU" lnInst="1"
                    doName="PhV.phsC" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="VPh" lnClass="MMXU" lnInst="1"
                    doName="PhV.neut" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="IPh" lnClass="MMXU" lnInst="1"
                    doName="A.phsA" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="IPh" lnClass="MMXU" lnInst="1"
                    doName="A.phsB" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="IPh" lnClass="MMXU" lnInst="1"
                    doName="A.phsC" daName="instMag.i" fc="MX"/>
              <FCDA ldInst="LD0" prefix="IPh" lnClass="MMXU" lnInst="1"
                    doName="A.neut" daName="instMag.i" fc="MX"/>
            </DataSet>
          </LN0>
        </LDevice>
      </Server>
    </AccessPoint>
  </IED>
  <Communication>
    <SubNetwork name="SN1">
      <ConnectedAP iedName="MU-SSIEC-01" apName="AP1">
        <Address>
          <P type="MAC-Address">01-0C-CD-04-00-01</P>
          <P type="APPID">4000</P>
          <P type="VLAN-ID">000</P>
          <P type="VLAN-PRIORITY">4</P>
        </Address>
      </ConnectedAP>
    </SubNetwork>
  </Communication>
</SCL>
"#;

    #[test]
    fn parses_one_mu_with_eight_channels() {
        let doc = parse_scd(SAMPLE_SCD).expect("parse");
        assert_eq!(doc.merging_units.len(), 1);
        let mu = &doc.merging_units[0];
        assert_eq!(mu.id, "MU-SSIEC-01");
        assert_eq!(mu.sv_id, "SVDC_DEMO_01");
        assert_eq!(mu.smp_rate, 4800);
        assert_eq!(mu.mac, [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01]);
        assert_eq!(mu.appid, 0x4000);
        assert_eq!(mu.channels.len(), 8);
    }

    #[test]
    fn channel_units_classified_by_do_name() {
        let doc = parse_scd(SAMPLE_SCD).unwrap();
        let mu = &doc.merging_units[0];
        let voltages = mu
            .channels
            .iter()
            .filter(|c| c.unit == ChannelUnit::Voltage)
            .count();
        let currents = mu
            .channels
            .iter()
            .filter(|c| c.unit == ChannelUnit::Current)
            .count();
        assert_eq!(voltages, 4, "expected 4 voltage channels");
        assert_eq!(currents, 4, "expected 4 current channels");
    }

    #[test]
    fn channel_names_include_prefix_and_fc() {
        let doc = parse_scd(SAMPLE_SCD).unwrap();
        let names: Vec<&str> = doc.merging_units[0]
            .channels
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(names.iter().any(|n| n.contains("VPhMMXU1.PhV.phsA.MX")));
        assert!(names.iter().any(|n| n.contains("IPhMMXU1.A.neut.MX")));
    }

    #[test]
    fn rejects_malformed_xml_cleanly() {
        let r = parse_scd("<SCL><IED");
        assert!(matches!(r, Err(ScdError::Xml(_))));
    }

    #[test]
    fn rejects_wrong_root_element() {
        let r = parse_scd("<?xml version='1.0'?><NotSCL/>");
        assert!(matches!(r, Err(ScdError::Missing(_))));
    }

    #[test]
    fn rejects_short_mac_address() {
        let scd_with_bad_mac = SAMPLE_SCD.replace("01-0C-CD-04-00-01", "01-0C-CD-04-00");
        let r = parse_scd(&scd_with_bad_mac);
        assert!(matches!(
            r,
            Err(ScdError::Malformed {
                field: "MAC-Address",
                ..
            })
        ));
    }

    #[test]
    fn mu_with_no_sv_control_is_skipped() {
        let scd = r#"<?xml version="1.0"?>
<SCL>
  <IED name="MU-NO-SV">
    <AccessPoint name="AP1">
      <Server><LDevice inst="LD0"><LN0 lnClass="LLN0"></LN0></LDevice></Server>
    </AccessPoint>
  </IED>
</SCL>"#;
        let doc = parse_scd(scd).unwrap();
        assert!(doc.merging_units.is_empty());
    }
}
