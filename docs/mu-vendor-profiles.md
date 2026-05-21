# MU vendor profiles — wire-level reference

Quick reference for the four vendor presets shipped in
[`ssiec_sv_publisher::vendor`](../crates/ssiec-sv-publisher/src/vendor.rs).
Design rationale lives in [ADR-0014](decisions/0014-vendor-profiles.md);
operator playbook in [field-connection-guide.md](field-connection-guide.md).

## Reference table

| Field                 | ABB Relion 670 / SAM600                            | Siemens SIPROTEC 5 (6MU85)              | GE Vernova UR (F60)              | SEL-2240 Axion / SEL-401         |
| --------------------- | -------------------------------------------------- | --------------------------------------- | -------------------------------- | -------------------------------- |
| Preset name           | `abb_relion_670`                                   | `siemens_siprotec_5`                    | `ge_ur_series`                   | `sel_2240`                       |
| MAC OUI               | `00:21:C1` (ABB Switzerland)                       | `00:1F:F8` (Siemens AG EM)              | `00:11:30` (GE Drive Systems)    | `00:30:A7` (SEL)                 |
| Multicast MAC         | `01:0C:CD:04:00:01`                                | `01:0C:CD:04:00:02`                     | `01:0C:CD:04:00:03`              | `01:0C:CD:04:00:04`              |
| Default APPID         | `0x4000`                                           | `0x4001`                                | `0x4002`                         | `0x4003`                         |
| Default smpRate (Hz)  | 4800 (80 SPC × 60 Hz)                              | 4800                                    | 4800                             | 4800                             |
| Default confRev       | 1                                                  | 10001                                   | 1                                | 1                                |
| svID template         | `{name}MU01/LLN0$MX$Phsmeas9$svID`                 | `{name}_MU01_PB`                        | `{name}_F60_MU`                  | `{name}_PB_MU`                   |
| VLAN PCP              | 4                                                  | 4                                       | 4                                | 4                                |
| VLAN VID              | 100 (`0x064`)                                      | 4000 (`0xFA0`)                          | 0 (priority-only tag)            | 0 (priority-only tag)            |
| Sample ICD            | [abb_relion_670.icd](vendor-samples/abb_relion_670.icd) | [siemens_siprotec_5.icd](vendor-samples/siemens_siprotec_5.icd) | [ge_ur_series.icd](vendor-samples/ge_ur_series.icd) | [sel_2240.icd](vendor-samples/sel_2240.icd) |
| Sample calibration CSV| [abb_relion_670.csv](vendor-samples/abb_relion_670.csv) | [siemens_siprotec_5.csv](vendor-samples/siemens_siprotec_5.csv) | [ge_ur_series.csv](vendor-samples/ge_ur_series.csv) | [sel_2240.csv](vendor-samples/sel_2240.csv) |

## Field caveats

- **MAC OUI**: each vendor has *multiple* registered OUIs (acquisitions,
  business units, product families). The values here are the ones
  commonly observed on protection-bus MUs. The real source MAC is
  unit-serial-number dependent and will not match exactly; the OUI
  prefix is what the operator confirms with the `manuf` lookup in
  Wireshark.

- **Multicast MAC last byte**: arbitrary in the simulator. Real
  installations pick the value during engineering — there is no
  vendor convention. The fact that the four presets use
  `…:00:01/02/03/04` is a simulator-side choice so that simultaneous
  sim runs don't collide on one multicast group.

- **APPID**: 9-2 LE recommends `0x4000..=0x7FFF`. All four presets
  sit in the low end; the same caveat as MAC last-byte applies —
  real installations choose per bus during engineering.

- **smpRate = 4800 Hz**: 80 SPC × 60 Hz. The 50 Hz NA-grid variant
  (4000 Hz) and the high-speed busbar-protection variant (256 SPC
  × 60 Hz = 15360 Hz) are deferred to a follow-up that adds
  `<preset>_50hz` and `<preset>_busbar_256_spc` variants.

- **svID template**: vendors differ here more than anywhere else.
  ABB uses the full IEC 61850 functional name. Siemens, GE, and
  SEL use shorter custom names. The template `{name}` placeholder
  is the simulator's hook for substituting the operator's MU
  identifier.

- **VLAN VID**: 0 is a legal value meaning "priority-only tag" —
  the tag exists for QoS, no VLAN segmentation. GE and SEL ship
  this way out of the box; ABB and Siemens default to a populated
  VID. All four vendors override at engineering time.

- **confRev convention**: Siemens uses the `10000 + revision_index`
  encoding documented in the SIPROTEC manuals. ABB / GE / SEL
  start at 1 and increment per dataset change.

## How to extend

Adding a fifth vendor:

1. Append the constant to `vendor.rs` matching the public docs.
2. Add it to `vendor::ALL`.
3. Drop a sample `.icd` + `.csv` under `docs/vendor-samples/` that
   parses cleanly with `vendor_loader::tests` (the sample assertion
   is currently in-test; consider promoting to a dedicated
   integration test).
4. Append a row to this table.
5. Cite the public source in the constant's doc comment and in
   the table footnote.

Variants of an existing vendor (e.g. ABB Relion 670 with 256 SPC
busbar profile) are best added as additional constants
(`ABB_RELION_670_BUSBAR_256`) rather than fields on the existing
profile — the on-wire shape differs enough that a flat constant is
clearer than a knob.
