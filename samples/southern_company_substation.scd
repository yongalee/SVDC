<?xml version="1.0" encoding="UTF-8"?>
<!-- 
  Substation Configuration Description (SCD) File for Southern Company Substation
  Complies with IEC 61850 ASDU and Merging Unit specifications for a2SDP Node.
  NFR-10: English comments and tags.
-->
<SCL xmlns="http://www.iec.ch/61850/2003/SCL" version="2007" revision="B" release="4">
    <Header id="Southern-Company-Substation-01" version="2.0" revision="B" toolID="SSIEC SCL Tool" nameStructure="IEDName">
        <History>
            <Hitem version="2.0" revision="B" when="2026-05-21T13:30:00" who="SSIEC Antigravity" what="Generated sample substation SCL for Southern Company" />
        </History>
    </Header>
    <Substation name="Southern_Atlanta_Substation">
        <VoltageLevel name="VL230" desc="230kV Transmission Bus">
            <Bay name="Bay_MU1">
                <desc>Bay for Merging Unit 1 Ingestion</desc>
                <ConductingEquipment name="QA1" type="CBR" desc="Circuit Breaker" />
                <ConductingEquipment name="QB1" type="DIS" desc="Disconnector" />
            </Bay>
        </VoltageLevel>
    </Substation>
    <Communication>
        <SubNetwork name="ProcessBus" type="802.3">
            <ConnectedAP iedName="MU_01_IED" apName="AP1">
                <Address>
                    <P type="MAC-Address">00-50-C2-88-99-A1</P>
                    <P type="APPID">4001</P>
                    <P type="VLAN-ID">001</P>
                    <P type="VLAN-PRIORITY">4</P>
                </Address>
            </ConnectedAP>
        </SubNetwork>
    </Communication>
    <IED name="MU_01_IED" desc="Southern Company Substation Merging Unit IED">
        <AccessPoint name="AP1">
            <Server>
                <Authentication />
                <LDevice inst="MU1">
                    <LN0 lnClass="LLN0" inst="" lnType="LLN0_Type" />
                    <LN lnClass="TCTR" inst="1" lnType="TCTR_Type" />
                    <LN lnClass="TVTR" inst="1" lnType="TVTR_Type" />
                </LDevice>
            </Server>
        </AccessPoint>
    </IED>
</SCL>
