/* SVDC Southbound Merging Units Router
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::{
    response::Html,
    routing::get,
    Router,
};
use maud::html;

use crate::templates::base;

/// Register routes related to southbound Merging Units list and actions
pub fn register(router: Router) -> Router {
    router.route("/south/mus", get(mus_list_page))
}

/// Renders the Southbound Merging Units page
async fn mus_list_page() -> Html<String> {
    let content = html! {
        div "x-data" "{
            searchQuery: '',
            statusFilter: 'all',
            selectedMus: [],
            showBulkCalibrate: false,
            bulkCalibFactor: 1.000,
            mus: [
                { id: 'MU-01', ip: '192.168.1.101', mac: '00:50:C2:88:99:A1', status: 'Healthy', rate: 4000, dropped: 0, rtt: '3 ms', calib: 1.000, pinging: false },
                { id: 'MU-02', ip: '192.168.1.102', mac: '00:50:C2:88:99:A2', status: 'Degraded', rate: 4000, dropped: 142, rtt: '18 ms', calib: 1.000, pinging: false },
                { id: 'MU-03', ip: '192.168.1.103', mac: '00:50:C2:88:99:A3', status: 'Disconnected', rate: 0, dropped: 8563, rtt: '--', calib: 1.000, pinging: false },
                { id: 'MU-04', ip: '192.168.1.104', mac: '00:50:C2:88:99:A4', status: 'Degraded', rate: 0, dropped: 12, rtt: '9 ms', calib: 1.000, pinging: false },
                { id: 'MU-05', ip: '192.168.1.105', mac: '00:50:C2:88:99:A5', status: 'Healthy', rate: 4800, dropped: 0, rtt: '2 ms', calib: 1.000, pinging: false },
                { id: 'MU-06', ip: '192.168.1.106', mac: '00:50:C2:88:99:A6', status: 'Healthy', rate: 4000, dropped: 0, rtt: '4 ms', calib: 1.000, pinging: false }
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
                    return matchesSearch && matchesStatus;
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
            calibrateMu(id) {
                const mu = this.mus.find(m => m.id === id);
                if (!mu) return;
                alert('Calibration offset of ' + id + ' set to ' + mu.calib);
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
            // Summary header
            div class="glass-card" {
                div class="card-header flex items-center gap-2" {
                    span class="card-icon" {
                        svg class="w-4 h-4 text-accent-blue" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" {
                            path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" {}
                        }
                    }
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
                
                div class="search-box w-full md:w-64" {
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
                                input type="checkbox" x-on:click="toggleSelectAll()" x-bind:checked="selectedMus.length === filteredMus().length && filteredMus().length > 0";
                            }
                            th { "MU ID" }
                            th { "Status" }
                            th { "IP Address" }
                            th { "MAC Address" }
                            th { "Sample Rate" }
                            th { "Dropped" }
                            th { "Latency" }
                            th { "Calib. Factor" }
                            th class="w-48" { "Actions" }
                        }
                    }
                    tbody {
                        template x-for="mu in filteredMus()" x-bind:key="mu.id" {
                            tr x-bind:class="selectedMus.includes(mu.id) ? 'row-selected' : ''" {
                                td class="text-center" {
                                    input type="checkbox" x-bind:value="mu.id" x-model="selectedMus";
                                }
                                td class="font-semibold" x-text="mu.id" {}
                                td {
                                    span class="status-badge" x-bind:class="mu.status === 'Healthy' ? 'status-badge-healthy' : (mu.status === 'Degraded' ? 'status-badge-degraded' : 'status-badge-fault')" {
                                        span class="status-dot-pulse" {}
                                        span x-text="mu.status" {}
                                    }
                                }
                                td class="font-mono" x-text="mu.ip" {}
                                td class="font-mono text-xs" x-text="mu.mac" {}
                                td class="font-semibold text-accent-blue" x-text="mu.rate > 0 ? mu.rate + ' sps' : '0 sps'" {}
                                td class="font-semibold" x-bind:class="mu.dropped > 0 ? 'text-accent-red' : 'text-text-primary'" x-text="mu.dropped" {}
                                td class="font-mono text-accent-green" x-text="mu.rtt" {}
                                td {
                                    input type="number" step="0.001" min="0.5" max="2.0" class="mini-num-input" x-model="mu.calib";
                                }
                                td {
                                    div class="flex gap-2" {
                                        button x-on:click="pingMu(mu.id)" class="btn-primary py-1 px-2 text-[11px] flex items-center gap-1" {
                                            span class="btn-spinner" x-show="mu.pinging" {}
                                            span x-text="mu.pinging ? 'Pinging...' : 'Ping'" {}
                                        }
                                        button x-on:click="calibrateMu(mu.id)" class="btn-primary py-1 px-2 text-[11px] bg-accent-green hover:bg-[#047857]" {
                                            "Calibrate"
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

