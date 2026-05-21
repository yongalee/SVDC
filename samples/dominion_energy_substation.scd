<?xml version="1.0" encoding="UTF-8"?>
<!-- 
  Substation Configuration Description (SCD) File for Dominion Energy Substation
  Complies with IEC 61850 ASDU and Merging Unit specifications for a2SDP Node.
  NFR-10: English comments and tags.
-->
<SCL xmlns="http://www.iec.ch/61850/2003/SCL" version="2007" revision="B" release="4">
    <Header id="Dominion-Energy-Substation-01" version="2.0" revision="B" toolID="SSIEC SCL Tool" nameStructure="IEDName">
        <History>
            <Hitem version="2.0" revision="B" when="2026-05-21T13:36:00" who="SSIEC Antigravity" what="Generated sample substation SCL for Dominion Energy" />
        </History>
    </Header>
    <Substation name="Dominion_Richmond_Substation">
        <VoltageLevel name="VL500" desc="500kV Extra High Voltage Bus">
            <Bay name="Bay_MU3">
                <desc>Bay for Merging Unit 3 Ingestion</desc>
                <ConductingEquipment name="QA3" type="CBR" desc="Circuit Breaker" />
                <ConductingEquipment name="QB3" type="DIS" desc="Disconnector" />
            </Bay>
        </VoltageLevel>
    </Substation>
    <Communication>
        <SubNetwork name="ProcessBus" type="802.3">
            <ConnectedAP iedName="MU_03_IED" apName="AP1">
                <Address>
                    <P type="MAC-Address">00-50-C2-88-99-A3</P>
                    <P type="APPID">4003</P>
                    <P type="VLAN-ID">003</P>
                    <P type="VLAN-PRIORITY">5</P>
                </Address>
            </ConnectedAP>
        </SubNetwork>
    </Communication>
    <IED name="MU_03_IED" desc="Dominion Energy Substation Merging Unit IED">
        <AccessPoint name="AP1">
            <Server>
                <Authentication />
                <LDevice inst="MU3">
                    <LN0 lnClass="LLN0" inst="" lnType="LLN0_Type" />
                    <LN lnClass="TCTR" inst="3" lnType="TCTR_Type" />
                    <LN lnClass="TVTR" inst="3" lnType="TVTR_Type" />
                </LDevice>
            </Server>
        </AccessPoint>
    </IED>
</SCL>
