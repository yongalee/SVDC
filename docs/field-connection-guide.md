# SVDC field-connection guide

Operator playbook for taking a real vendor merging unit from "in the
box" to "TickRecords landing in the SVDC buffer". Each step has a
**simulator-side dry-run** so the same procedure works offline before
the physical unit is on the bench.

Audience: Prof. Meliopoulos and his lab team. Anyone who has run
`cargo build -p svdc-bin` once.

---

## 0. Before the unit arrives

You can dry-run every step below without any hardware by pointing
the simulator at one of the four vendor presets:

```sh
cargo run -p ssiec-sv-publisher -- pcap demo.pcap \
    --vendor abb_relion_670 --frames 200
```

`--vendor` accepts `abb_relion_670`, `siemens_siprotec_5`,
`ge_ur_series`, `sel_2240`, and `list` (prints the table).

---

## 1. Collect the vendor deliverable

Ask the vendor for:

1. **ICD file** (`.icd`, `.cid`, or `.scd`) — IEC 61850-6 SCL with
   the unit's `<SampledValueControl>` and `<SMV><Address>` blocks.
2. **Calibration sheet** — usually a PDF; transcribe into a CSV
   that matches
   [`docs/vendor-samples/<vendor>.csv`](vendor-samples/) (or get an
   Excel export from their engineering tool).
3. **(Optional) reference PCAP** — a Wireshark capture taken at
   the factory acceptance test. Useful as a known-good baseline.

For each of the four supported vendors there is a synthetic sample
artifact in [`docs/vendor-samples/`](vendor-samples/README.md) that
matches the simulator's preset bit-for-bit. Use those for dry runs.

---

## 2. Verify the simulator matches the unit's intended config

Run the publisher against the vendor's ICD:

```sh
cargo run -p ssiec-sv-publisher -- pcap before.pcap \
    --vendor abb_relion_670 \
    --vendor-icd /path/to/vendor.icd \
    --calibration-csv /path/to/vendor.csv \
    --frames 200
```

The publisher prints a one-screen summary:

```
vendor-icd: loaded fields {"default_appid", "multicast_mac", "vlan",
            "svid_template", "default_smp_rate_hz", "default_conf_rev"}
            from /path/to/vendor.icd (manufacturer = Some("ABB"))
ssiec-sv-publisher summary:
  frame bytes : 134
  dst MAC     : 01:0C:CD:04:00:01
  src MAC     : 00:21:C1:00:00:01
  APPID       : 0x4000
  VLAN (802.1Q): PCP=4, VID=100 (0x064), DEI=0
  svID        : AA1J1Q01A1MU01/LLN0$MX$Phsmeas9$svID
  sample rate : 4800 Hz
  fundamental : 60.000 Hz
  vendor      : abb_relion_670 — ABB Relion 670 / SAM600.
  channels    : Ia Ib Ic In Va Vb Vc Vn
```

Open `before.pcap` in Wireshark. The frame dissection must show
**every field from the summary**:

- destination MAC matches `<P type="MAC-Address">`
- source MAC matches the vendor OUI you expect for that family
- 0x8100 / TCI tag matches the VLAN PCP+VID
- APPID matches `<P type="APPID">`
- ASN.1 `svID` VisibleString matches `smvID`
- `smpRate` matches the SCL value

Any mismatch → fix the ICD or fix the preset before the unit arrives.

---

## 3. Physical setup

Minimum hardware:

- The merging unit, powered, configured against its own SCD
- A managed switch (1 GbE), or a directly-attached host with a
  multicast-capable NIC
- A PTP grandmaster (the GPS-disciplined boundary clock the a²SDP
  spec assumes; linuxptp on the SVDC machine when no dedicated
  GM is on the bench)
