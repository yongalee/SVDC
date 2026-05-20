# ADR-0002 — Embedded Web-based UI and Monitoring Dashboard

- **Status:** accepted
- **Date:** 2026-05-21
- **Deciders:** SSIEC SVDC team, Google Antigravity Agent
- **WBS Code:** WBS-1.7 (UI/UX Design and Monitoring)

## Context

The Sampled Value Data Concentrator (SVDC) is a high-performance, backend-focused Rust daemon. As a key substation local node, it must process IEC 61850-9-2 Sampled Values (SV) on a sub-millisecond hot path under strict real-time constraints (NFR-2, NFR-4). 

However, during deployment, academic presentation (to the professor), and daily operation, it is essential to have a highly visual, real-time monitoring interface. Operators and reviewers must be able to:
1. **Verify Southbound Devices:** Connect, ping, and check the stream status, MAC/IP, and live values of incoming Merging Units (MUs).
2. **Control Northbound Applications:** Monitor and dynamically toggle connections to the Phasor Module (C ABI/Shared Memory), SCADA (OPC UA), Cloud publish (MQTT), and Historian (TimescaleDB).
3. **Monitor System Telemetry:** View the real-time state of PTP synchronization (clock offset in nanoseconds), circular buffer saturation, latency metrics, and reconstructed 3-phase waveforms.

Without a UI, verifying these complex, high-speed interactions requires tedious manual inspection of log files, Wireshark captures, or direct database queries.

## Decision

We will introduce a lightweight, high-performance, fully embedded web-based UI into the SVDC.

```
┌────────────────────────────────────────────────────────────────────────┐
│                              SVDC Daemon                               │
│                                                                        │
│  ┌───────────────────────┐            ┌─────────────────────────────┐  │
│  │   Hot Path Ingest     │            │      Embedded Web Server    │  │
│  │   (Zero allocation)   │            │   (Axum / Tokio / Async)    │  │
│  └──────────┬────────────┘            └──────────────┬──────────────┘  │
│             │                                        │ Serves static   │
│             │ Real-time Telemetry                    │ UI assets via   │
│             ▼ (Lock-free RingBuffer)                 │ rust-embed      │
│  ┌───────────────────────┐                           ▼                 │
│  │ Circular Buffer / PTP │                  ┌─────────────────┐        │
│  └───────────────────────┘                  │    Web UI SPA   │◄───────┘
│                                             │ (HTML/CSS/JS)   │
│                                             └────────┬────────┘
│                                                      │
│                                                      ▼ (WebSocket)
│                                             Reconstructed Waveforms,
│                                             PTP state, MU Statuses
└────────────────────────────────────────────────────────────────────────┘
```

### 1. Architectural Guidelines

To prevent UI operations from interfering with the latency-critical hot path:
- **Strict Isolation:** The web server runs entirely on a separate background thread pool managed by `tokio`, completely decoupled from the lock-free data path.
- **Zero-Allocation Telemetry Sharing:** The data-plane communicates telemetry metrics (e.g., PTP offset, buffer saturation, active MU counts) to the web server using atomic counters and lock-free ring buffers (e.g., `crossbeam-channel` or `ringbuf`). No mutexes are held on the hot path.
- **Single-Binary Portability:** Static UI assets (HTML, CSS, JS, images) are compiled directly into the SVDC binary using `rust-embed`. The daemon remains self-contained with no external dependencies (like Node.js, Python, or Apache) needed for production deployment.

### 2. Frontend Technology Stack

To ensure instant loading, high reliability, and premium aesthetics:
- **Aesthetics (Glassmorphism & HSL Curated Palette):** Premium dark-theme using subtle borders, backing gradients, backdrop-filters (`blur()`), and HSL-based Tailwind-like styles. Vibrant accent colors indicate statuses (Green: Locked/Healthy, Orange: Degraded/Holdover, Red: Disconnected/Fault).
- **Structure:** Semantic HTML5 with unique IDs (`id="mu-card-..."`) for robust testing.
- **Logic:** Modern Vanilla JavaScript (ES6) with direct DOM manipulation for maximum speed.
- **Waveform Rendering:** HTML5 Canvas API or a lightweight charting library (e.g., `Chart.js` via CDN) to draw smooth real-time 3-phase sine waves of incoming sampled voltages and currents.

