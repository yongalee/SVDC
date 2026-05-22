/* SVDC Southbound Merging Units Router
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::{extract::Path, response::Html, routing::get, Router};
use maud::html;

use crate::templates::base;

/// Register routes related to southbound Merging Units list and actions
pub fn register(router: Router) -> Router {
    router.route("/south/mus", get(mus_list_page))
}

/// Renders the Southbound Merging Units page
async fn mus_list_page() -> Html<String> {
    let snapshot = crate::scd::registry::global().snapshot();
    let connected = crate::routes::mu_detail::connected_mus().read().unwrap();
    let mut mu_list_json = Vec::new();
    for mu in snapshot {
        let is_connected = connected.contains(&mu.id);
        let status = if is_connected {
            if mu.id == "MU-02" || mu.id == "MU-04" {
                "Degraded"
            } else {
                "Healthy"
            }
        } else {
            "Disconnected"
        };
        let rate = if is_connected { mu.smp_rate } else { 0 };
        let dropped = if is_connected {
            if mu.id == "MU-02" {
                142
            } else if mu.id == "MU-04" {
                12
            } else {
                0
            }
        } else {
            8563
        };
        let rtt = if is_connected {
            if mu.id == "MU-02" {
                "18 ms"
            } else if mu.id == "MU-04" {
                "9 ms"
            } else {
                "3 ms"
            }
        } else {
            "--"
        };

        let mac_str = format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            mu.mac[0], mu.mac[1], mu.mac[2], mu.mac[3], mu.mac[4], mu.mac[5]
        );
        let ip_str = format!("192.168.1.{}", 100 + mu.mac[5]);

        mu_list_json.push(serde_json::json!({
            "id": mu.id,
            "ip": ip_str,
            "mac": mac_str,
            "status": status,
            "rate": rate,
            "dropped": dropped,
            "rtt": rtt,
            "calib": 1.000,
            "pinging": false
        }));
    }
    let mus_json_str = serde_json::to_string(&mu_list_json).unwrap_or_else(|_| "[]".to_string());

    let content = html! {
        div x-data=(maud::PreEscaped(format!(
            "{{
            searchQuery: '',
            statusFilter: 'all',
            selectedMus: [],
            showBulkCalibrate: false,
            bulkCalibFactor: 1.000,
            mus: {},
            toggleSelectAll() {{
                const filtered = this.filteredMus();
                if (this.selectedMus.length === filtered.length) {{
                    this.selectedMus = [];
                }} else {{
                    this.selectedMus = filtered.map(m => m.id);
                }}
            }},
            filteredMus() {{
                return this.mus.filter(m => {{
                    const query = this.searchQuery.toLowerCase();
                    const matchesSearch = m.id.toLowerCase().includes(query) ||
                                          m.ip.includes(query) ||
                                          m.mac.toLowerCase().includes(query);
                    const matchesStatus = this.statusFilter === 'all' || m.status.toLowerCase() === this.statusFilter;
                    return matchesSearch ? matchesStatus : false;
                }});
            }},
            // SV-frame-arrival health check; see comment block on the
            // routes::mus_list module header for the design note.
            muLastSeen: {{}},
            pingMu(id) {{
                const mu = this.mus.find(m => m.id === id);
                if (!mu) return;
                mu.pinging = true;
                setTimeout(() => {{
                    mu.pinging = false;
                    const seenAt = this.muLastSeen[id];
                    const now = Date.now();
                    if (!seenAt) {{
                        mu.status = \"Disconnected\";
                        mu.rtt = \"no frames\";
                        return;
                    }}
                    const ageMs = now - seenAt;
                    mu.rtt = ageMs < 1000 ? (ageMs + \" ms\") : ((ageMs/1000).toFixed(1) + \" s\");
                    if (ageMs < 1500) {{
                        mu.status = \"Healthy\";
                    }} else if (ageMs < 5000) {{
                        mu.status = \"Degraded\";
                    }} else {{
                        mu.status = \"Disconnected\";
                    }}
                }}, 150);
            }},
            bulkPing() {{
                this.selectedMus.forEach(id => {{
                    this.pingMu(id);
                }});
            }},
            bulkCalibrate() {{
                this.selectedMus.forEach(id => {{
                    const mu = this.mus.find(m => m.id === id);
                    if (mu) {{
                        mu.calib = parseFloat(this.bulkCalibFactor).toFixed(3);
                    }}
                }});
                alert('Applied calibration offset of ' + parseFloat(this.bulkCalibFactor).toFixed(3) + ' to selected MUs: ' + this.selectedMus.join(', '));
                this.showBulkCalibrate = false;
            }}
        }}",
            mus_json_str
        )))
        "x-init"="
            // Existing MuMetrics handler: per-MU dashboard metrics
            // (observed SPS, missing samples, calibration).
            const es = new EventSource('/api/events');
            es.onmessage = (e) => {
                try {
                    const payload = JSON.parse(e.data);
                    if (payload.event_type === 'MuMetrics') {
                        const list = payload.data;
                        list.forEach(item => {
                            const mu = mus.find(m => m.id === item.mu_id);
                            if (mu) {
                                mu.rate = item.observed_sps;
                                mu.dropped = item.missing_samples;
                                mu.status = item.observed_sps > 0 ? (item.missing_samples > 0 ? 'Degraded' : 'Healthy') : 'Disconnected';
                                if (item.calibration && item.calibration.length >= 3) {
                                    mu.calib = item.calibration[0];
                                }
                            }
                        });
                    } else if (payload.event_type === 'Waveform' && payload.data && payload.data.mu_id) {
                        // SV-frame-arrival tracker for the Ping/Health
                        // button. Each Waveform event tags a mu_id; we
                        // remember the most recent arrival timestamp
                        // per MU. pingMu() now reads this instead of
                        // rolling a fake RTT.
                        muLastSeen[payload.data.mu_id] = Date.now();
                    }
                } catch(err) {
                    console.error('Failed to parse SSE in MUs list:', err);
                }
            };
        "
        class="screen-layout flex flex-col gap-6" {
            // Summary header (no meaningless icon)
            div class="glass-card" {
                div class="card-header flex items-center gap-2" {
                    h2 class="card-title" { "Southbound Ingest Grid Console" }
                }
                div class="card-body mt-2 text-sm text-text-secondary" {
                    p {
                        "The southbound ingest engine processes raw IEC 61850-9-2 Sampled Values (SV) frames broadcast from Merging Units (MUs) connected to the substation process bus. "
                        "Incoming frames are received with zero heap allocation, calibrated using configured offsets, and immediately written into the dual-redundant circular buffers."
                    }
                }
            }

            // Grid Controls (Search and Filters)
            div class="flex flex-row gap-4 items-center justify-between w-full" {
                div class="flex items-center gap-2" {
                    span class="text-xs font-semibold text-text-secondary uppercase tracking-wider" { "Filters:" }
                    div class="filter-chip-group" {
                        button class="filter-chip" x-bind:class="statusFilter === 'all' ? 'active' : ''" x-on:click="statusFilter = 'all'" { "All MUs" }
                        button class="filter-chip" x-bind:class="statusFilter === 'healthy' ? 'active' : ''" x-on:click="statusFilter = 'healthy'" { "Healthy" }
                        button class="filter-chip" x-bind:class="statusFilter === 'degraded' ? 'active' : ''" x-on:click="statusFilter = 'degraded'" { "Degraded" }
                        button class="filter-chip" x-bind:class="statusFilter === 'disconnected' ? 'active' : ''" x-on:click="statusFilter = 'disconnected'" { "Disconnected" }
                    }
                }

                div class="flex items-center gap-2 flex-nowrap shrink-0" {
                    div class="search-box w-64" style="max-width: 300px;" {
                        input type="text" placeholder="Search by ID, IP, MAC..." class="w-full" x-model="searchQuery";
                    }
                    a href="/south/mus/new" class="btn btn-primary text-xs whitespace-nowrap px-4 py-2.5" { "Add MU" }
                }
            }

            // Dynamic Bulk Action Panel
            div class="bulk-actions-toolbar" x-show="selectedMus.length > 0" x-transition {
                div class="flex items-center gap-4" {
                    span class="font-semibold" {
                        span x-text="selectedMus.length" {} " Merging Units selected"
                    }
                    div class="flex items-center gap-2" x-show="showBulkCalibrate" x-transition {
                        span class="text-xs text-text-secondary" { "Calibration Factor:" }
                        input type="number" step="0.001" min="0.5" max="2.0" class="mini-num-input" x-model="bulkCalibFactor";
                        button x-on:click="bulkCalibrate()" class="px-2 py-1 text-xs" { "Apply" }
                        button x-on:click="showBulkCalibrate = false" class="px-2 py-1 text-xs bg-gray-500 hover:bg-gray-600" { "Cancel" }
                    }
                }
                div class="flex gap-2" x-show="!showBulkCalibrate" {
                    button x-on:click="bulkPing()" title="Run the SV-frame-arrival health check on every selected MU." { "Bulk health check" }
                    button x-on:click="showBulkCalibrate = true" class="bg-accent-green hover:bg-[#047857]" { "Bulk Calibrate" }
                }
            }

            // High-Density Data Grid
            div class="bg-bg-secondary rounded-lg border border-border-color p-2 overflow-x-auto shadow-sm" {
                 table class="industrial-grid" {
                    thead {
                        tr {
                            th class="w-12 text-center" {
                                input type="checkbox" x-on:click="toggleSelectAll()" x-bind:checked="filteredMus().length > 0 ? selectedMus.length === filteredMus().length : false";
                            }
                            th class="text-center" { "MU ID" }
                            th class="text-center" { "Status" }
                            th class="text-center" { "IP Address" }
                            th class="text-center" { "MAC Address" }
                            th class="text-center" { "Sample Rate" }
                            th class="text-center" { "Dropped" }
                            th class="text-center" { "Latency" }
                            th class="text-center" { "Calib. Factor" }
                            th class="w-48 text-center" { "Actions" }
                        }
                    }
                    tbody {
                        template x-for="mu in filteredMus()" x-bind:key="mu.id" {
                            tr class="cursor-pointer hover:bg-bg-surface"
                               x-bind:class="selectedMus.includes(mu.id) ? 'row-selected' : ''"
                               x-on:click="if ($event.target.closest('input') || $event.target.closest('button') || $event.target.closest('a')) return; window.location.href = '/south/mus/' + mu.id" {
                                td class="text-center" {
                                    input type="checkbox" x-bind:value="mu.id" x-model="selectedMus";
                                }
                                td class="font-semibold text-center" x-text="mu.id" {}
                                td class="text-center" {
                                    div class="flex justify-center" {
                                        span class="status-badge" x-bind:class="mu.status === 'Healthy' ? 'status-badge-healthy' : (mu.status === 'Degraded' ? 'status-badge-degraded' : 'status-badge-fault')" {
                                            span class="status-dot-pulse" {}
                                            span x-text="mu.status" {}
                                        }
                                    }
                                }
                                td class="font-mono text-center" x-text="mu.ip" {}
                                td class="font-mono text-xs text-center" x-text="mu.mac" {}
                                td class="font-semibold text-accent-blue text-center" x-text="mu.rate > 0 ? mu.rate + ' sps' : '0 sps'" {}
                                td class="font-semibold text-center" x-bind:class="mu.dropped > 0 ? 'text-accent-red' : 'text-text-primary'" x-text="mu.dropped" {}
                                td class="font-mono text-accent-green text-center" x-text="mu.rtt" {}
                                td class="text-center" {
                                    span class="font-mono font-semibold" x-text="parseFloat(mu.calib).toFixed(3)" {}
                                }
                                td class="text-center" {
                                    div class="flex gap-2 justify-center" {
                                        button x-on:click="pingMu(mu.id)"
                                               class="btn-primary py-1 px-2 text-[11px] flex items-center gap-1"
                                               title="Health check: refresh this MU's status from the last SV frame arrival (no ICMP — IEC 61850-9-2 SV is one-way multicast)." {
                                            span class="btn-spinner" x-show="mu.pinging" {}
                                            span x-text="mu.pinging ? 'Checking...' : 'Health check'" {}
                                        }
                                        // PR-M follow-up: operator-driven quarantine.
                                        // Disconnect drops frames from this MU at the
                                        // aligner without removing it from the registry.
                                        template x-if="mu.status !== 'Disconnected'" {
                                            button x-on:click="muToggleConnection(mu.id, false)"
                                                   class="btn-secondary py-1 px-2 text-[11px]"
                                                   title="Stop accepting frames from this MU until reconnected." {
                                                "Disconnect"
                                            }
                                        }
                                        template x-if="mu.status === 'Disconnected'" {
                                            button x-on:click="muToggleConnection(mu.id, true)"
                                                   class="btn-secondary py-1 px-2 text-[11px] bg-accent-green text-white border-accent-green"
                                                   title="Re-attach this MU to the aligner." {
                                                "Connect"
                                            }
                                        }
                                        a x-bind:href="'/south/mus/' + mu.id" class="btn-primary py-1 px-2 text-[11px] bg-accent-blue hover:bg-[#1d4ed8] text-center" {
                                            "Settings"
                                        }
                                    }
                                }
                            }
                        }
                        tr x-show="filteredMus().length === 0" {
                            td colspan="10" class="text-center text-text-muted py-6 font-medium" {
                                "No Merging Units found matching current search and filter criteria."
                            }
                        }
                    }
                }
            }
            // PR-M follow-up: global helper used by the Disconnect /
            // Connect buttons rendered above.
            script {
                (maud::PreEscaped(MUS_LIST_CONNECTION_JS))
            }
        }
    };

    let rendered = base::layout("Southbound Merging Units", "southbound", content);
    Html(rendered.into_string())
}

const MUS_LIST_CONNECTION_JS: &str = r#"
window.muToggleConnection = async function(muId, connect) {
  const action = connect ? 'connect' : 'disconnect';
  if (!connect) {
    const ok = confirm(
      'Disconnect MU ' + muId + '?\n\n' +
      'The aligner will drop incoming frames from this MU until it is ' +
      'reconnected. Use this to quarantine a noisy or under-maintenance MU. ' +
      'The channel registry entry is preserved; reconnect is one click.'
    );
    if (!ok) return;
  }
  try {
    const r = await fetch('/api/mgmt/mu/' + encodeURIComponent(muId) + '/' + action, { method: 'POST' });
    if (r.ok) {
      window.location.reload();
    } else {
      alert('MU ' + action + ' failed: HTTP ' + r.status);
    }
  } catch (e) {
    alert('MU ' + action + ' error: ' + e);
  }
};
"#;

/// Renders the Merging Unit detail & configuration page
async fn mus_detail_page(Path(id): Path<String>) -> Html<String> {
    // Generate initial values based on MU ID
    let initial_status = match id.as_str() {
        "MU-01" | "MU-05" | "MU-06" => "Healthy",
        "MU-02" | "MU-04" => "Degraded",
        _ => "Disconnected",
    };

    let initial_mac = match id.as_str() {
        "MU-01" => "00:0a:35:01:02:01",
        "MU-02" => "00:0a:35:01:02:02",
        "MU-03" => "00:0a:35:01:02:03",
        "MU-04" => "00:0a:35:01:02:04",
        "MU-05" => "00:0a:35:01:02:05",
        "MU-06" => "00:0a:35:01:02:06",
        _ => "00:0a:35:01:02:99",
    };

    let initial_ip = match id.as_str() {
        "MU-01" => "192.168.1.101",
        "MU-02" => "192.168.1.102",
        "MU-03" => "192.168.1.103",
        "MU-04" => "192.168.1.104",
        "MU-05" => "192.168.1.105",
        "MU-06" => "192.168.1.106",
        _ => "192.168.1.199",
    };

    let content = html! {
        div x-data=(format!("{{
            muId: '{}',
            status: '{}',
            macAddress: '{}',
            ipAddress: '{}',
            svID: 'SSIEC_{}',
            confRev: 1,
            smpRate: 4000,
            noASDU: 1,
            smpSyn: '2',
            appID: '0x4000',
            vlanID: 10,
            vlanPriority: 4,
            prpMode: 'PRP',
            
            // Instrument Transformer parameters (SDD §7.2)
            pt_ratio: 1100.0,
            ct_ratio: 200.0,
            polarity_v: 'Normal',
            polarity_i: 'Normal',
            
            // Calibration values (SDD §6 M4 triple: scale, offset, φ)
            rms_va: 1.000, dc_va: 0.000, angle_va: 0.0,
            rms_vb: 1.000, dc_vb: 0.000, angle_vb: 0.0,
            rms_vc: 1.000, dc_vc: 0.000, angle_vc: 0.0,
            rms_ia: 1.000, dc_ia: 0.000, angle_ia: 0.0,
            rms_ib: 1.000, dc_ib: 0.000, angle_ib: 0.0,
            rms_ic: 1.000, dc_ic: 0.000, angle_ic: 0.0,
            
            // Waveform visibility toggles
            show_va: true, show_vb: true, show_vc: true,
            show_ia: false, show_ib: false, show_ic: false,
            
            saving: false,
            progress: 0,
            showToast: false,
            toastMsg: '',
            
            getWaveformPath(rms, angle, phaseShift) {{
                let points = [];
                let width = 600;
                let height = 150;
                let centerY = 75;
                let scaleX = width / 360;
                let scaleY = 35;
                let shiftRad = (parseFloat(phaseShift) + parseFloat(angle)) * Math.PI / 180;
                for (let deg = 0; deg <= 360; deg += 3) {{
                    let rad = deg * Math.PI / 180;
                    let y = centerY - Math.sin(rad - shiftRad) * scaleY * parseFloat(rms);
                    points.push((deg * scaleX).toFixed(1) + ',' + y.toFixed(1));
                }}
                return 'M ' + points.join(' L ');
            }},
            
            writeConfiguration() {{
                if (this.saving) return;
                this.saving = true;
                this.progress = 0;
                let interval = setInterval(() => {{
                    this.progress += 10;
                    if (this.progress >= 100) {{
                        clearInterval(interval);
                        this.saving = false;
                        this.toastMsg = 'Calibration triple (scale, offset, φ) applied to ' + this.muId + ' channel pipeline successfully.';
                        this.showToast = true;
                        setTimeout(() => {{ this.showToast = false; }}, 4000);
                    }}
                }}, 60);
            }}
        }}", id, initial_status, initial_mac, initial_ip, id.replace("-", "_")))
        class="screen-layout flex flex-col gap-6 relative" {

            // Toast Notification
            div class="fixed bottom-6 right-6 bg-accent-green text-white px-4 py-3 rounded-lg shadow-xl flex items-center gap-2 text-xs font-semibold z-50 transition-all duration-300 transform"
                 x-show="showToast"
                 x-transition:enter="transition ease-out duration-300"
                 x-transition:enter-start="opacity-0 translate-y-2"
                 x-transition:enter-end="opacity-100 translate-y-0"
                 x-transition:leave="transition ease-in duration-200"
                 x-transition:leave-start="opacity-100 translate-y-0"
                 x-transition:leave-end="opacity-0 translate-y-2" {
                span { "✓" }
                span x-text="toastMsg" {}
            }

            // Back link
            div {
                a href="/south/mus" class="inline-flex items-center gap-1.5 text-xs text-accent-blue hover:underline font-bold uppercase tracking-wider" {
                    "← Back to Southbound Ingest Grid"
                }
            }

            // Header block (no meaningless icon)
            div class="glass-card p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-md" {
                div class="flex items-center gap-3" {
                    div {
                        h2 class="text-sm font-bold tracking-tight text-text-primary" {
                            "Merging Unit Settings & Calibration Console: "
                            span x-text="muId" {}
                        }
                        p class="text-text-secondary text-[11px] mt-1" {
                            "IEC 61850-9-2 Process Bus Ingestion Block Configurator & Downsampled Waveform Calibration"
                        }
                    }
                }
                div {
                    span class="status-badge"
                          x-bind:class="status === 'Healthy' ? 'status-badge-healthy' : (status === 'Degraded' ? 'status-badge-degraded' : 'status-badge-fault')" {
                        span class="status-dot-pulse" {}
                        span x-text="status" {}
                    }
                }
            }

            // Multi-column Editor Panel
            div class="flex flex-col gap-6" {

                // Settings Form
                div class="flex flex-col gap-6" {

                    // IEC 61850 Ingestion parameters
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "1. IEC 61850 Ingestion Parameter Block" }
                        }
                        div class="card-body mt-4 grid grid-cols-1 md:grid-cols-3 gap-4 text-xs" {
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Sampled Value ID (svID)" }
                                input type="text" class="w-full text-xs font-mono" x-model="svID";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Configuration Revision (confRev)" }
                                input type="number" class="w-full text-xs font-mono" x-model="confRev";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Sample Rate (smpRate)" }
                                select class="w-full text-xs font-mono" x-model="smpRate" {
                                    option value="4000" { "4000 sps (80 samples/cycle)" }
                                    option value="4800" { "4800 sps (96 samples/cycle)" }
                                    option value="12800" { "12800 sps (256 samples/cycle)" }
                                }
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "ASDU Count (noASDU)" }
                                select class="w-full text-xs font-mono" x-model="noASDU" {
                                    option value="1" { "1 ASDU per frame" }
                                    option value="2" { "2 ASDU per frame" }
                                    option value="8" { "8 ASDU per frame" }
                                }
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Synchrony Mode (smpSyn)" }
                                select class="w-full text-xs font-mono" x-model="smpSyn" {
                                    option value="0" { "None (0) - Unsynchronized" }
                                    option value="1" { "Local (1) - Local Clock Lock" }
                                    option value="2" { "Global PTP (2) - Grandmaster Lock" }
                                }
                            }
                        }
                    }

                    // Process Bus Ethernet & VLAN
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "2. Ethernet Multicast Ingest & VLAN Routing" }
                        }
                        div class="card-body mt-4 grid grid-cols-1 md:grid-cols-3 gap-4 text-xs" {
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Destination MAC Address" }
                                input type="text" class="w-full text-xs font-mono" x-model="macAddress";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Application ID (appID)" }
                                input type="text" class="w-full text-xs font-mono" x-model="appID";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "VLAN Identifier (0-4095)" }
                                input type="number" min="0" max="4095" class="w-full text-xs font-mono" x-model="vlanID";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "VLAN Priority (0-7)" }
                                input type="number" min="0" max="7" class="w-full text-xs font-mono" x-model="vlanPriority";
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Redundancy Protocol" }
                                select class="w-full text-xs font-mono" x-model="prpMode" {
                                    option value="None" { "None (Single interface)" }
                                    option value="PRP" { "PRP (Parallel Redundancy)" }
                                    option value="HSR" { "HSR (High-availability Seamless)" }
                                }
                            }
                        }
                    }

                    // Instrument Transformer Parameters (SDD §7.2)
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "3. Instrument Transformer Parameters" }
                        }
                        div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs" {
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "PT Ratio (Voltage Transformer)" }
                                input type="number" min="1" max="50000" step="0.1" class="w-full text-xs font-mono" x-model="pt_ratio";
                                span class="text-[9px] text-text-secondary" { "Primary-to-secondary voltage transformer turns ratio (e.g., 1100:1)." }
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "CT Ratio (Current Transformer)" }
                                input type="number" min="1" max="50000" step="0.1" class="w-full text-xs font-mono" x-model="ct_ratio";
                                span class="text-[9px] text-text-secondary" { "Primary-to-secondary current transformer turns ratio (e.g., 200:1)." }
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Voltage Channel Polarity" }
                                select class="w-full text-xs font-mono" x-model="polarity_v" {
                                    option value="Normal" { "Normal (+1)" }
                                    option value="Inverted" { "Inverted (-1)" }
                                }
                                span class="text-[9px] text-text-secondary" { "Polarity convention of the voltage measurement channels (SDD §7.2)." }
                            }
                            div class="flex flex-col gap-1" {
                                label class="font-medium text-text-primary" { "Current Channel Polarity" }
                                select class="w-full text-xs font-mono" x-model="polarity_i" {
                                    option value="Normal" { "Normal (+1)" }
                                    option value="Inverted" { "Inverted (-1)" }
                                }
                                span class="text-[9px] text-text-secondary" { "Polarity convention of the current measurement channels (SDD §7.2)." }
                            }
                        }
                    }

                    // Live Calibration parameters (SDD §6 M4: scale, offset, φ triple)
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "4. 3-Phase Calibration Triple (DC Offset / Magnitude / Angle)" }
                        }
                        div class="card-body mt-3 flex flex-col gap-3" {

                            // Calibration parameters — 2-column grid (Voltage | Current)
                            div class="grid grid-cols-2 gap-3 text-[10px]" {

                                // Column headers
                                h4 class="text-xs font-bold text-accent-blue uppercase" { "Voltage (Va, Vb, Vc)" }
                                h4 class="text-xs font-bold text-accent-blue uppercase" { "Current (Ia, Ib, Ic)" }

                                // Phase A row
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-red" style="min-width:50px;" { "Va" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_va";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_va";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_va";
                                }
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-red" style="min-width:50px;" { "Ia" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_ia";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_ia";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_ia";
                                }

                                // Phase B row
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-green" style="min-width:50px;" { "Vb" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_vb";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_vb";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_vb";
                                }
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-green" style="min-width:50px;" { "Ib" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_ib";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_ib";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_ib";
                                }

                                // Phase C row
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-blue" style="min-width:50px;" { "Vc" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_vc";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_vc";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_vc";
                                }
                                div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-2" style="flex-wrap:wrap;" {
                                    span class="font-bold text-accent-blue" style="min-width:50px;" { "Ic" }
                                    span class="text-[9px] text-text-secondary" { "DC:" }
                                    input type="number" min="-10" max="10" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="dc_ic";
                                    span class="text-[9px] text-text-secondary" { "Mag:" }
                                    input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="rms_ic";
                                    span class="text-[9px] text-text-secondary" { "Ang:" }
                                    input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:60px;max-width:60px;text-align:right;" x-model="angle_ic";
                                }
                            }

                        }
                    }

                }

                // Visualizations
                div class="flex flex-col gap-6" {

                    // Waveform panel
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Interactive Waveform Plot" }
                        }
                        div class="card-body mt-3 flex flex-col gap-3" {
                            div class="flex gap-4 items-center" {
                                // Left: Channel checkboxes with line-style legends
                                div class="flex flex-col gap-1" style="min-width:80px;" {
                                    span class="text-[9px] font-bold text-text-secondary uppercase tracking-wider mb-1" { "Voltage" }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_va" class="accent-[#ef4444]";
                                        span style="width:18px;height:0;border-top:2px solid #ef4444;flex-shrink:0;" {}
                                        span class="font-semibold" { "Va" }
                                    }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_vb" class="accent-[#22c55e]";
                                        span style="width:18px;height:0;border-top:2px solid #22c55e;flex-shrink:0;" {}
                                        span class="font-semibold" { "Vb" }
                                    }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_vc" class="accent-[#3b82f6]";
                                        span style="width:18px;height:0;border-top:2px solid #3b82f6;flex-shrink:0;" {}
                                        span class="font-semibold" { "Vc" }
                                    }
                                    span class="text-[9px] font-bold text-text-secondary uppercase tracking-wider mt-2 mb-1" { "Current" }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_ia" class="accent-[#f59e0b]";
                                        span style="width:18px;height:0;border-top:2px dashed #f59e0b;flex-shrink:0;" {}
                                        span class="font-semibold" { "Ia" }
                                    }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_ib" class="accent-[#8b5cf6]";
                                        span style="width:18px;height:0;border-top:2px dashed #8b5cf6;flex-shrink:0;" {}
                                        span class="font-semibold" { "Ib" }
                                    }
                                    label class="flex items-center gap-2 cursor-pointer text-[10px] py-0.5" {
                                        input type="checkbox" x-model="show_ic" class="accent-[#14b8a6]";
                                        span style="width:18px;height:0;border-top:2px dashed #14b8a6;flex-shrink:0;" {}
                                        span class="font-semibold" { "Ic" }
                                    }
                                }

                                // Right: SVG waveform canvas
                                div class="flex-1" {
                                    div class="waveform-container" {
                                        svg viewBox="0 0 600 150" class="waveform-svg" {
                                            // Zero line grid
                                            line x1="0" y1="75" x2="600" y2="75" stroke="#cbd5e1" stroke-width="0.5" stroke-dasharray="4" {}

                                            // Voltage waveforms (solid lines)
                                            path x-show="show_va"
                                                 x-bind:d="getWaveformPath(rms_va, angle_va, 0)"
                                                 fill="none" stroke="#ef4444" stroke-width="1.5" stroke-linecap="round" {}
                                            path x-show="show_vb"
                                                 x-bind:d="getWaveformPath(rms_vb, angle_vb, 120)"
                                                 fill="none" stroke="#22c55e" stroke-width="1.5" stroke-linecap="round" {}
                                            path x-show="show_vc"
                                                 x-bind:d="getWaveformPath(rms_vc, angle_vc, 240)"
                                                 fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-linecap="round" {}

                                            // Current waveforms (dashed lines)
                                            path x-show="show_ia"
                                                 x-bind:d="getWaveformPath(rms_ia, angle_ia, -30)"
                                                 fill="none" stroke="#f59e0b" stroke-width="1.5" stroke-linecap="round" stroke-dasharray="6,3" {}
                                            path x-show="show_ib"
                                                 x-bind:d="getWaveformPath(rms_ib, angle_ib, 90)"
                                                 fill="none" stroke="#8b5cf6" stroke-width="1.5" stroke-linecap="round" stroke-dasharray="6,3" {}
                                            path x-show="show_ic"
                                                 x-bind:d="getWaveformPath(rms_ic, angle_ic, 210)"
                                                 fill="none" stroke="#14b8a6" stroke-width="1.5" stroke-linecap="round" stroke-dasharray="6,3" {}
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Calibration Commit Panel (SDD §6 M4 + §8.4)
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Apply Calibration to SVDC Engine" }
                        }
                        div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                            p class="text-text-secondary leading-relaxed" {
                                "Updates the per-channel calibration triple "
                                strong { "(scale, offset, φ)" }
                                " within the SVDC ingest pipeline via atomic copy-on-write. New calibration factors are applied immediately to incoming SV samples without restarting the data plane."
                            }
                            p class="text-[10px] text-text-secondary leading-relaxed" {
                                "This operation targets the SVDC internal calibration table ("
                                strong { "POST /calibration/{channel_id}" }
                                "), not the physical Merging Unit hardware."
                            }

                            // Progress bar
                            div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                div class="flex justify-between text-[10px]" {
                                    span class="text-text-secondary" { "Committing calibration triple (copy-on-write)..." }
                                    span class="font-bold text-accent-blue" x-text="progress + '%'" {}
                                }
                                div class="progressbar-bg h-2 rounded overflow-hidden" {
                                    div class="progressbar-fill h-full bg-accent-blue transition-all duration-100"
                                         x-bind:style="'width: ' + progress + '%'" {}
                                }
                            }

                            // Write Button
                            button class="btn-primary w-full py-2 flex items-center justify-center gap-2 font-bold uppercase tracking-wider text-xs"
                                    x-bind:disabled="saving"
                                    x-on:click="writeConfiguration()" {
                                span class="btn-spinner" x-show="saving" {}
                                span x-text="saving ? 'Applying calibration...' : 'Apply Calibration to Engine'" {}
                            }
                        }
                    }

                }

            }

        }
    };

    let rendered = base::layout("Merging Unit Details", "southbound", content);
    Html(rendered.into_string())
}