- The SVDC machine — must be on the same VLAN as the MU. On Linux,
  bring up the VLAN sub-interface:

  ```sh
  sudo ip link add link eth0 name eth0.100 type vlan id 100
  sudo ip link set dev eth0.100 up
  sudo ip maddr add 01:0c:cd:04:00:01 dev eth0.100
  ```

  (VID and multicast MAC come from the vendor's ICD, see §2.)

On Windows, install Npcap + use Wireshark to confirm the VLAN tag
shows in capture.

---

## 4. Capture from the real unit

With the unit publishing, take a short capture:

```sh
# Linux:
sudo tcpdump -i eth0.100 -w real.pcap ether proto 0x88ba

# Windows (PowerShell, Wireshark/Npcap installed):
& "C:\Program Files\Wireshark\dumpcap.exe" -i 1 -f "ether proto 0x88ba" -w real.pcap
```

Stop after ~10 seconds (≈ 48 000 frames at 4800 Hz). The file is
~8 MB; Wireshark opens it instantly.

---

## 5. Diff the real frame against the simulator

In Wireshark, filter `sv` and pick frame #1 from both `before.pcap`
(simulator) and `real.pcap` (live). Side-by-side, the following
must match:

| Field                              | Source                       |
| ---------------------------------- | ---------------------------- |
| Destination MAC                    | ICD `MAC-Address`            |
| EtherType + 802.1Q tag             | ICD `VLAN-ID` + `VLAN-PRIORITY` |
| APPID                              | ICD `APPID`                  |
| SV ID (ASN.1 VisibleString)        | ICD `smvID`                  |
| smpRate                            | ICD `smpRate`                |
| confRev                            | ICD `confRev`                |
| smpSynch                           | 2 (PTP-locked)               |

What may differ:

- **Source MAC** — vendor-OUI prefix matches; the last three octets
  vary per unit (serial number).
- **Sample values** — the simulator emits a synthetic sinusoid; the
  real unit reads CT/PT analog inputs. Compare the *shape*
  (3-phase sinusoid, correct frequency, expected amplitude after
  applying the calibration triple), not the literal bytes.

---

## 6. Feed the real PCAP into SVDC

Run the daemon and navigate to `/dataplane`. The synthetic
demo pipeline is still useful; for real-frame verification, follow
this flow once Phase 1's PCAP-replay subscriber lands (tracked
issue: WBS-2.1 `PcapSubscriber`):

```sh
# Phase 1+ command (not yet implemented):
cargo run -p svdc-bin -- --no-ui \
    --ingress-pcap real.pcap \
    --historian-out real.csv
```

For Phase 0, the verification path is:

1. Open `/dataplane` in the browser
2. Click **Start pipeline** — confirms the M1→M2→buffer→historian
   stack runs on this machine
3. Capture a Wireshark trace of the publisher emitting against
   `--vendor abb_relion_670` and compare against the real unit's
   capture from §4

The Phase 1 PCAP-replay subscriber adds the literal "feed real
capture into SVDC" leg.

---

## 7. Operator checklist

Before declaring "MU integrated":

- [ ] Vendor ICD file in `docs/vendor-samples/` (or operator-private
      equivalent)
- [ ] Calibration CSV in same location, matching the schema
- [ ] Simulator pcap (`before.pcap`) matches the ICD per §2
- [ ] Real-unit pcap (`real.pcap`) matches the ICD per §5
- [ ] `/dataplane` page running, tick buffer growing,
      `verify_all() = 0`
- [ ] `/api/mgmt/health` returns `{"status":"ok"}`
- [ ] `/api/mgmt/metrics` shows `svdc_tick_buffer_len > 0`
- [ ] Historian CSV opens in pandas / Excel cleanly

When all eight boxes are checked the SVDC ingress is confirmed
against that vendor's wire-level convention. Repeat per vendor.

---

## 8. Troubleshooting matrix

| Symptom                            | Likely cause                          | Fix                                  |
| ---------------------------------- | ------------------------------------- | ------------------------------------ |
| `Wireshark` shows no SV frames     | NIC not in promiscuous mode           | `sudo ip link set eth0 promisc on`   |
| Frames captured, no VLAN tag       | VLAN sub-interface not configured     | See §3                               |
| SVDC ingress refuses frame         | EtherType 0x88BA + 0x8100 tag missing | Confirm `--vendor` has VLAN set      |
| svID literal does not match        | Simulator preset != real unit         | Pass `--vendor-icd <vendor.icd>`     |
| Sample values off by factor of 10  | Unit_scale wrong in CSV               | Re-check calibration sheet           |
| `/health` reports `degraded`       | A record's CRC does not verify        | See ADR-0012 §5; usually a tamper   |
| Tick buffer length stuck at 0      | Pipeline not started in `/dataplane`  | Click Start                          |
| PCAP file empty                    | Forgot to run sudo with tcpdump       | Add `sudo`                           |
