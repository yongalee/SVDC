# ADR-0014: MU vendor profiles + artifact ingestion

- Status: Accepted
- Date: 2026-05-21
- Owner: claude-code
- Supersedes: —
- Superseded by: —
- Related: WBS-6.7 (vendor interop test plan, IP §9.2), ADR-0003
  (SV encoder), ADR-0007 (SCD vs operational separation)

## Context

The SVDC must interoperate with merging units (MUs) from multiple
vendors, and the Georgia Tech bench will eventually have **real**
units from ABB, Siemens, GE Vernova, and/or SEL on it. The Phase 0
publisher emits a single hardcoded `FrameParams::DEMO` frame; it
cannot mimic vendor-specific wire-level quirks (VLAN tag, source
MAC OUI, svID format, default sample rate), which means:

- We can't dry-run the integration before hardware arrives.
- We can't generate canonical PCAPs the operator can diff against
  a real capture during commissioning.
- The svdc-ingress regression suite has no "what does an ABB frame
  look like vs a Siemens frame" coverage.

A vendor will not deliver a Markdown document with these fields —
they ship an **ICD/SCD XML file** (IEC 61850-6 SCL) plus a
**calibration sheet** (PDF, or sometimes a CSV/XLSX export). The
simulator must accept those artifacts directly so the operator does
not have to translate them into Rust constants by hand.

## Decision

### 1. Four vendor presets seeded from public documentation

Constants in `ssiec_sv_publisher::vendor`:

- `ABB_RELION_670` — ABB Relion 670 / SAM600 stand-alone MU
- `SIEMENS_SIPROTEC_5` — SIPROTEC 5 6MU85 / 7SS85 process-bus card
- `GE_UR_SERIES` — GE Vernova UR-series (F60 / T60 / B30)
- `SEL_2240` — SEL-2240 Axion / SEL-401

Each `VendorProfile` carries:

- `mac_oui` — IEEE-registered OUI for that vendor's family
- `multicast_mac` — destination MAC inside the 9-2 multicast block
- `default_appid` — 16-bit APPID inside the 9-2 LE range
- `default_smp_rate_hz` — 4800 Hz (80 SPC × 60 Hz) for NA-grid
  presets; the 9-2 LE 50 Hz variant lands as a follow-up
- `default_conf_rev` — vendor convention (e.g. `10001` for Siemens)
- `svid_template` — vendor-specific ASN.1 VisibleString shape
- `vlan` — optional 802.1Q tag (PCP = 4 per the 9-2 LE
  Implementation Guideline, VID varies)
- `notes` — short prose, source citation

These are **starting points**, not a mandate. Every field is
overridable from the vendor's ICD (see §3 below).

### 2. 802.1Q VLAN tagging is now first-class

`FrameParams.vlan: Option<VlanTag>` controls whether the encoder
emits `[TPID 0x8100][TCI]` between the source MAC and the SV
EtherType. The decoder peeks for `0x8100` and skips the tag if
present. Existing untagged callers (`FrameParams::DEMO`) stay
backward compatible — the field is `Option<...>` defaulting to
`None`.

Why now: every real substation MU tags its SV traffic. Without
VLAN support the simulator cannot reproduce a real frame, and the
ingress decoder cannot consume a real capture either.

### 3. Vendor artifact loaders

Two new modules:

- `vendor_loader::load_from_icd_*` parses IEC 61850-6 SCL
  (`.icd` / `.cid` / `.scd`), extracts the `SampledValueControl`
  attributes (`smvID`, `confRev`, `smpRate`) plus the `<SMV><Address>`
  parameters (`MAC-Address`, `APPID`, `VLAN-ID`, `VLAN-PRIORITY`),
  and merges them into a `VendorProfile`. The result includes an
  audit trail (`LoadedProfile::overridden`) so the UI/CLI can
  distinguish "came from the file" from "preset default".
- `calibration_loader::load_csv_*` parses the per-channel CSV the
  operator transcribes from the vendor's calibration sheet. The
  CSV schema (header row, flexible column order, quoted-comma
  tolerance) is documented in the module's doc comment.

Both loaders are **stateless and pure-Rust**; they live inside the
publisher crate (rather than `svdc-console` where the SCD parser
already exists) because:

- The publisher needs them for the simulator path
  (`--vendor-icd`, `--calibration-csv`)
