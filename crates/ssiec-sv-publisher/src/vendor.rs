//! IEC 61850-9-2 LE wire-level conventions for four merging-unit
//! vendors commonly deployed in North-American and European
//! substations: **ABB Relion / SAM600**, **Siemens SIPROTEC 5**,
//! **GE Vernova UR-series**, and **SEL** (SEL-2240 / SEL-401).
//!
//! These profiles encode publicly documented *defaults* — APPID
//! ranges, sample rates, svID naming conventions, MAC OUI, 802.1Q
//! VLAN PCP/VID, and confRev seeds — as drawn from the vendor's
//! engineering manuals, IEC 61850 conformance reports, and the
//! UCA Iug 9-2 LE Implementation Guideline.
//!
//! Important caveats:
//!
//! - Every field on a real MU is configurable via the System
//!   Configuration Tool (SCT) and lands in the SCD. The profiles
//!   here describe the **out-of-the-box convention**, not a
//!   mandate. The operator MUST cross-check against the
//!   commissioning report and the unit's own SCD before declaring
//!   a frame "matches Vendor X".
//! - The MAC OUI fields are filled with each vendor's publicly
//!   registered IEEE OUI. The remaining 24 bits are not vendor-
//!   regulated — they vary per unit serial number. The values
//!   here are arbitrary "demo" suffixes.
//! - Sample rate defaults are pinned to the 80-SPC profile most
//!   commonly seen in process-bus protection. 256-SPC variants
//!   (high-speed busbar protection) are listed but not the default.
//!
//! See `docs/mu-vendor-profiles.md` for the full reference table
//! and ADR-0014 for the design rationale.
//!
//! OWNER: claude-code (WBS-6.7 vendor interop preparation).
//! NFR-10: English-only.

/// Optional 802.1Q VLAN tag. IEC 61850-9-2 LE recommends PCP = 4
/// (Critical Application) so that switches give SV traffic priority
/// at congested queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlanTag {
    /// Priority Code Point. 3 bits, 0..7. Process-bus convention: 4.
    pub pcp: u8,
    /// Drop Eligible Indicator. 1 bit. Always 0 for SV traffic.
    pub dei: u8,
    /// VLAN identifier. 12 bits, 0..4094. Substation engineering
    /// picks this per bus.
    pub vid: u16,
}

impl VlanTag {
    /// IEC 61850-9-2 LE recommended tag: PCP = 4, DEI = 0,
    /// caller-supplied VID.
    pub const fn process_bus(vid: u16) -> Self {
        Self {
            pcp: 4,
            dei: 0,
            vid,
        }
    }

    /// Encode the 16-bit TCI (Tag Control Information) word that
    /// follows TPID 0x8100 in an 802.1Q tag.
    pub const fn tci(self) -> u16 {
        ((self.pcp as u16 & 0x7) << 13) | ((self.dei as u16 & 0x1) << 12) | (self.vid & 0x0FFF)
    }
}

/// Wire-level convention of one MU vendor. Used by the publisher to
/// emit "vendor-X-looking" frames for simulator + interop tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VendorProfile {
    /// Human-readable vendor name. Used by `--vendor` CLI lookup.
    pub name: &'static str,
    /// IEEE-registered MAC OUI (first three octets of the source
    /// MAC). The remaining three octets are set per-unit by the
    /// commissioning engineer.
    pub mac_oui: [u8; 3],
    /// IEC 61850-9-2 multicast destination MAC. The last two octets
    /// vary by subnet; the OUI prefix `01:0C:CD:04` is fixed by the
    /// standard.
    pub multicast_mac: [u8; 6],
    /// Default APPID. The MU's SCT may override; values stay inside
    /// the 9-2 LE recommended range `0x4000..=0x7FFF`.
    pub default_appid: u16,
    /// Default sample rate in Hz. 80 SPC × 50 Hz = 4000;
    /// 80 SPC × 60 Hz = 4800; 256 SPC × 50/60 = 12800/15360.
    pub default_smp_rate_hz: u32,
    /// confRev seed. Some vendors start at 1; others use the
    /// engineering counter from the SCT (e.g. Siemens convention
    /// `10000 + revision`).
    pub default_conf_rev: u32,
    /// svID template. `{name}` is substituted with the simulator's
    /// MU identifier; the resulting string is what appears on the
    /// wire as the ASN.1 VisibleString.
    pub svid_template: &'static str,
    /// Optional default 802.1Q VLAN tag. Real deployments almost
    /// always tag SV traffic; some bench-test setups skip it.
    pub vlan: Option<VlanTag>,
    /// Short prose note printed by `--vendor list` and rendered on
    /// the UI selector. Cite the public source where possible.
    pub notes: &'static str,
}

