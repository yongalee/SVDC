//! Built-in sample SCD for the demo / commissioning workflow.
//!
//! Exposed via `POST /api/config/scd/sample` so a fresh installation
//! can populate the channel registry with one click — no SCD file
//! needs to be uploaded by the operator.
//!
//! The sample matches the SSIEC reference deployment described in
//! the paper: one MU with 4 voltage + 4 current channels, 80 SPC × 60
//! Hz = 4800 Hz, multicast MAC in the 9-2 LE range, APPID 0x4000.
//!
//! OWNER: claude-code (WBS-9.6a extension).
//! NFR-10: English-only.

/// Built-in SCL document. Compiled into the binary via `include_str!`
/// from `assets-static/sample-scd.xml` once that file lands in M0;
/// embedded inline here for v0.0.1.
pub const SAMPLE_SCD_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SCL xmlns="http://www.iec.ch/61850/2003/SCL">
  <IED name="MU-SSIEC-DEMO">
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
      <ConnectedAP iedName="MU-SSIEC-DEMO" apName="AP1">
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scd::parse_scd;

    #[test]
    fn sample_scd_parses_into_one_mu_with_eight_channels() {
        let doc = parse_scd(SAMPLE_SCD_XML).expect("sample SCD must parse");
        assert_eq!(doc.merging_units.len(), 1);
        let mu = &doc.merging_units[0];
        assert_eq!(mu.id, "MU-SSIEC-DEMO");
        assert_eq!(mu.sv_id, "SVDC_DEMO_01");
        assert_eq!(mu.smp_rate, 4800);
        assert_eq!(mu.mac, [0x01, 0x0C, 0xCD, 0x04, 0x00, 0x01]);
        assert_eq!(mu.appid, 0x4000);
        assert_eq!(mu.channels.len(), 8);
    }
}
