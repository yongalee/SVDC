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
    router
        .route("/south/mus", get(mus_list_page))
        .route("/south/mus/:id", get(mus_detail_page))
}

/// Renders the Southbound Merging Units page
async fn mus_list_page() -> Html<String> {
    let content = html! {
        div x-data="{
            searchQuery: '',
            statusFilter: 'all',
            selectedMus: [],
            showBulkCalibrate: false,
            bulkCalibFactor: 1.000,
            mus: [
                { id: 'MU-01', ip: '192.168.1.101', mac: '00:0a:35:01:02:01', status: 'Healthy', rate: 4000, dropped: 0, rtt: '3 ms', calib: 1.000, pinging: false },
                { id: 'MU-02', ip: '192.168.1.102', mac: '00:0a:35:01:02:02', status: 'Degraded', rate: 4000, dropped: 142, rtt: '18 ms', calib: 1.000, pinging: false },
                { id: 'MU-03', ip: '192.168.1.103', mac: '00:0a:35:01:02:03', status: 'Disconnected', rate: 0, dropped: 8563, rtt: '--', calib: 1.000, pinging: false },
                { id: 'MU-04', ip: '192.168.1.104', mac: '00:0a:35:01:02:04', status: 'Degraded', rate: 0, dropped: 12, rtt: '9 ms', calib: 1.000, pinging: false },
                { id: 'MU-05', ip: '192.168.1.105', mac: '00:0a:35:01:02:05', status: 'Healthy', rate: 4800, dropped: 0, rtt: '2 ms', calib: 1.000, pinging: false },
                { id: 'MU-06', ip: '192.168.1.106', mac: '00:0a:35:01:02:06', status: 'Healthy', rate: 4000, dropped: 0, rtt: '4 ms', calib: 1.000, pinging: false }
            ],
            toggleSelectAll() {
                const filtered = this.filteredMus();
                if (this.selectedMus.length === filtered.length) {
                    this.selectedMus = [];
                } else {
                    this.selectedMus = filtered.map(m => m.id);
                }
            },
            filteredMus() {
                return this.mus.filter(m => {
                    const query = this.searchQuery.toLowerCase();
                    const matchesSearch = m.id.toLowerCase().includes(query) ||
                                          m.ip.includes(query) ||
                                          m.mac.toLowerCase().includes(query);
                    const matchesStatus = this.statusFilter === 'all' || m.status.toLowerCase() === this.statusFilter;
                    return matchesSearch ? matchesStatus : false;
                });
            },
            pingMu(id) {
                const mu = this.mus.find(m => m.id === id);
                if (!mu) return;
                mu.pinging = true;
                setTimeout(() => {
                    mu.pinging = false;
                    if (mu.status === 'Disconnected') {
                        mu.rtt = '--';
                    } else {
                        mu.rtt = (Math.floor(Math.random() * 5) + 2) + ' ms';
                    }
                }, 400);
            },
            bulkPing() {
                this.selectedMus.forEach(id => {
                    this.pingMu(id);
                });
            },
            bulkCalibrate() {
                this.selectedMus.forEach(id => {
                    const mu = this.mus.find(m => m.id === id);
                    if (mu) {
                        mu.calib = parseFloat(this.bulkCalibFactor).toFixed(3);
                    }
                });
                alert('Applied calibration offset of ' + parseFloat(this.bulkCalibFactor).toFixed(3) + ' to selected MUs: ' + this.selectedMus.join(', '));
                this.showBulkCalibrate = false;
            }
        }"
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
            div class="flex flex-col md:flex-row gap-4 items-center justify-between" {
                div class="flex items-center gap-2 w-full md:w-auto" {
                    span class="text-xs font-semibold text-text-secondary uppercase tracking-wider" { "Filters:" }
                    div class="filter-chip-group" {
                        button class="filter-chip" x-bind:class="statusFilter === 'all' ? 'active' : ''" x-on:click="statusFilter = 'all'" { "All MUs" }
                        button class="filter-chip" x-bind:class="statusFilter === 'healthy' ? 'active' : ''" x-on:click="statusFilter = 'healthy'" { "Healthy" }
                        button class="filter-chip" x-bind:class="statusFilter === 'degraded' ? 'active' : ''" x-on:click="statusFilter = 'degraded'" { "Degraded" }
                        button class="filter-chip" x-bind:class="statusFilter === 'disconnected' ? 'active' : ''" x-on:click="statusFilter = 'disconnected'" { "Disconnected" }
                    }
                }

                div class="search-box w-full md:w-64" style="max-width: 300px;" {
                    input type="text" placeholder="Search by ID, IP, MAC..." class="w-full text-xs" x-model="searchQuery";
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
                    button x-on:click="bulkPing()" { "Bulk Ping" }
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
                                        button x-on:click="pingMu(mu.id)" class="btn-primary py-1 px-2 text-[11px] flex items-center gap-1" {
                                            span class="btn-spinner" x-show="mu.pinging" {}
                                            span x-text="mu.pinging ? 'Pinging...' : 'Ping'" {}
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
        }
    };

    let rendered = base::layout("Southbound Merging Units", "southbound", content);
    Html(rendered.into_string())
}

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
            
            // Calibration values
            rms_va: 1.000, angle_va: 0.0,
            rms_vb: 1.000, angle_vb: 0.0,
            rms_vc: 1.000, angle_vc: 0.0,
            rms_ia: 1.000, angle_ia: 0.0,
            rms_ib: 1.000, angle_ib: 0.0,
            rms_ic: 1.000, angle_ic: 0.0,
            
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
                        this.toastMsg = 'IEC 61850 configuration parameters saved to ' + this.muId + ' successfully.';
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
            div class="grid grid-cols-1 lg:grid-cols-12 gap-6" {

                // Left & Center Column (Settings Form)
                div class="lg:col-span-9 flex flex-col gap-6" {

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

                    // Live Calibration parameters
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "3. 3-Phase Calibration Magnitude & Angle Offsets" }
                        }
                        div class="card-body mt-3 flex flex-col gap-3" {

                            // Voltage calibration parameters
                            div class="border-b border-border-color pb-2.5" {
                                h4 class="text-xs font-bold text-accent-blue uppercase mb-1.5" { "Voltage Channels (Va, Vb, Vc)" }
                                div class="flex flex-col gap-2 text-[10px]" {

                                    // Va
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold text-accent-red" style="min-width:130px;" { "Phase A Voltage (Va)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_va";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_va";
                                        }
                                    }

                                    // Vb
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold text-accent-green" style="min-width:130px;" { "Phase B Voltage (Vb)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_vb";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_vb";
                                        }
                                    }

                                    // Vc
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold text-accent-blue" style="min-width:130px;" { "Phase C Voltage (Vc)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_vc";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_vc";
                                        }
                                    }

                                }
                            }

                            // Current calibration parameters
                            div {
                                h4 class="text-xs font-bold text-accent-yellow uppercase mb-1.5" { "Current Channels (Ia, Ib, Ic)" }
                                div class="flex flex-col gap-2 text-[10px]" {

                                    // Ia
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold" style="min-width:130px;color:#d97706;" { "Phase A Current (Ia)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_ia";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_ia";
                                        }
                                    }

                                    // Ib
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold" style="min-width:130px;color:#8b5cf6;" { "Phase B Current (Ib)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_ib";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_ib";
                                        }
                                    }

                                    // Ic
                                    div class="bg-bg-secondary p-2 rounded border border-border-color flex items-center gap-3" style="flex-wrap:wrap;" {
                                        span class="font-bold" style="min-width:130px;color:#14b8a6;" { "Phase C Current (Ic)" }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Mag:" }
                                            input type="number" min="0.5" max="2.0" step="0.001" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="rms_ic";
                                        }
                                        div class="flex items-center gap-1" {
                                            span class="text-[9px] text-text-secondary" { "Angle:" }
                                            input type="number" min="-30" max="30" step="0.5" class="mini-num-input" style="width:70px;max-width:70px;text-align:right;" x-model="angle_ic";
                                        }
                                    }


                                }
                            }

                        }
                    }

                }

                // Right Column (Visualizations & Operations)
                div class="lg:col-span-3 flex flex-col gap-6" {

                    // Waveform panel
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Interactive Waveform Plot" }
                        }
                        div class="card-body mt-3 flex flex-col items-center gap-3" {
                            div class="waveform-container" {
                                // SVG waveform canvas
                                svg viewBox="0 0 600 150" class="waveform-svg" {
                                    // Zero line grid
                                    line x1="0" y1="75" x2="600" y2="75" stroke="#cbd5e1" stroke-width="0.5" stroke-dasharray="4" {}

                                    // Dynamic Phase Paths (Voltage sine waves Va, Vb, Vc)
                                    path x-bind:d="getWaveformPath(rms_va, angle_va, 0)"
                                         fill="none" stroke="#ef4444" stroke-width="1.5" stroke-linecap="round" {}

                                    path x-bind:d="getWaveformPath(rms_vb, angle_vb, 120)"
                                         fill="none" stroke="#22c55e" stroke-width="1.5" stroke-linecap="round" {}

                                    path x-bind:d="getWaveformPath(rms_vc, angle_vc, 240)"
                                         fill="none" stroke="#3b82f6" stroke-width="1.5" stroke-linecap="round" {}
                                }
                            }

                            // Waveform Legend
                            div class="flex justify-center gap-4 text-[10px] font-semibold w-full border-t border-border-color pt-2" {
                                div class="flex items-center gap-1.5" {
                                    span class="w-2.5 h-1.5 rounded-full bg-accent-red" {}
                                    span { "Va Phase A" }
                                }
                                div class="flex items-center gap-1.5" {
                                    span class="w-2.5 h-1.5 rounded-full bg-accent-green" {}
                                    span { "Vb Phase B" }
                                }
                                div class="flex items-center gap-1.5" {
                                    span class="w-2.5 h-1.5 rounded-full bg-accent-blue" {}
                                    span { "Vc Phase C" }
                                }
                            }
                        }
                    }

                    // Write-back Substation Commands Panel
                    div class="glass-card shadow-md" {
                        div class="card-header border-b border-border-color pb-3" {
                            h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Write & Commit Substation Config" }
                        }
                        div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                            p class="text-text-secondary leading-relaxed" {
                                "Writing configuration coefficients to a live Merging Unit triggers a lock discipline recalculation. In accordance with "
                                strong { "IEC 61850-9-2" }
                                " standards, this action is audited under quasi-dynamic state estimator write-back controls."
                            }

                            // Progress bar
                            div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                div class="flex justify-between text-[10px]" {
                                    span class="text-text-secondary" { "Uploading config via PRP..." }
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
                                span x-text="saving ? 'Committing parameters...' : 'Write Configuration to Device'" {}
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