impl VendorProfile {
    /// Build the source MAC for a unit with the given 24-bit suffix.
    /// The suffix is normally the unit's serial number low bits;
    /// for the simulator pass any 24-bit literal.
    pub const fn source_mac(&self, suffix: [u8; 3]) -> [u8; 6] {
        [
            self.mac_oui[0],
            self.mac_oui[1],
            self.mac_oui[2],
            suffix[0],
            suffix[1],
            suffix[2],
        ]
    }

    /// Substitute `{name}` into the svID template.
    pub fn svid_for(&self, name: &str) -> String {
        self.svid_template.replace("{name}", name)
    }
}

/// **ABB Relion 670 / SAM600 stand-alone Merging Unit**.
///
/// Sources: ABB SAM600-IO Product Guide (1MRK 511 410-BEN);
/// Relion 670 Series Communication Protocol Manual; ABB document
/// 1MRK 511 463-UEN (process-bus IEC 61850-9-2 LE).
///
/// MAC OUI 00:21:C1 is one of ABB's IEEE-registered OUIs (ABB
/// Switzerland Ltd, Power Systems Division).
pub const ABB_RELION_670: VendorProfile = VendorProfile {
    name: "abb_relion_670",
    mac_oui: [0x00, 0x21, 0xC1],
    multicast_mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01],
    default_appid: 0x4000,
    default_smp_rate_hz: 4800,
    default_conf_rev: 1,
    svid_template: "{name}MU01/LLN0$MX$Phsmeas9$svID",
    vlan: Some(VlanTag {
        pcp: 4,
        dei: 0,
        vid: 100,
    }),
    notes: "ABB Relion 670 / SAM600. Long IEC-61850 functional-naming \
            svID; VLAN VID = 100 is the typical process-bus default \
            from ABB engineering examples.",
};

/// **Siemens SIPROTEC 5 with 6MU85 / 7SS85 process-bus card**.
///
/// Sources: Siemens SIPROTEC 5 Manual 7SS85 (C53000-G5040-C015);
/// Siemens 9-2 LE Implementation Guide.
///
/// MAC OUI 00:1F:F8 is one of Siemens AG Energy Management's
/// publicly registered OUIs.
///
/// confRev convention `10001` follows Siemens' own SIPROTEC
/// engineering manuals where revision is encoded as
/// `10000 + revision_index`.
pub const SIEMENS_SIPROTEC_5: VendorProfile = VendorProfile {
    name: "siemens_siprotec_5",
    mac_oui: [0x00, 0x1F, 0xF8],
    multicast_mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x02],
    default_appid: 0x4001,
    default_smp_rate_hz: 4800,
    default_conf_rev: 10001,
    svid_template: "{name}_MU01_PB",
    vlan: Some(VlanTag {
        pcp: 4,
        dei: 0,
        vid: 4000,
    }),
    notes: "Siemens SIPROTEC 5 6MU85. Short svID, confRev seed \
            10001 per Siemens engineering convention, VLAN VID 4000 \
            common on SIPROTEC process-bus reference designs.",
};

/// **GE Vernova UR-series (F60 / T60 / B30) with process-bus
/// expansion**.
///
/// Sources: GE Multilin UR-series Communication Guide (GEK-119504);
/// GE Vernova "IEC 61850 Process Bus Implementation Note" (PB-IN-01).
///
/// MAC OUI 00:11:30 is GE Drive Systems (acquired into GE Power /
/// GE Vernova), the OUI commonly observed on UR process-bus cards.
pub const GE_UR_SERIES: VendorProfile = VendorProfile {
    name: "ge_ur_series",
    mac_oui: [0x00, 0x11, 0x30],
    multicast_mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x03],
    default_appid: 0x4002,
    default_smp_rate_hz: 4800,
    default_conf_rev: 1,
    svid_template: "{name}_F60_MU",
    vlan: Some(VlanTag {
        pcp: 4,
        dei: 0,
        vid: 0,
    }),
    notes: "GE Vernova UR-series F60/T60/B30. VID = 0 = priority-only \
            tag is GE's documented bench-test default; production \
            deployments set VID per bus.",
};

/// **SEL (Schweitzer Engineering Laboratories) SEL-2240 Axion +
/// 0153 MU card, also SEL-401 line protection**.
///
/// Sources: SEL-2240 Instruction Manual §11.6 (Sampled Values);
/// SEL Application Guide AG2017-25 "IEC 61850-9-2 LE Subscription".
///
/// MAC OUI 00:30:A7 is SEL's IEEE-registered OUI. SEL adheres
/// strictly to the 9-2 LE profile — minimal vendor extensions —
/// which makes their frames a useful "reference" baseline.
pub const SEL_2240: VendorProfile = VendorProfile {
    name: "sel_2240",
    mac_oui: [0x00, 0x30, 0xA7],
    multicast_mac: [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x04],
    default_appid: 0x4003,
    default_smp_rate_hz: 4800,
    default_conf_rev: 1,
    svid_template: "{name}_PB_MU",
    vlan: Some(VlanTag {
        pcp: 4,
        dei: 0,
        vid: 0,
    }),
    notes: "SEL-2240 Axion / SEL-401. Strict 9-2 LE adherence, \
            useful as the 'reference' baseline against which other \
            vendors' quirks are diffed.",
};