- The ingress crate can reuse `vendor_loader` to recognise frames
  from a specific vendor without depending on `svdc-console`
- `svdc-console`'s existing SCD parser solves a different problem
  (channel registry indexing) and uses a different SCL subset

The two SCD parsers will likely converge in Phase 2; ADR-0007's
separation between SCD-derived and operator-tunable state still
holds.

### 4. Sample artifacts in `docs/vendor-samples/`

Four `.icd` + four `.csv` files, hand-authored to match the four
presets bit-for-bit. They serve three purposes:

1. **Worked examples** — show what the operator should expect a
   real vendor delivery to look like.
2. **Regression fixtures** — `vendor_loader::tests` parses the
   sample ICDs and asserts the resulting `VendorProfile` matches
   the corresponding preset.
3. **Drift detection** — if a vendor profile constant changes
   without the sample ICD changing in lockstep, CI fails.

They are **synthetic**; the README in that directory explains
they are not real vendor deliverables.

### 5. CLI surface

`ssiec-sv-publisher` gains three flags on `pcap` and `udp`:

```
--vendor <preset>            # apply a preset (preset names per §1)
--vendor-icd <path>          # load profile from SCL file
--calibration-csv <path>     # print calibration table
```

`--vendor list` prints the table and exits, so an operator with no
shell history can discover the supported presets.

When both `--vendor` and `--vendor-icd` are provided, the preset
loads first and the ICD overlays. Missing ICD fields keep preset
defaults; the publisher prints which fields the file overrode.

### 6. Field-connection guide as a checked-in document

`docs/field-connection-guide.md` is the operator playbook from
"vendor delivers MU" to "TickRecords land in SVDC". It cites every
artifact (ICD, CSV, simulator output, Wireshark capture) and
provides a §7 commissioning checklist. The guide is intentionally
versioned with the code so Phase 1's PCAP-replay subscriber update
lands the doc change in the same PR.

## Consequences

- The integration story is now repeatable: same six commands work
  for the simulator and (with the ICD substituted) for the real
  MU. The professor's first hands-on session with a real unit
  reuses the dry-run he ran the day before.
- The svdc-ingress regression suite gains an obvious extension
  point — Phase 1 can generate a canonical PCAP per vendor preset
  and assert decoder output is identical across all four.
- The publisher's `MAX_FRAME_BYTES` grew (256 → 320) to absorb the
  4-byte VLAN tag plus longer svID strings ABB uses. This is a
  fixed-stack allocation change only; no heap allocation per frame.
- The PCAP files generated by the simulator now look "real":
  Wireshark recognises the 802.1Q tag, the source MAC OUI maps
  to the right vendor in `manuf` lookups, and the svID matches
  the ICD literally.
- The vendor-OUI fields are filled with each company's publicly
  registered IEEE OUI. The simulator's source MAC is therefore
  **not** a private-range MAC (the previous Phase 0 default).
  Operator must be aware that the simulator's MAC will appear
  identical to a real-unit MAC on the wire from `manuf` lookups —
  it is **not** identical at the bit level (the last three octets
  are simulator-controlled).

## Out of scope

- Real `PcapSubscriber` for ingest replay (Phase 1, WBS-2.1).
- 50 Hz / 256 SPC variants (a follow-up that adds `_50hz` and
  `_busbar_256_spc` profile variants).
- UI vendor selector on `/dataplane` (a follow-up to this PR).
- Authentication / signed SCL (Phase 5, security plan).
- Vendor-specific extensions beyond the wire fields enumerated
  (e.g. ABB's proprietary `ed2.1` extensions, Siemens' SIPROTEC
  configuration backplane). These are vendor-engineering surface,
  not on-wire surface; the SVDC subscribes only to the standard
  9-2 LE frame.

## References

- IEC 61850-6 SCL schema (2007 ed. B rev. 4)
- IEC 61850-9-2 LE Implementation Guideline (UCA Iug)
- ABB SAM600-IO Product Guide (1MRK 511 410-BEN)
- Siemens SIPROTEC 5 Manual 7SS85 (C53000-G5040-C015)
- GE Multilin UR-series Communication Guide (GEK-119504)
- SEL-2240 Instruction Manual §11.6 (Sampled Values)
- SEL Application Guide AG2017-25