### 3. API & WebSockets Ingestion

The embedded server (running by default on port `8080` or configurable via `--ui-port`) exposes two integration layers:

#### A. WebSockets Telemetry (`/ws/telemetry`)
A real-time, sub-second (100ms interval) broadcast containing:
```json
{
  "timestamp": "2026-05-21T08:20:30.123456Z",
  "ptp": {
    "offset_ns": 12,
    "holdover_state": "LOCKED",
    "grandmaster_mac": "00:1B:21:BA:C1:14"
  },
  "buffer": {
    "saturation_percent": 14.5,
    "write_cursor": 10294,
    "read_cursor": 10280
  },
  "metrics": {
    "cpu_percent": 2.4,
    "memory_mb": 18.2,
    "latency_ns": 45000
  },
  "merging_units": [
    {
      "id": "MU-SSIEC-01",
      "ip": "192.168.1.101",
      "mac": "00:50:C2:88:99:A1",
      "status": "HEALTHY",
      "packet_rate_hz": 4000,
      "dropped_packets": 0,
      "phase_a_voltage": 220.1,
      "phase_b_voltage": 219.8,
      "phase_c_voltage": 220.4
    }
  ]
}
```

#### B. REST API Endpoints
- `GET /api/v1/status`: Retrieve general system statistics.
- `GET /api/v1/merging-units`: Fetch detailed configuration and state of all Merging Units.
- `POST /api/v1/merging-units/:id/ping`: Trigger an active ICMP or Layer-2 ping test.
- `GET /api/v1/northbound`: Fetch connection parameters for SCADA, MQTT, and TimescaleDB.
- `POST /api/v1/northbound/:adapter/toggle`: Dynamically enable or disable a northbound integration.

---

## UI Layout Design (The Wireframe)

The Web UI dashboard consists of a responsive three-column grid optimized for high-density information display:

```
┌────────────────────────────────────────────────────────────────────────┐
│ [★ SSIEC a²SDP]  SVDC MONITORING DASHBOARD         [SYS: Healthy (●)]  │
│ Grandmaster: LOCKED | PTP Offset: 12 ns | Buffer Saturation: 14.5%     │
├──────────────────────┬──────────────────────────┬──────────────────────┤
│ 🔌 Southbound (MUs)   │ 📈 Live Waveforms        │ 🚀 Northbound        │
│                      │ ┌──────────────────────┐ │                      │
│ ┌──────────────────┐ │ │   Reconstructed      │ │ ┌──────────────────┐ │
│ │ MU-SSIEC-01  (●) │ │ │   3-Phase Voltage    │ │ │ OPC UA Server (●)│ │
│ │ 192.168.1.101    │ │ │   (Va, Vb, Vc)       │ │ │ Port: 4840       │ │
│ │ Rate: 4000 Hz    │ │ └──────────────────────┘ │ │ [Disable Toggle] │ │
│ │ [Ping MU Button] │ │ ┌──────────────────────┐ │ └──────────────────┘ │
│ └──────────────────┘ │ │   Reconstructed      │ │ ┌──────────────────┐ │
│ ┌──────────────────┐ │ │   3-Phase Current    │ │ │ MQTT Cloud    (○)│ │
│ │ MU-SSIEC-02  (○) │ │ │   (Ia, Ib, Ic)       │ │ │ Broker: broker.io│ │
│ │ 192.168.1.102    │ │ └──────────────────────┘ │ │ [Enable Toggle]  │ │
│ │ Rate: -- Hz      │ │                          │ └──────────────────┘ │
│ └──────────────────┘ │                          │ ┌──────────────────┐ │
│                      │                          │ │ TimescaleDB   (●)│ │
│                      │                          │ │ Status: Writing  │ │
│                      │                          │ └──────────────────┘ │
├──────────────────────┴──────────────────────────┴──────────────────────┤
│ 📜 System Diagnostics & Warnings Log Console                           │
│ [08:20:30] [INFO] OPC UA Client subscribed to Node-1 L1 OPC UA server. │
│ [08:20:31] [WARN] PTP jitter exceeded 100ns threshold (offset: 104ns). │
└────────────────────────────────────────────────────────────────────────┘
```