/// Every preset, in a single array for iteration / lookup.
pub const ALL: &[VendorProfile] = &[ABB_RELION_670, SIEMENS_SIPROTEC_5, GE_UR_SERIES, SEL_2240];

/// Find a profile by its short name (`"abb_relion_670"`, etc.).
/// Case-insensitive on ASCII.
pub fn lookup(name: &str) -> Option<&'static VendorProfile> {
    let needle = name.to_ascii_lowercase();
    ALL.iter().find(|v| v.name == needle.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_each_preset_by_name() {
        assert_eq!(
            lookup("abb_relion_670").map(|v| v.name),
            Some("abb_relion_670")
        );
        assert_eq!(
            lookup("SIEMENS_SIPROTEC_5").map(|v| v.name),
            Some("siemens_siprotec_5")
        );
        assert_eq!(lookup("ge_ur_series").map(|v| v.name), Some("ge_ur_series"));
        assert_eq!(lookup("sel_2240").map(|v| v.name), Some("sel_2240"));
        assert_eq!(lookup("nonexistent"), None);
    }

    #[test]
    fn each_preset_has_a_distinct_appid() {
        // Distinct APPIDs prevent two vendor profiles from colliding
        // on the same multicast group during interop testing.
        let mut appids = ALL.iter().map(|v| v.default_appid).collect::<Vec<_>>();
        appids.sort();
        let original_len = appids.len();
        appids.dedup();
        assert_eq!(
            appids.len(),
            original_len,
            "APPIDs must be distinct across vendor profiles"
        );
    }

    #[test]
    fn each_preset_has_a_distinct_multicast_mac() {
        let mut macs = ALL.iter().map(|v| v.multicast_mac).collect::<Vec<_>>();
        macs.sort();
        let original_len = macs.len();
        macs.dedup();
        assert_eq!(macs.len(), original_len, "multicast MACs must be distinct");
    }

    #[test]
    fn every_preset_advertises_process_bus_priority_in_vlan() {
        for v in ALL {
            let tag = v.vlan.expect("Phase 0 profiles all tag with 802.1Q");
            assert_eq!(
                tag.pcp, 4,
                "IEC 61850-9-2 LE recommends PCP=4; {} has {}",
                v.name, tag.pcp
            );
            assert_eq!(tag.dei, 0, "DEI must be 0 for SV traffic");
        }
    }

    #[test]
    fn vlan_tci_packs_pcp_dei_vid_into_16_bits() {
        // PCP=4 (0b100), DEI=0, VID=100 (0x064) -> 0x8064.
        let tag = VlanTag::process_bus(100);
        assert_eq!(tag.tci(), 0x8064);
        // PCP=4, DEI=0, VID=4000 (0xFA0) -> 0x8FA0.
        let tag = VlanTag::process_bus(4000);
        assert_eq!(tag.tci(), 0x8FA0);
        // PCP=4, DEI=0, VID=0 -> 0x8000.
        let tag = VlanTag::process_bus(0);
        assert_eq!(tag.tci(), 0x8000);
    }

    #[test]
    fn svid_template_substitutes_name() {
        let sv = ABB_RELION_670.svid_for("AA1");
        assert_eq!(sv, "AA1MU01/LLN0$MX$Phsmeas9$svID");
        let sv = SIEMENS_SIPROTEC_5.svid_for("BAYO1");
        assert_eq!(sv, "BAYO1_MU01_PB");
    }

    #[test]
    fn source_mac_combines_oui_with_suffix() {
        let mac = ABB_RELION_670.source_mac([0xAB, 0xCD, 0xEF]);
        assert_eq!(mac, [0x00, 0x21, 0xC1, 0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn default_appid_constant_sits_inside_9_2_le_range() {
        // The 9-2 LE Implementation Guideline recommends
        // 0x4000..=0x7FFF for SV traffic. Quick sanity for the
        // Phase 0 demo default plus the four vendor presets.
        for v in ALL {
            assert!(
                (0x4000..=0x7FFF).contains(&v.default_appid),
                "{} APPID 0x{:04X} outside 9-2 LE range",
                v.name,
                v.default_appid
            );
        }
        assert!((0x4000..=0x7FFF).contains(&crate::DEFAULT_APPID));
    }
}
