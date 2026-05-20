# SVDC Verification Plan and Test Manual

This document details the concrete verification procedures, automated scripts, and manual check sequences designed to validate each Work Breakdown Structure (WBS) major item of the **Sampled Value Data Concentrator (SVDC)**. Follow these instructions on every repository update to ensure zero-regression and architectural alignment.

---

## 1. WBS-1: Project Foundation & Code Standards

These checks verify the software environment sanity, coding conventions, toolchain parameters, and handoff protocols.

### Automated Checks
Run these commands locally to ensure the workspace conforms to base specifications before committing:
```bash
# 1. Verify exact Rust toolchain pin and format
cargo fmt --all -- --check

# 2. Run Clippy static analysis with strict warning escalation
cargo clippy --workspace -- -D warnings

# 3. Check for non-ASCII characters (Hangul) to enforce English-only NFR-10
powershell -File scripts/lint-english-only.ps1
```

### Handoff Protocol Checks
- Check the git commit history of edited files for the correct trailing marker:
  - `OWNER: claude-code` or `OWNER: antigravity`
- Ensure that the current branch starts with the authorized prefix (`antigravity/` or `claude/`) matching the WBS issue under development.

---

## 2. WBS-2: Core Data Plane (The Hot Path)

The core data plane is latency-critical and operates with **zero heap allocation** and **zero mutexes** on the hot path (Ingest → Time Aligner → Interpolation/Calibration → Circular Buffer write).

### Automated Unit and Integration Tests
Validate data structures, PTP-timestamp decoding, and buffer cursor math:
```bash
# Run all unit tests for core data-plane algorithms
cargo test --package svdc-core --lib

# Run binary-level packet parsing sanity checks
cargo test --package svdc-bin
```

### Hot-Path Heap Allocation Verification (Heaptrack)
To verify NFR-4 (No allocations on hot-path), compile in release mode and profile using `heaptrack` during a sustained ingest load:
```bash
# 1. Compile in release mode (retains debug symbols for profiling)
cargo build --release

# 2. Run heaptrack against the binary while feeding Sampled Value packets
heaptrack target/release/svdc

# 3. Analyze heaptrack output.
# PASS CRITERION: Zero allocation/deallocation calls after the initial configuration phase.
```

---

## 3. WBS-3: Northbound External Interfaces

Verify failure-isolation and data consistency across L0, L1, L2, and L3 northbound integration layers.

### L0: In-Process C ABI & Shared Memory
- **Goal:** Sub-ms latency transmission to Phasor Computation Module.
- **Verification Command:**
  ```bash
  # Compile C ABI test stub and run validation loop
  cargo test --package svdc-cabi
  ```
- **Manual Verification:** Check shared memory segment allocation on Linux:
  ```bash
  ipcs -m | grep svdc
  ```

### L1: OPC UA Server
- **Goal:** SCADA integration per IEC 61850 ↔ OPC UA mapping.
- **Verification Steps:**
  1. Launch SVDC OPC UA server: `target/debug/svdc --opcua`
  2. Open an OPC UA client like **UaExpert** or run `opcua-commander`:
     ```bash
     npx opcua-commander -e opc.tcp://localhost:4840
     ```
  3. Browse the AddressSpace to verify that the Sampled Value channels mapped under `L1` match the schema configured in the SCD/CID file.

### L2: MQTT Publisher
- **Goal:** Cloud and ML application fan-out.
- **Verification Steps:**
  1. Start a local MQTT broker (e.g., `mosquitto`):
     ```bash
     mosquitto -v
     ```
  2. Start SVDC with MQTT output enabled.
  3. Subscribe to the Sampled Value publishing topic using a client to verify the JSON payload format:
     ```bash
     mosquitto_sub -t "svdc/mu/+/sampled_value" -h localhost -p 1883
     ```
  4. **Expected Output:** JSON payload containing aligned phase voltages and currents with disciplined PTP timestamps.

### L3: TimescaleDB Sidecar
- **Goal:** Long-term archival and phasor replay.
- **Verification Steps:**
  1. Ensure TimescaleDB container is active:
     ```bash
     docker ps | grep timescale
     ```
  2. Connect to the database and query the sampled value records to check if data is persisting without packet drops:
     ```sql
     SELECT time_bucket('1 second', time) AS bucket, avg(voltage_a) 
     FROM sv_records 
     GROUP BY bucket 
     ORDER BY bucket DESC 
     LIMIT 10;
     ```

---

## 4. WBS-4: Configuration and Registry

Validates runtime configuration loading and dynamic calibration updates.

### SCD (System Configuration Description) Parser Check
- **Verification Command:**
  ```bash
  # Run CID/SCD loading stub tests
  cargo test --package svdc-core --lib config::tests::test_scd_parsing
  ```
- **Manual Validation:** Feed a malformed XML file to `svdc` and ensure that it rejects the file gracefully with structured diagnostic logging without crashing (panic protection check).

---

## 5. WBS-5: Observability

Validates telemetry export, health metrics, and structured logging.

### Health Status and Telemetry Check
- **Verification Command:**
  ```bash
  # Query the HTTP health endpoint
  curl http://localhost:8080/health
  ```
- **Expected Payload:**
  ```json
  {
    "status": "OK",
    "ptp_offset_ns": 12,
    "ptp_holdover_state": "LOCKED",
    "buffer_saturation_percentage": 14.5,
    "dropped_packets": 0
  }
  ```

---

## 6. WBS-6: Test Infrastructure (MU Simulator)

Checks the conformance-grade SV Publisher stub (`ssiec-sv-publisher`).

### Conformance SV Packet Verification
- **Verification Steps:**
  1. Launch the publisher stub:
     ```bash
     target/debug/ssiec-sv-publisher --rate 80 --interface eth0
     ```
  2. Capture outgoing traffic using `tshark` (Wireshark CLI) and verify packet format:
     ```bash
     tshark -i eth0 -Y "sv" -c 10 -T fields -e sv.smpCnt -e sv.timestamp
     ```
  3. **Expected Result:** Multi-cast IEC 61850-9-2 SV packets are emitted precisely spaced at 80 samples per cycle (Protection class) or 256 samples per cycle (Measurement class).

---

## 7. WBS-7: Validation & Soak Testing

Validates systemic performance, throughput, and memory bounds over time.

### Automated Micro-Benchmarks
Ensure hot path algorithms are free of CPU regressions:
```bash
cargo bench --workspace
```

### Memory Leak / Soak Test
- **Goal:** Run SVDC continuously under peak throughput for 24 hours.
- **Verification Sequence:**
  1. Execute soak daemon scripts:
     ```bash
     powershell -File scripts/run-soak-test.ps1 -DurationHours 24
     ```
  2. **Acceptance Criteria:** RSS memory utilization remains constant (flat memory profile); packet gap rate is strictly `< 0.01%` under optimal PTP discipline.

---

## 8. Continuous Integration & Automation Handoff

The background monitoring agent `scripts/monitor-and-test.ps1` runs on a 5-minute recurring cron schedule. Every time the remote origin receives a push:
1. It downloads the changes.
2. It executes **WBS-1** (Formatting, Clippy, English Lint).
3. It executes **WBS-2** (Data plane unit tests).
4. If successful, it compiles release-ready binaries into `/debug` and updates the repository state.

To manually trigger the entire validation sequence locally:
```bash
powershell -File scripts/monitor-and-test.ps1 -DryRun
```
