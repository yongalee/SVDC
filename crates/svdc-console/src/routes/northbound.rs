/* SVDC Northbound Adapters Router
   OWNER: claude-code (Shared / Extended by Antigravity under WBS-9.4b)
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use axum::{
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Form, Router,
};
use maud::html;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::templates::base;

// Thread-safe atomic states to persist user toggles across dashboard sessions
static L0_ENABLED: AtomicBool = AtomicBool::new(true);
static L1_ENABLED: AtomicBool = AtomicBool::new(true);
static L2_ENABLED: AtomicBool = AtomicBool::new(false);
static L3_ENABLED: AtomicBool = AtomicBool::new(true);

// L0 Settings
static L0_PATH: OnceLock<Mutex<String>> = OnceLock::new();
static L0_BUFFER_SIZE: Mutex<u32> = Mutex::new(65536);
static L0_LOCK_MODE: AtomicBool = AtomicBool::new(true);
static L0_SYNC_MODE: OnceLock<Mutex<String>> = OnceLock::new();

fn get_l0_path() -> &'static Mutex<String> {
    L0_PATH.get_or_init(|| Mutex::new("/dev/shm/svdc_l0_ring".to_string()))
}
fn get_l0_sync_mode() -> &'static Mutex<String> {
    L0_SYNC_MODE.get_or_init(|| Mutex::new("SPSC Lock-Free".to_string()))
}

// L1 Settings
static L1_ADDRESS: OnceLock<Mutex<String>> = OnceLock::new();
static L1_NAMESPACE: OnceLock<Mutex<String>> = OnceLock::new();
static L1_SECURITY: OnceLock<Mutex<String>> = OnceLock::new();
static L1_MAX_SESSIONS: Mutex<u32> = Mutex::new(10);
static L1_PUB_INTERVAL: Mutex<u32> = Mutex::new(100);

fn get_l1_address() -> &'static Mutex<String> {
    L1_ADDRESS.get_or_init(|| Mutex::new("opc.tcp://127.0.0.1:4840/free/svdc/server".to_string()))
}
fn get_l1_namespace() -> &'static Mutex<String> {
    L1_NAMESPACE.get_or_init(|| Mutex::new("http://shinsung.co.kr/svdc/opcua/".to_string()))
}
fn get_l1_security() -> &'static Mutex<String> {
    L1_SECURITY.get_or_init(|| Mutex::new("None".to_string()))
}

// L2 Settings
static L2_BROKER: OnceLock<Mutex<String>> = OnceLock::new();
static L2_TOPIC: OnceLock<Mutex<String>> = OnceLock::new();
static L2_QOS: Mutex<u32> = Mutex::new(1);
static L2_KEEP_ALIVE: Mutex<u32> = Mutex::new(60);
static L2_CLEAN_SESSION: AtomicBool = AtomicBool::new(true);
static L2_PUBLISH_RATE: Mutex<u32> = Mutex::new(10);

fn get_l2_broker() -> &'static Mutex<String> {
    L2_BROKER.get_or_init(|| Mutex::new("mqtt://broker.hivemq.com:1883".to_string()))
}
fn get_l2_topic() -> &'static Mutex<String> {
    L2_TOPIC.get_or_init(|| Mutex::new("ssiec/svdc/telemetry".to_string()))
}

// L3 Settings
static L3_CONN_STRING: OnceLock<Mutex<String>> = OnceLock::new();
static L3_TARGET_TABLE: OnceLock<Mutex<String>> = OnceLock::new();
static L3_BATCH_SIZE: Mutex<u32> = Mutex::new(1000);
static L3_DELAY_LIMIT: Mutex<u32> = Mutex::new(50);
static L3_RETENTION_DAYS: Mutex<u32> = Mutex::new(30);
static L3_POOL_SIZE: Mutex<u32> = Mutex::new(10);

fn get_l3_conn_string() -> &'static Mutex<String> {
    L3_CONN_STRING.get_or_init(|| {
        Mutex::new("postgresql://svdc_user:pass@127.0.0.1:5432/svdc_archive".to_string())
    })
}
fn get_l3_target_table() -> &'static Mutex<String> {
    L3_TARGET_TABLE.get_or_init(|| Mutex::new("sampled_values".to_string()))
}

// Settings Deserializer Structs
#[derive(Deserialize, Debug)]
pub struct L0Form {
    path: String,
    buffer_size: u32,
    lock_mode: Option<String>,
    sync_mode: String,
}

#[derive(Deserialize, Debug)]
pub struct L1Form {
    address: String,
    namespace: String,
    security: String,
    max_sessions: u32,
    pub_interval: u32,
}

#[derive(Deserialize, Debug)]
pub struct L2Form {
    broker: String,
    topic: String,
    qos: u32,
    keep_alive: u32,
    clean_session: Option<String>,
    pub_rate: u32,
}

#[derive(Deserialize, Debug)]
pub struct L3Form {
    conn_string: String,
    target_table: String,
    batch_size: u32,
    delay_limit: u32,
    retention_days: u32,
    pool_size: u32,
}

/// Register routes related to northbound adapters controls and API
pub fn register(router: Router) -> Router {
    router
        .route("/north", get(northbound_page))
        .route("/north/:layer", get(adapter_detail_page))
        .route("/api/v1/northbound/:layer/toggle", post(toggle_adapter))
        .route("/api/v1/northbound/l0/save", post(save_l0_settings))
        .route("/api/v1/northbound/l1/save", post(save_l1_settings))
        .route("/api/v1/northbound/l2/save", post(save_l2_settings))
        .route("/api/v1/northbound/l3/save", post(save_l3_settings))
}

/// Renders the Northbound Controls list page
async fn northbound_page() -> Html<String> {
    let l0_active = L0_ENABLED.load(Ordering::Relaxed);
    let l1_active = L1_ENABLED.load(Ordering::Relaxed);
    let l2_active = L2_ENABLED.load(Ordering::Relaxed);
    let l3_active = L3_ENABLED.load(Ordering::Relaxed);

    let l0_path = get_l0_path().lock().unwrap().clone();
    let l1_addr = get_l1_address().lock().unwrap().clone();
    let l2_broker = get_l2_broker().lock().unwrap().clone();
    let l3_conn = get_l3_conn_string().lock().unwrap().clone();

    let x_data_str = format!(
        "{{
            searchQuery: '',
            statusFilter: 'all',
            selectedAdapters: [],
            adapters: [
                {{ id: 'L0', name: 'Shared Memory RingBuffer', endpoint: '{}', consumers: 3, throughput: 4000, active: {} }},
                {{ id: 'L1', name: 'SCADA OPC UA Server', endpoint: '{}', consumers: 2, throughput: 4000, active: {} }},
                {{ id: 'L2', name: 'MQTT Cloud Publisher', endpoint: '{}', consumers: 1, throughput: 4000, active: {} }},
                {{ id: 'L3', name: 'TimescaleDB Sidecar', endpoint: '{}', consumers: 1, throughput: 4000, active: {} }}
            ],
            toggleSelectAll() {{
                const filtered = this.filteredAdapters();
                if (this.selectedAdapters.length === filtered.length) {{
                    this.selectedAdapters = [];
                }} else {{
                    this.selectedAdapters = filtered.map(a => a.id);
                }}
            }},
            filteredAdapters() {{
                return this.adapters.filter(a => {{
                    const query = this.searchQuery.toLowerCase();
                    const matchesSearch = a.id.toLowerCase().includes(query) ||
                                          a.name.toLowerCase().includes(query) ||
                                          a.endpoint.toLowerCase().includes(query);
                    const statusStr = a.active ? 'active' : 'inactive';
                    const matchesStatus = this.statusFilter === 'all' || statusStr === this.statusFilter;
                    return matchesSearch ? matchesStatus : false;
                }});
            }}
        }}",
        l0_path, l0_active,
        l1_addr, l1_active,
        l2_broker, l2_active,
        l3_conn, l3_active
    );

    let content = html! {
        div x-data=(x_data_str)
        class="screen-layout flex flex-col gap-6" {
            // High-level explanation block
            div class="glass-card shadow-sm" {
                div class="card-header flex items-center gap-2" {
                    h2 class="card-title" { "Northbound Adapters Grid Console" }
                }
                div class="card-body mt-2 text-sm text-text-secondary" {
                    p {
                        "The northbound adapters layer exposes calibrated, aligned telemetry streams "
                        "to all node-local and enterprise operational applications. "
                        "Each layer serves a distinct communication architecture, and can be dynamically "
                        "enabled, disabled, or isolated to optimize compute resources and network overhead."
                    }
                }
            }

            // Grid Controls (Search and Filters)
            div class="flex flex-col md:flex-row gap-4 items-center justify-between" {
                div class="flex items-center gap-2 w-full md:w-auto" {
                    span class="text-xs font-semibold text-text-secondary uppercase tracking-wider" { "Filters:" }
                    div class="filter-chip-group" {
                        button class="filter-chip" x-bind:class="statusFilter === 'all' ? 'active' : ''" x-on:click="statusFilter = 'all'" { "All Adapters" }
                        button class="filter-chip" x-bind:class="statusFilter === 'active' ? 'active' : ''" x-on:click="statusFilter = 'active'" { "Active" }
                        button class="filter-chip" x-bind:class="statusFilter === 'inactive' ? 'active' : ''" x-on:click="statusFilter = 'inactive'" { "Inactive" }
                    }
                }

                div class="search-box w-full md:w-64" style="max-width: 300px;" {
                    input type="text" placeholder="Search by ID, Protocol, URL..." class="w-full text-xs" x-model="searchQuery";
                }
            }

            // High-Density Data Grid
            div class="bg-bg-secondary rounded-lg border border-border-color p-2 overflow-x-auto shadow-sm" {
                 table class="industrial-grid" {
                    thead {
                        tr {
                            th class="w-12 text-center" {
                                input type="checkbox" x-on:click="toggleSelectAll()" x-bind:checked="filteredAdapters().length > 0 ? selectedAdapters.length === filteredAdapters().length : false";
                            }
                            th class="text-center" { "Layer ID" }
                            th class="text-center" { "Protocol Name" }
                            th class="text-center" { "Active Clients" }
                            th class="text-center" { "Throughput" }
                            th class="text-center" { "Endpoint Destination / Address" }
                            th class="text-center" { "Status" }
                            th class="text-center" { "Adapter Enable Action" }
                            th class="w-32 text-center" { "Actions" }
                        }
                    }
                    tbody {
                        template x-for="a in filteredAdapters()" x-bind:key="a.id" {
                            tr class="cursor-pointer hover:bg-bg-surface"
                               x-bind:class="selectedAdapters.includes(a.id) ? 'row-selected' : ''"
                               x-on:click="if ($event.target.closest('input') || $event.target.closest('a') || $event.target.closest('.switch-container')) return; window.location.href = '/north/' + a.id.toLowerCase()" {
                                td class="text-center" {
                                    input type="checkbox" x-bind:value="a.id" x-model="selectedAdapters";
                                }
                                td class="font-bold text-accent-blue text-center" x-text="a.id" {}
                                td class="font-semibold text-text-primary text-center" x-text="a.name" {}
                                td class="font-semibold text-center" x-text="a.active ? a.consumers : 0" {}
                                td class="font-semibold text-accent-blue text-center" x-text="a.active ? a.throughput + ' fps' : '0 fps'" {}
                                td class="font-mono text-xs text-text-secondary text-center" x-text="a.endpoint" {}
                                td class="text-center" {
                                    div class="flex justify-center" {
                                        span class="status-badge" x-bind:class="a.active ? 'status-badge-healthy' : 'status-badge-fault'" {
                                            span class="status-dot-pulse" {}
                                            span x-text="a.active ? 'Active' : 'Inactive'" {}
                                        }
                                    }
                                }
                                td class="text-center" {
                                    div class="flex justify-center" {
                                        label class="switch-container" {
                                            input type="checkbox"
                                                   x-bind:checked="a.active"
                                                   x-on:click="
                                                        fetch('/api/v1/northbound/' + a.id.toLowerCase() + '/toggle', { method: 'POST' })
                                                            .then(() => { a.active = !a.active; });
                                                   "
                                                   class="switch-input";
                                            span class="switch-slider" {}
                                        }
                                    }
                                }
                                td class="text-center" {
                                    div class="flex justify-center" {
                                        a x-bind:href="'/north/' + a.id.toLowerCase()" class="btn-primary py-1 px-2 text-[11px] bg-accent-blue hover:bg-[#1d4ed8] text-center" {
                                            "Settings"
                                        }
                                    }
                                }
                            }
                        }
                        tr x-show="filteredAdapters().length === 0" {
                            td colspan="9" class="text-center text-text-muted py-6 font-medium" {
                                "No Northbound Adapters found matching current search and filter criteria."
                            }
                        }
                    }
                }
            }
        }
    };

    let rendered = base::layout("Northbound Controls", "northbound", content);
    Html(rendered.into_string())
}

/// Renders the individual northbound adapter configurations detail page
async fn adapter_detail_page(Path(layer): Path<String>) -> Html<String> {
    let layer_lower = layer.to_lowercase();
    let layer_upper = layer.to_uppercase();

    if !matches!(layer_lower.as_str(), "l0" | "l1" | "l2" | "l3") {
        return Html(format!("<h1>Layer {} not found</h1>", layer));
    }

    let content = match layer_lower.as_str() {
        "l0" => {
            let path = get_l0_path().lock().unwrap().clone();
            let buffer_size = *L0_BUFFER_SIZE.lock().unwrap();
            let lock_mode = L0_LOCK_MODE.load(Ordering::Relaxed);
            let sync_mode = get_l0_sync_mode().lock().unwrap().clone();
            let is_enabled = L0_ENABLED.load(Ordering::Relaxed);

            html! {
                div x-data=(format!("{{
                    path: '{}',
                    bufferSize: {},
                    lockMode: {},
                    syncMode: '{}',
                    saving: false,
                    progress: 0,
                    showToast: false,
                    toastMsg: '',
                    writeConfiguration() {{
                        if (!this.path || this.path.trim() === '') {{
                            this.toastMsg = 'Error: Path cannot be empty';
                            this.showToast = true;
                            setTimeout(() => {{ this.showToast = false; }}, 4000);
                            return;
                        }}
                        if (this.saving) return;
                        this.saving = true;
                        this.progress = 0;
                        let interval = setInterval(() => {{
                            this.progress += 10;
                            if (this.progress >= 100) {{
                                clearInterval(interval);
                                let formData = new URLSearchParams();
                                formData.append('path', this.path);
                                formData.append('buffer_size', this.bufferSize);
                                if (this.lockMode) formData.append('lock_mode', 'on');
                                formData.append('sync_mode', this.syncMode);
                                
                                fetch('/api/v1/northbound/l0/save', {{
                                    method: 'POST',
                                    headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                                    body: formData
                                }}).then(response => {{
                                    if (!response.ok) throw new Error('Server returned ' + response.status);
                                    this.saving = false;
                                    this.toastMsg = 'L0 Shared Memory RingBuffer configuration saved successfully.';
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }}).catch(err => {{
                                    this.saving = false;
                                    this.toastMsg = 'Failed to save: ' + err.message;
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }});
                            }}
                        }}, 60);
                    }}
                }}", path, buffer_size, lock_mode, sync_mode))
                class="screen-layout flex flex-col gap-6 relative" {

                    div class="fixed bottom-6 right-6 bg-accent-green text-white px-4 py-3 rounded-lg shadow-xl flex items-center gap-2 text-xs font-semibold z-50 transition-all duration-300 transform"
                         x-show="showToast"
                         x-transition {
                        span { "✓" }
                        span x-text="toastMsg" {}
                    }

                    div {
                        a href="/north" class="inline-flex items-center gap-1.5 text-xs text-accent-blue hover:underline font-bold uppercase tracking-wider" {
                            "← Back to Northbound Adapters Console"
                        }
                    }

                    div class="glass-card p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-md" {
                        div {
                            h2 class="text-sm font-bold tracking-tight text-text-primary" {
                                "Northbound Adapter Configurator: L0 Shared Memory RingBuffer"
                            }
                            p class="text-text-secondary text-[11px] mt-1" {
                                "In-Process High-Speed Lock-Free Circular Buffer Telemetry Ingestion Layer"
                            }
                        }
                        div {
                            span class=(format!("status-badge {}", if is_enabled { "status-badge-healthy" } else { "status-badge-fault" })) {
                                span class="status-dot-pulse" {}
                                (if is_enabled { "Active" } else { "Inactive" })
                            }
                        }
                    }

                    div class="flex flex-col gap-6" {
                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "L0 Shared Memory Configuration Parameters" }
                                }
                                div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs" {
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "Shared Memory Segment Ingest Path" }
                                        input type="text" class="w-full text-xs font-mono" x-model="path";
                                        span class="text-[9px] text-text-secondary" { "Target file descriptor inside the Linux virtual filesystem (/dev/shm) for IPC." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Buffer Saturation Capacity (Frames)" }
                                        select class="w-full text-xs font-mono" x-model="bufferSize" {
                                            option value="16384" { "16384 frames" }
                                            option value="32768" { "32768 frames" }
                                            option value="65536" { "65536 frames" }
                                            option value="131072" { "131072 frames" }
                                        }
                                        span class="text-[9px] text-text-secondary" { "Maximum circular queue capacity allocated in virtual memory segments." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Synchronization Concurrency Model" }
                                        select class="w-full text-xs font-mono" x-model="syncMode" {
                                            option value="SPSC Lock-Free" { "SPSC Lock-Free (Atomic release/acquire)" }
                                            option value="MPSC Mutex" { "MPSC Mutex Protected Queue" }
                                        }
                                        span class="text-[9px] text-text-secondary" { "Low-level concurrency model protecting multi-threaded consumers." }
                                    }
                                    div class="flex flex-col gap-1.5 col-span-2 border-t border-border-color pt-3 mt-1 flex-row items-center justify-between" {
                                        div class="flex flex-col" {
                                            span class="font-medium text-text-primary" { "Memory Lock Mode (MLOCK)" }
                                            span class="text-[9px] text-text-secondary" { "Pins the shared segment memory pages in RAM to prevent physical swap page faults." }
                                        }
                                        label class="switch-container" {
                                            input type="checkbox" x-model="lockMode" class="switch-input";
                                            span class="switch-slider" {}
                                        }
                                    }
                                }
                            }
                        }

                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Active Application Process Subscriptions" }
                                }
                                div class="card-body mt-3 flex flex-col gap-2 text-xs" {
                                    p class="text-text-secondary text-[11px]" {
                                        "Currently active local background processes consuming aligned telemetry from the shared ringbuffer:"
                                    }
                                    div class="overflow-x-auto mt-2" {
                                        table class="industrial-grid" {
                                            thead {
                                                tr {
                                                    th class="text-center" { "PID" }
                                                    th class="text-center" { "Process Name" }
                                                    th class="text-center" { "Lag" }
                                                    th class="text-center" { "Sat. %" }
                                                }
                                            }
                                            tbody {
                                                tr {
                                                    td class="font-mono text-[10px] text-center" { "4092" }
                                                    td class="font-semibold text-center" { "ebp_protection" }
                                                    td class="font-mono text-accent-green text-center" { "2 μs" }
                                                    td class="font-mono text-text-secondary text-center" { "0.1%" }
                                                }
                                                tr {
                                                    td class="font-mono text-[10px] text-center" { "5122" }
                                                    td class="font-semibold text-center" { "pcm_phasor" }
                                                    td class="font-mono text-accent-green text-center" { "8 μs" }
                                                    td class="font-mono text-text-secondary text-center" { "0.3%" }
                                                }
                                                tr {
                                                    td class="font-mono text-[10px] text-center" { "7180" }
                                                    td class="font-semibold text-center" { "fault_locator" }
                                                    td class="font-mono text-accent-green text-center" { "12 μs" }
                                                    td class="font-mono text-text-secondary text-center" { "0.4%" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Write & Commit Configuration" }
                                }
                                div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                                    p class="text-text-secondary leading-relaxed" {
                                        "Committing parameters triggers an atomic IPC ringbuffer re-alignment. Local application queues will momentarily buffer data during segment re-mapping."
                                    }

                                    div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                        div class="flex justify-between text-[10px]" {
                                            span class="text-text-secondary" { "Re-mapping shared memory segment..." }
                                            span class="font-bold text-accent-blue" x-text="progress + '%'" {}
                                        }
                                        div class="progressbar-bg h-2 rounded overflow-hidden" {
                                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-100"
                                                 x-bind:style="'width: ' + progress + '%'" {}
                                        }
                                    }

                                    button class="btn-primary w-full py-2 flex items-center justify-center gap-2 font-bold uppercase tracking-wider text-xs"
                                            x-bind:disabled="saving"
                                            x-on:click="writeConfiguration()" {
                                        span class="btn-spinner" x-show="saving" {}
                                        span x-text="saving ? 'Committing parameters...' : 'Write Configuration to Engine'" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "l1" => {
            let address = get_l1_address().lock().unwrap().clone();
            let namespace = get_l1_namespace().lock().unwrap().clone();
            let security = get_l1_security().lock().unwrap().clone();
            let max_sessions = *L1_MAX_SESSIONS.lock().unwrap();
            let pub_interval = *L1_PUB_INTERVAL.lock().unwrap();
            let is_enabled = L1_ENABLED.load(Ordering::Relaxed);

            html! {
                div x-data=(format!("{{
                    address: '{}',
                    namespace: '{}',
                    security: '{}',
                    maxSessions: {},
                    pubInterval: {},
                    saving: false,
                    progress: 0,
                    showToast: false,
                    toastMsg: '',
                    writeConfiguration() {{
                        if (!this.address || this.address.trim() === '') {{
                            this.toastMsg = 'Error: Server Bind Address cannot be empty';
                            this.showToast = true;
                            setTimeout(() => {{ this.showToast = false; }}, 4000);
                            return;
                        }}
                        if (!confirm('Applying new endpoints will cause active SCADA sessions to restart. Proceed?')) {{
                            return;
                        }}
                        if (this.saving) return;
                        this.saving = true;
                        this.progress = 0;
                        let interval = setInterval(() => {{
                            this.progress += 10;
                            if (this.progress >= 100) {{
                                clearInterval(interval);
                                let formData = new URLSearchParams();
                                formData.append('address', this.address);
                                formData.append('namespace', this.namespace);
                                formData.append('security', this.security);
                                formData.append('max_sessions', this.maxSessions);
                                formData.append('pub_interval', this.pubInterval);
                                
                                fetch('/api/v1/northbound/l1/save', {{
                                    method: 'POST',
                                    headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                                    body: formData
                                }}).then(response => {{
                                    if (!response.ok) throw new Error('Server returned ' + response.status);
                                    this.saving = false;
                                    this.toastMsg = 'L1 OPC UA Server configuration saved successfully.';
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }}).catch(err => {{
                                    this.saving = false;
                                    this.toastMsg = 'Failed to save: ' + err.message;
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }});
                            }}
                        }}, 60);
                    }}
                }}", address, namespace, security, max_sessions, pub_interval))
                class="screen-layout flex flex-col gap-6 relative" {

                    div class="fixed bottom-6 right-6 bg-accent-green text-white px-4 py-3 rounded-lg shadow-xl flex items-center gap-2 text-xs font-semibold z-50 transition-all duration-300 transform"
                         x-show="showToast"
                         x-transition {
                        span { "✓" }
                        span x-text="toastMsg" {}
                    }

                    div {
                        a href="/north" class="inline-flex items-center gap-1.5 text-xs text-accent-blue hover:underline font-bold uppercase tracking-wider" {
                            "← Back to Northbound Adapters Console"
                        }
                    }

                    div class="glass-card p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-md" {
                        div {
                            h2 class="text-sm font-bold tracking-tight text-text-primary" {
                                "Northbound Adapter Configurator: L1 SCADA OPC UA Server"
                            }
                            p class="text-text-secondary text-[11px] mt-1" {
                                "IEC 62541 Industry-Standard Interoperable SCADA Integration Service Layer"
                            }
                        }
                        div {
                            span class=(format!("status-badge {}", if is_enabled { "status-badge-healthy" } else { "status-badge-fault" })) {
                                span class="status-dot-pulse" {}
                                (if is_enabled { "Active" } else { "Inactive" })
                            }
                        }
                    }

                    div class="flex flex-col gap-6" {
                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "L1 OPC UA Server Settings" }
                                }
                                div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs" {
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "Server Bind Address URL" }
                                        input type="text" class="w-full text-xs font-mono" x-model="address";
                                        span class="text-[9px] text-text-secondary" { "The TCP network endpoint bind address for SCADA client connections." }
                                    }
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "Namespace URI Schema" }
                                        input type="text" class="w-full text-xs font-mono" x-model="namespace";
                                        span class="text-[9px] text-text-secondary" { "URI schema identifying the SVDC AddressSpace namespace context." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Minimum Cryptographic Security Policy" }
                                        select class="w-full text-xs font-mono" x-model="security" {
                                            option value="None" { "None (Unencrypted plaintext)" }
                                            option value="Sign" { "Basic256Sha256 - Sign" }
                                            option value="SignAndEncrypt" { "Basic256Sha256 - Sign & Encrypt" }
                                        }
                                        span class="text-[9px] text-text-secondary" { "Enforces standard cryptographic verification of client connection requests." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Publish Interval Margin (ms)" }
                                        input type="number" class="w-full text-xs font-mono" x-model="pubInterval";
                                        span class="text-[9px] text-text-secondary" { "Telemetry publishing cycle frequency to SCADA client sessions." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Maximum Connected Concurrent Sessions" }
                                        input type="number" class="w-full text-xs font-mono" x-model="maxSessions";
                                        span class="text-[9px] text-text-secondary" { "Soft limit on concurrent HMI or developer UA client connections." }
                                    }
                                }
                            }
                        }

                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "SCADA Active Sessions Diagnostics" }
                                }
                                div class="card-body mt-3 flex flex-col gap-2 text-xs" {
                                    p class="text-text-secondary text-[11px]" {
                                        "Active client connections monitoring on OPC UA TCP endpoint:"
                                    }
                                    div class="overflow-x-auto mt-2" {
                                        table class="industrial-grid" {
                                            thead {
                                                tr {
                                                    th class="text-center" { "Client IP" }
                                                    th class="text-center" { "Session ID" }
                                                    th class="text-center" { "Sub. Nodes" }
                                                }
                                            }
                                            tbody {
                                                tr {
                                                    td class="font-mono text-[10px] text-center" { "192.168.1.50" }
                                                    td class="font-semibold text-center" { "SCADA_HMI_01" }
                                                    td class="font-semibold text-accent-blue text-center" { "42 nodes" }
                                                }
                                                tr {
                                                    td class="font-mono text-[10px] text-center" { "192.168.1.52" }
                                                    td class="font-semibold text-center" { "UaExpert_Verifier" }
                                                    td class="font-semibold text-accent-blue text-center" { "12 nodes" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Write & Commit Configuration" }
                                }
                                div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                                    p class="text-text-secondary leading-relaxed" {
                                        "Applying new endpoints will cause active SCADA sessions to restart. Connection failovers will trigger automatically."
                                    }

                                    div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                        div class="flex justify-between text-[10px]" {
                                            span class="text-text-secondary" { "Restarting OPC UA Server stack..." }
                                            span class="font-bold text-accent-blue" x-text="progress + '%'" {}
                                        }
                                        div class="progressbar-bg h-2 rounded overflow-hidden" {
                                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-100"
                                                 x-bind:style="'width: ' + progress + '%'" {}
                                        }
                                    }

                                    button class="btn-primary w-full py-2 flex items-center justify-center gap-2 font-bold uppercase tracking-wider text-xs"
                                            x-bind:disabled="saving"
                                            x-on:click="writeConfiguration()" {
                                        span class="btn-spinner" x-show="saving" {}
                                        span x-text="saving ? 'Committing parameters...' : 'Write Configuration to Engine'" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "l2" => {
            let broker = get_l2_broker().lock().unwrap().clone();
            let topic = get_l2_topic().lock().unwrap().clone();
            let qos = *L2_QOS.lock().unwrap();
            let keep_alive = *L2_KEEP_ALIVE.lock().unwrap();
            let clean_session = L2_CLEAN_SESSION.load(Ordering::Relaxed);
            let pub_rate = *L2_PUBLISH_RATE.lock().unwrap();
            let is_enabled = L2_ENABLED.load(Ordering::Relaxed);

            html! {
                div x-data=(format!("{{
                    broker: '{}',
                    topic: '{}',
                    qos: {},
                    keepAlive: {},
                    cleanSession: {},
                    pubRate: {},
                    saving: false,
                    progress: 0,
                    showToast: false,
                    toastMsg: '',
                    writeConfiguration() {{
                        if (!this.broker || this.broker.trim() === '') {{
                            this.toastMsg = 'Error: Broker URL cannot be empty';
                            this.showToast = true;
                            setTimeout(() => {{ this.showToast = false; }}, 4000);
                            return;
                        }}
                        if (this.saving) return;
                        this.saving = true;
                        this.progress = 0;
                        let interval = setInterval(() => {{
                            this.progress += 10;
                            if (this.progress >= 100) {{
                                clearInterval(interval);
                                let formData = new URLSearchParams();
                                formData.append('broker', this.broker);
                                formData.append('topic', this.topic);
                                formData.append('qos', this.qos);
                                formData.append('keep_alive', this.keepAlive);
                                if (this.cleanSession) formData.append('clean_session', 'on');
                                formData.append('pub_rate', this.pubRate);
                                
                                fetch('/api/v1/northbound/l2/save', {{
                                    method: 'POST',
                                    headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                                    body: formData
                                }}).then(response => {{
                                    if (!response.ok) throw new Error('Server returned ' + response.status);
                                    this.saving = false;
                                    this.toastMsg = 'L2 MQTT Cloud Publisher configuration saved successfully.';
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }}).catch(err => {{
                                    this.saving = false;
                                    this.toastMsg = 'Failed to save: ' + err.message;
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }});
                            }}
                        }}, 60);
                    }}
                }}", broker, topic, qos, keep_alive, clean_session, pub_rate))
                class="screen-layout flex flex-col gap-6 relative" {

                    div class="fixed bottom-6 right-6 bg-accent-green text-white px-4 py-3 rounded-lg shadow-xl flex items-center gap-2 text-xs font-semibold z-50 transition-all duration-300 transform"
                         x-show="showToast"
                         x-transition {
                        span { "✓" }
                        span x-text="toastMsg" {}
                    }

                    div {
                        a href="/north" class="inline-flex items-center gap-1.5 text-xs text-accent-blue hover:underline font-bold uppercase tracking-wider" {
                            "← Back to Northbound Adapters Console"
                        }
                    }

                    div class="glass-card p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-md" {
                        div {
                            h2 class="text-sm font-bold tracking-tight text-text-primary" {
                                "Northbound Adapter Configurator: L2 MQTT Cloud Publisher"
                            }
                            p class="text-text-secondary text-[11px] mt-1" {
                                "Low-overhead Pub/Sub Telemetry Connector for Distributed Substation Fleets"
                            }
                        }
                        div {
                            span class=(format!("status-badge {}", if is_enabled { "status-badge-healthy" } else { "status-badge-fault" })) {
                                span class="status-dot-pulse" {}
                                (if is_enabled { "Active" } else { "Inactive" })
                            }
                        }
                    }

                    div class="flex flex-col gap-6" {
                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "L2 MQTT Publisher settings" }
                                }
                                div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs" {
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "MQTT Cloud Broker Address URL" }
                                        input type="text" class="w-full text-xs font-mono" x-model="broker";
                                        span class="text-[9px] text-text-secondary" { "Standard protocol target broker URI (tcp/mqtt schemes, port 1883)." }
                                    }
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "MQTT Publishing Topic Namespace Prefix" }
                                        input type="text" class="w-full text-xs font-mono" x-model="topic";
                                        span class="text-[9px] text-text-secondary" { "The structured target topic path hierarchy where telemetry is pushed." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Quality of Service (QoS) Level" }
                                        select class="w-full text-xs font-mono" x-model="qos" {
                                            option value="0" { "QoS 0 - At most once (Fastest)" }
                                            option value="1" { "QoS 1 - At least once (Guaranteed)" }
                                            option value="2" { "QoS 2 - Exactly once (Transaction)" }
                                        }
                                        span class="text-[9px] text-text-secondary" { "Ensures message reliability limits over lossy wide-area backhauls." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Keep-Alive Interval (Seconds)" }
                                        input type="number" class="w-full text-xs font-mono" x-model="keepAlive";
                                        span class="text-[9px] text-text-secondary" { "MQTT connection ping frequency to maintain active broker bindings." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Publish Throttle Rate (Hz)" }
                                        select class="w-full text-xs font-mono" x-model="pubRate" {
                                            option value="1" { "1 Hz (Diagnostic)" }
                                            option value="5" { "5 Hz (Standard)" }
                                            option value="10" { "10 Hz (Dense)" }
                                            option value="50" { "50 Hz (High Frequency)" }
                                        }
                                        span class="text-[9px] text-text-secondary" { "Sample rates downsampled from the process bus frequency." }
                                    }
                                    div class="flex flex-col gap-1.5 border-t border-border-color pt-3 mt-1 flex-row items-center justify-between" {
                                        div class="flex flex-col" {
                                            span class="font-medium text-text-primary" { "Clean Session Protocol" }
                                            span class="text-[9px] text-text-secondary" { "Discards outstanding client transactions on broker reconnects." }
                                        }
                                        label class="switch-container" {
                                            input type="checkbox" x-model="cleanSession" class="switch-input";
                                            span class="switch-slider" {}
                                        }
                                    }
                                }
                            }
                        }

                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Cloud Gateway Sync Log feed" }
                                }
                                div class="card-body mt-3 flex flex-col gap-2 text-xs" {
                                    p class="text-text-secondary text-[11px]" {
                                        "Live MQTT publisher events and sync statuses:"
                                    }
                                    pre class="logs-terminal h-32 mt-2 font-mono text-[9px] leading-relaxed select-text p-2" {
                                        "[2026-05-21 15:25:02] Connected to MQTT Broker."
                                        "\n[2026-05-21 15:25:03] Published channel state Va: 1.020, Ia: 1.010"
                                        "\n[2026-05-21 15:25:10] Keep-Alive ping sent (RTT: 42ms)"
                                        "\n[2026-05-21 15:25:13] Published channel state Vb: 1.018, Ib: 1.008"
                                    }
                                }
                            }

                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Write & Commit Configuration" }
                                }
                                div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                                    p class="text-text-secondary leading-relaxed" {
                                        "Committing configuration reconnects the MQTT publisher stack. Topic structures will refresh upon next publishing interval."
                                    }

                                    div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                        div class="flex justify-between text-[10px]" {
                                            span class="text-text-secondary" { "Re-establishing broker subscription..." }
                                            span class="font-bold text-accent-blue" x-text="progress + '%'" {}
                                        }
                                        div class="progressbar-bg h-2 rounded overflow-hidden" {
                                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-100"
                                                 x-bind:style="'width: ' + progress + '%'" {}
                                        }
                                    }

                                    button class="btn-primary w-full py-2 flex items-center justify-center gap-2 font-bold uppercase tracking-wider text-xs"
                                            x-bind:disabled="saving"
                                            x-on:click="writeConfiguration()" {
                                        span class="btn-spinner" x-show="saving" {}
                                        span x-text="saving ? 'Committing parameters...' : 'Write Configuration to Engine'" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "l3" => {
            let conn_string = get_l3_conn_string().lock().unwrap().clone();
            let target_table = get_l3_target_table().lock().unwrap().clone();
            let batch_size = *L3_BATCH_SIZE.lock().unwrap();
            let delay_limit = *L3_DELAY_LIMIT.lock().unwrap();
            let retention_days = *L3_RETENTION_DAYS.lock().unwrap();
            let pool_size = *L3_POOL_SIZE.lock().unwrap();
            let is_enabled = L3_ENABLED.load(Ordering::Relaxed);

            html! {
                div x-data=(format!("{{
                    connString: '{}',
                    targetTable: '{}',
                    batchSize: {},
                    delayLimit: {},
                    retentionDays: {},
                    poolSize: {},
                    saving: false,
                    progress: 0,
                    showToast: false,
                    toastMsg: '',
                    writeConfiguration() {{
                        if (!this.connString || this.connString.trim() === '') {{
                            this.toastMsg = 'Error: Connection string cannot be empty';
                            this.showToast = true;
                            setTimeout(() => {{ this.showToast = false; }}, 4000);
                            return;
                        }}
                        if (this.saving) return;
                        this.saving = true;
                        this.progress = 0;
                        let interval = setInterval(() => {{
                            this.progress += 10;
                            if (this.progress >= 100) {{
                                clearInterval(interval);
                                let formData = new URLSearchParams();
                                formData.append('conn_string', this.connString);
                                formData.append('target_table', this.targetTable);
                                formData.append('batch_size', this.batchSize);
                                formData.append('delay_limit', this.delayLimit);
                                formData.append('retention_days', this.retentionDays);
                                formData.append('pool_size', this.poolSize);
                                
                                fetch('/api/v1/northbound/l3/save', {{
                                    method: 'POST',
                                    headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                                    body: formData
                                }}).then(response => {{
                                    if (!response.ok) throw new Error('Server returned ' + response.status);
                                    this.saving = false;
                                    this.toastMsg = 'L3 TimescaleDB Archive configuration saved successfully.';
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }}).catch(err => {{
                                    this.saving = false;
                                    this.toastMsg = 'Failed to save: ' + err.message;
                                    this.showToast = true;
                                    setTimeout(() => {{ this.showToast = false; }}, 4000);
                                }});
                            }}
                        }}, 60);
                    }}
                }}", conn_string, target_table, batch_size, delay_limit, retention_days, pool_size))
                class="screen-layout flex flex-col gap-6 relative" {

                    div class="fixed bottom-6 right-6 bg-accent-green text-white px-4 py-3 rounded-lg shadow-xl flex items-center gap-2 text-xs font-semibold z-50 transition-all duration-300 transform"
                         x-show="showToast"
                         x-transition {
                        span { "✓" }
                        span x-text="toastMsg" {}
                    }

                    div {
                        a href="/north" class="inline-flex items-center gap-1.5 text-xs text-accent-blue hover:underline font-bold uppercase tracking-wider" {
                            "← Back to Northbound Adapters Console"
                        }
                    }

                    div class="glass-card p-4 flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-md" {
                        div {
                            h2 class="text-sm font-bold tracking-tight text-text-primary" {
                                "Northbound Adapter Configurator: L3 TimescaleDB Sidecar Archive"
                            }
                            p class="text-text-secondary text-[11px] mt-1" {
                                "IEC 61850 Historical Archive & High-Performance Time-Series Database Service"
                            }
                        }
                        div {
                            span class=(format!("status-badge {}", if is_enabled { "status-badge-healthy" } else { "status-badge-fault" })) {
                                span class="status-dot-pulse" {}
                                (if is_enabled { "Active" } else { "Inactive" })
                            }
                        }
                    }

                    div class="flex flex-col gap-6" {
                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "L3 TimescaleDB Archive settings" }
                                }
                                div class="card-body mt-4 grid grid-cols-1 md:grid-cols-2 gap-4 text-xs" {
                                    div class="flex flex-col gap-1 col-span-2" {
                                        label class="font-medium text-text-primary" { "Database PostgreSQL Connection String URI" }
                                        input type="text" class="w-full text-xs font-mono" x-model="connString";
                                        span class="text-[9px] text-text-secondary" { "Standard PostgreSQL database URI with authentication credentials." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Target Hypertable Name" }
                                        input type="text" class="w-full text-xs font-mono" x-model="targetTable";
                                        span class="text-[9px] text-text-secondary" { "The partition hypertable inside PostgreSQL storing raw records." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Batch Insert Chunk Size" }
                                        input type="number" class="w-full text-xs font-mono" x-model="batchSize";
                                        span class="text-[9px] text-text-secondary" { "Configures raw records buffered in memory before bulk flushing to database." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Ingestion Delay Limit (ms)" }
                                        input type="number" class="w-full text-xs font-mono" x-model="delayLimit";
                                        span class="text-[9px] text-text-secondary" { "Maximum time database transaction is permitted to wait." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Retention Policy (Days)" }
                                        input type="number" class="w-full text-xs font-mono" x-model="retentionDays";
                                        span class="text-[9px] text-text-secondary" { "Number of days database retains raw time-series data before pruning partitions." }
                                    }
                                    div class="flex flex-col gap-1" {
                                        label class="font-medium text-text-primary" { "Connection Pool Max Size" }
                                        input type="number" class="w-full text-xs font-mono" x-model="poolSize";
                                        span class="text-[9px] text-text-secondary" { "Maximum connections established in PostgreSQL client connection pools." }
                                    }
                                }
                            }
                        }

                        div class="flex flex-col gap-6" {
                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Historical Archiving Live logs" }
                                }
                                div class="card-body mt-3 flex flex-col gap-2 text-xs" {
                                    p class="text-text-secondary text-[11px]" {
                                        "TimescaleDB archiver engine write performance:"
                                    }
                                    pre class="logs-terminal h-32 mt-2 font-mono text-[9px] leading-relaxed select-text p-2" {
                                        "[2026-05-21 15:25:00] Batch write completed: 1000 rows (elapsed: 14ms)"
                                        "\n[2026-05-21 15:25:05] Batch write completed: 1000 rows (elapsed: 12ms)"
                                        "\n[2026-05-21 15:25:10] Retention sweeper: deleted 0 partitions (database within storage bounds)"
                                        "\n[2026-05-21 15:25:15] Batch write completed: 1000 rows (elapsed: 13ms)"
                                    }
                                }
                            }

                            div class="glass-card shadow-md" {
                                div class="card-header border-b border-border-color pb-3" {
                                    h3 class="card-title text-xs uppercase text-text-muted font-bold tracking-wider" { "Write & Commit Configuration" }
                                }
                                div class="card-body mt-4 flex flex-col gap-4 text-xs" {
                                    p class="text-text-secondary leading-relaxed" {
                                        "Applying parameters restarts database connection pools. Tables are auto-migrated if table schemas don't match."
                                    }

                                    div class="w-full flex flex-col gap-1.5" x-show="saving" x-transition {
                                        div class="flex justify-between text-[10px]" {
                                            span class="text-text-secondary" { "Flushing database pools..." }
                                            span class="font-bold text-accent-blue" x-text="progress + '%'" {}
                                        }
                                        div class="progressbar-bg h-2 rounded overflow-hidden" {
                                            div class="progressbar-fill h-full bg-accent-blue transition-all duration-100"
                                                 x-bind:style="'width: ' + progress + '%'" {}
                                        }
                                    }

                                    button class="btn-primary w-full py-2 flex items-center justify-center gap-2 font-bold uppercase tracking-wider text-xs"
                                            x-bind:disabled="saving"
                                            x-on:click="writeConfiguration()" {
                                        span class="btn-spinner" x-show="saving" {}
                                        span x-text="saving ? 'Committing parameters...' : 'Write Configuration to Engine'" {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => unreachable!(),
    };

    let rendered = base::layout(
        &format!("Northbound Adapter - {}", layer_upper),
        "northbound",
        content,
    );
    Html(rendered.into_string())
}

/// Dynamic POST endpoint for enabling/disabling northbound adapters.
/// Triggered via Alpine/HTMX switch toggle and returns "OK".
async fn toggle_adapter(Path(layer): Path<String>) -> Html<String> {
    match layer.as_str() {
        "l0" => {
            let next_state = !L0_ENABLED.load(Ordering::Relaxed);
            L0_ENABLED.store(next_state, Ordering::Relaxed);
        }
        "l1" => {
            let next_state = !L1_ENABLED.load(Ordering::Relaxed);
            L1_ENABLED.store(next_state, Ordering::Relaxed);
        }
        "l2" => {
            let next_state = !L2_ENABLED.load(Ordering::Relaxed);
            L2_ENABLED.store(next_state, Ordering::Relaxed);
        }
        _ => {
            let next_state = !L3_ENABLED.load(Ordering::Relaxed);
            L3_ENABLED.store(next_state, Ordering::Relaxed);
        }
    };
    Html("OK".to_string())
}

// POST Save settings endpoints
async fn save_l0_settings(Form(payload): Form<L0Form>) -> impl IntoResponse {
    if payload.path.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Html("Path cannot be empty".to_string()));
    }
    *get_l0_path().lock().unwrap() = payload.path;
    *L0_BUFFER_SIZE.lock().unwrap() = payload.buffer_size;
    L0_LOCK_MODE.store(payload.lock_mode.is_some(), Ordering::Relaxed);
    *get_l0_sync_mode().lock().unwrap() = payload.sync_mode;
    (StatusCode::OK, Html("OK".to_string()))
}

async fn save_l1_settings(Form(payload): Form<L1Form>) -> impl IntoResponse {
    if payload.address.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Html("Address cannot be empty".to_string()));
    }
    *get_l1_address().lock().unwrap() = payload.address;
    *get_l1_namespace().lock().unwrap() = payload.namespace;
    *get_l1_security().lock().unwrap() = payload.security;
    *L1_MAX_SESSIONS.lock().unwrap() = payload.max_sessions;
    *L1_PUB_INTERVAL.lock().unwrap() = payload.pub_interval;
    (StatusCode::OK, Html("OK".to_string()))
}

async fn save_l2_settings(Form(payload): Form<L2Form>) -> impl IntoResponse {
    if payload.broker.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Html("Broker cannot be empty".to_string()));
    }
    *get_l2_broker().lock().unwrap() = payload.broker;
    *get_l2_topic().lock().unwrap() = payload.topic;
    *L2_QOS.lock().unwrap() = payload.qos;
    *L2_KEEP_ALIVE.lock().unwrap() = payload.keep_alive;
    L2_CLEAN_SESSION.store(payload.clean_session.is_some(), Ordering::Relaxed);
    *L2_PUBLISH_RATE.lock().unwrap() = payload.pub_rate;
    (StatusCode::OK, Html("OK".to_string()))
}

async fn save_l3_settings(Form(payload): Form<L3Form>) -> impl IntoResponse {
    if payload.conn_string.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Html("Connection string cannot be empty".to_string()));
    }
    *get_l3_conn_string().lock().unwrap() = payload.conn_string;
    *get_l3_target_table().lock().unwrap() = payload.target_table;
    *L3_BATCH_SIZE.lock().unwrap() = payload.batch_size;
    *L3_DELAY_LIMIT.lock().unwrap() = payload.delay_limit;
    *L3_RETENTION_DAYS.lock().unwrap() = payload.retention_days;
    *L3_POOL_SIZE.lock().unwrap() = payload.pool_size;
    (StatusCode::OK, Html("OK".to_string()))
}
