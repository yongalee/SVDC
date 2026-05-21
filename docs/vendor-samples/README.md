# Vendor sample artifacts

Synthetic IEC 61850 SCL (`.icd`) and per-channel calibration (`.csv`)
files matching the four vendor presets in
[`ssiec_sv_publisher::vendor`](../../crates/ssiec-sv-publisher/src/vendor.rs).

**These files are not real vendor deliverables.** They are
hand-authored examples that conform to publicly documented IEC
61850-6 SCL conventions for each vendor's family of merging units.
When the real units arrive from the manufacturer, the operator
replaces these samples with the vendor-supplied versions and re-runs
the field-connection checks in
[`docs/field-connection-guide.md`](../field-connection-guide.md).

| Vendor             | ICD sample                                                                 | Calibration CSV                                                            |
| ------------------ | -------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| ABB Relion / SAM600 | [`abb_relion_670.icd`](abb_relion_670.icd)                                | [`abb_relion_670.csv`](abb_relion_670.csv)                                |
| Siemens SIPROTEC 5 | [`siemens_siprotec_5.icd`](siemens_siprotec_5.icd)                        | [`siemens_siprotec_5.csv`](siemens_siprotec_5.csv)                        |
| GE Vernova UR      | [`ge_ur_series.icd`](ge_ur_series.icd)                                    | [`ge_ur_series.csv`](ge_ur_series.csv)                                    |
| SEL                | [`sel_2240.icd`](sel_2240.icd)                                            | [`sel_2240.csv`](sel_2240.csv)                                            |

## What's inside each ICD

Each sample SCL declares one IED with one logical device, one
`SampledValueControl` block, and one `<SMV><Address>` element. The
fields that matter on the wire are:

- `<SampledValueControl smvID="…" confRev="…" smpRate="…">`
- `<SMV><Address><P type="MAC-Address">…</P>` (destination multicast MAC)
- `<SMV><Address><P type="APPID">…</P>` (16-bit APPID)
- `<SMV><Address><P type="VLAN-ID">…</P>` (12-bit VID)
- `<SMV><Address><P type="VLAN-PRIORITY">…</P>` (3-bit PCP)
- `<IED manufacturer="…">` (drives vendor identification)

The values match the corresponding preset in `vendor.rs` so the
publisher emits an identical frame whether the operator passes
`--vendor abb_relion_670` or `--vendor-icd docs/vendor-samples/abb_relion_670.icd`.

## What's inside each CSV

Eight rows — Ia, Ib, Ic, In, Va, Vb, Vc, Vn — with the per-channel
`(gain, offset, unit_scale)` triple plus the CT/PT ratio and a
free-form notes column. The simulator prints the table to stdout
when `--calibration-csv <path>` is passed; the SVDC daemon imports
the same shape through `svdc-console::operational` (ADR-0007).

## Generating a PCAP that matches a vendor

```sh
# Pure preset (no ICD file required):
cargo run -p ssiec-sv-publisher -- pcap abb.pcap \
    --vendor abb_relion_670 --frames 200

# Preset layered with vendor ICD (real-world flow):
cargo run -p ssiec-sv-publisher -- pcap abb.pcap \
    --vendor abb_relion_670 \
    --vendor-icd docs/vendor-samples/abb_relion_670.icd \
    --calibration-csv docs/vendor-samples/abb_relion_670.csv \
    --frames 200
```

Open the resulting `abb.pcap` in Wireshark with display filter `sv`;
the frame dissection should match the corresponding ICD values
field-by-field.
