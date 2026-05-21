<?xml version="1.0" encoding="UTF-8"?>
<!-- 
  Substation Configuration Description (SCD) File for KEPCO Gwangju Substation
  Complies with IEC 61850 ASDU and Merging Unit specifications for a2SDP Node.
  NFR-10: English comments and tags.
-->
<SCL xmlns="http://www.iec.ch/61850/2003/SCL" version="2007" revision="B" release="4">
    <Header id="KEPCO-Gwangju-Substation-01" version="2.0" revision="B" toolID="SSIEC SCL Tool" nameStructure="IEDName">
        <History>
            <Hitem version="2.0" revision="B" when="2026-05-21T13:37:00" who="SSIEC Antigravity" what="Generated sample substation SCL for KEPCO Gwangju" />
        </History>
    </Header>
    <Substation name="KEPCO_Gwangju_Substation">
        <VoltageLevel name="VL154" desc="154kV Substation Bus">
            <Bay name="Bay_MU4">
                <desc>Bay for Merging Unit 4 Ingestion</desc>
                <ConductingEquipment name="QA4" type="CBR" desc="Circuit Breaker" />
                <ConductingEquipment name="QB4" type="DIS" desc="Disconnector" />
            </Bay>
        </VoltageLevel>
    </Substation>
    <Communication>
        <SubNetwork name="ProcessBus" type="802.3">
            <ConnectedAP iedName="MU_04_IED" apName="AP1">
                <Address>
                    <P type="MAC-Address">00-50-C2-88-99-A4</P>
                    <P type="APPID">4004</P>
                    <P type="VLAN-ID">004</P>
                    <P type="VLAN-PRIORITY">4</P>
                </Address>
            </ConnectedAP>
        </SubNetwork>
    </Communication>
    <IED name="MU_04_IED" desc="KEPCO Gwangju Substation Merging Unit IED">
        <AccessPoint name="AP1">
            <Server>
                <Authentication />
                <LDevice inst="MU4">
                    <LN0 lnClass="LLN0" inst="" lnType="LLN0_Type" />
                    <LN lnClass="TCTR" inst="4" lnType="TCTR_Type" />
                    <LN lnClass="TVTR" inst="4" lnType="TVTR_Type" />
                </LDevice>
            </Server>
        </AccessPoint>
    </IED>
</SCL>