### Column Details

1. **Southbound Merging Units Panel (Left):**
   - Renders individual glassmorphic cards for each configured Merging Unit.
   - Shows connection status with color-coded badges, stream frame rates, and sample index telemetry.
   - Includes a **"Ping MU"** button that sends an asynchronous network ping from the SVDC daemon and displays the round-trip latency inside the card.

2. **Reconstructed Waveform & Data Visualization (Center):**
   - High-performance `canvas`-based rendering updating at 60fps from the WebSockets stream.
   - Displays overlapping Voltage sine waves (Phase A: Red/Brown, Phase B: Black/Green, Phase C: Yellow/Blue) and Current waves.
   - Provides a visual confirmation of time-alignment: when streams are correctly synchronized, the phase offset between channels is stable. If synchronization drifts, the waves visually shift or distort.
   - Displays a dynamic circular ring indicating circular buffer usage.

3. **Northbound Adapters Panel (Right):**
   - Three dedicated cards for OPC UA, MQTT, and TimescaleDB.
   - Shows active subscriber count (SCADA terminals, database connections).
   - Includes user-actionable **"Enable/Disable Toggle"** switches, enabling the professor or operators to selectively route data streams.
   - Live metrics showing throughput in data frames per second (FPS).

4. **Diagnostics Log Console (Bottom):**
   - Real-time streaming log terminal capturing structured warnings, state transitions, and network diagnostic details directly from the core tracing subscriber.

---

## Consequences

### Pros:
- **High Review Appeal:** Provides the critical visual proof of SVDC correctness to external reviewers (such as academic advisors or company leadership) without raw code parsing.
- **Portability:** Serving static assets from within the Rust binary maintains a single-executable deployment strategy with no external runtime footprint.
- **Security & Efficiency:** Operating the Axum server on a decoupled thread pool protects the hard real-time latency budget of the data ingestion hot path.
- **Dynamic Configuration:** Toggling northbound endpoints directly via REST APIs eliminates the need to restart the daemon for adapter re-routing.

### Cons:
- **Dependency Overhead:** Adds workspace dependencies (`axum`, `tokio-tungstenite`, `rust-embed`, and a JSON serializer like `serde_json`) which increases compile-time and size of the release build by ~1.5 MB.
- **Port Allocation:** Demands an open port (default `8080`) on the host system, which must be configured in firewalls.

## Alternatives Considered

- **Command Line Interface (CLI) Only:** Rejected because visualizing real-time high-speed wave forms and PTP nanosecond sync status on a command terminal is highly illegible and lacks "review appeal".
- **Separate Web Frontend (Node.js/React App):** Rejected because it violates the portable, single-executable substation deployment constraint. Requiring operators to install Node.js increases configuration overhead and system vulnerability surface.
- **TimescaleDB + Grafana Dashboard:** An excellent option for archival data verification, but does not allow direct, real-time control operations (like pinging southbound MUs or toggling northbound adapters). We will use Grafana for L3 historical analysis, but the embedded dashboard serves as the authoritative, real-time, interactive interface.

## References
- `docs/SVDC_Design_Document_v0.1.html` (Authoritative software specifications)
- `docs/verification-plan.md` (Northbound L1, L2, L3 interface descriptions)
- WBS-3.7, 3.8, 3.9 (SCADA, MQTT, and Historian implementation tasks)
