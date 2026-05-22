//! `svdc-l1-opcua-client` — reference L1 client simulator
//! (ADR-0016 §6 / PR M).
//!
//! Connects to the SVDC daemon's L1 OPC UA server (PR L+), opens
//! a Subscription on the 8 reference channels, and prints every
//! data-change event to stdout. This is the operator-facing
//! verification companion to `--enable-opcua`: if this binary
//! sees value changes, the L0 → aligner → L1 trail is working.
//!
//! Default endpoint mirrors the server's default
//! (`opc.tcp://127.0.0.1:4840/`). Pass `--endpoint <url>` and
//! `--mu-svid <id>` to point at another deployment.
//!
//! NFR-10: English-only.

use std::sync::Arc;
use std::time::Duration;

use opcua::client::{ClientBuilder, DataChangeCallback, IdentityToken, MonitoredItem};
use opcua::crypto::SecurityPolicy;
use opcua::types::{
    DataValue, MessageSecurityMode, MonitoredItemCreateRequest, NodeId, TimestampsToReturn,
    UserTokenPolicy,
};

/// Default endpoint URL. Matches `svdc-bin --enable-opcua`
/// without arguments (loopback, port 4840).
const DEFAULT_ENDPOINT: &str = "opc.tcp://127.0.0.1:4840/";

/// Default MU svID. Matches `svdc-bin`'s thin-slice fallback
/// when no ingress traffic has been observed yet.
const DEFAULT_MU_SVID: &str = "SVDC_DEMO_PB_MU";

/// SVDC namespace registered by `svdc-opcua::server` as
/// `urn:svdc:l1`. The server is the only crate that registers
/// it, so the index is deterministic (`ns=2` after the standard
/// + server URIs in slots 0 and 1).
const SVDC_NS: u16 = 2;

/// Reference channel layout from `svdc-opcua::REFERENCE_CHANNELS`.
/// Kept as a literal here to avoid pulling the whole `svdc-opcua`
/// crate just for the constant — the canonical ordering only
/// changes alongside an ADR-0017 §2 revision.
const REFERENCE_CHANNELS: &[&str] = &[
    "Ch00_Va", "Ch01_Vb", "Ch02_Vc", "Ch03_Vn", "Ch04_Ia", "Ch05_Ib", "Ch06_Ic", "Ch07_In",
];

struct Config {
    help: bool,
    endpoint: String,
    mu_svid: String,
    /// When `true`, exit cleanly after `samples_to_print` data
    /// changes have been observed. Useful for the runbook smoke
    /// check; production usage runs forever.
    samples_to_print: Option<u64>,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut cfg = Config {
        help: false,
        endpoint: DEFAULT_ENDPOINT.to_string(),
        mu_svid: DEFAULT_MU_SVID.to_string(),
        samples_to_print: None,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => cfg.help = true,
            "--endpoint" => {
                i += 1;
                cfg.endpoint = args
                    .get(i)
                    .cloned()
                    .ok_or_else(|| "--endpoint requires a URL".to_string())?;
            }
            "--mu-svid" => {
                i += 1;
                cfg.mu_svid = args
                    .get(i)
                    .cloned()
                    .ok_or_else(|| "--mu-svid requires an svID".to_string())?;
            }
            "--samples" => {
                i += 1;
                let s = args
                    .get(i)
                    .ok_or_else(|| "--samples requires a count".to_string())?;
                cfg.samples_to_print = Some(
                    s.parse::<u64>()
                        .map_err(|_| format!("--samples: '{s}' is not a number"))?,
                );
            }
            other => return Err(format!("unknown argument '{other}'. Try --help.")),
        }
        i += 1;
    }
    Ok(cfg)
}

fn print_help() {
    println!("svdc-l1-opcua-client (PR M / ADR-0016 §6)");
    println!();
    println!("USAGE:");
    println!("    svdc-l1-opcua-client [FLAGS] [OPTIONS]");
    println!();
    println!("FLAGS:");
    println!("    -h, --help                 Show this help");
    println!();
    println!("OPTIONS:");
    println!("    --endpoint <url>           OPC UA server URL");
    println!("                               [default: {DEFAULT_ENDPOINT}]");
    println!("    --mu-svid <svID>           MU svID published by the SVDC server");
    println!("                               [default: {DEFAULT_MU_SVID}]");
    println!("    --samples <N>              Exit after N data-change events");
    println!("                               (omitted = run until Ctrl-C)");
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if cfg.help {
        print_help();
        return std::process::ExitCode::SUCCESS;
    }

    match run(cfg).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("svdc-l1-opcua-client: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(cfg: Config) -> Result<(), String> {
    println!(
        "svdc-l1-opcua-client: connecting to {} (mu={})",
        cfg.endpoint, cfg.mu_svid
    );

    let mut client = ClientBuilder::new()
        .application_name("SVDC L1 Client Simulator")
        .application_uri("urn:svdc:l1:client")
        .product_uri("urn:svdc:l1:client")
        .trust_server_certs(true)
        .create_sample_keypair(false)
        .session_retry_limit(3)
        .client()
        .map_err(|e| format!("client build failed: {e:?}"))?;

    let (session, event_loop) = client
        .connect_to_matching_endpoint(
            (
                cfg.endpoint.as_str(),
                SecurityPolicy::None.to_str(),
                MessageSecurityMode::None,
                UserTokenPolicy::anonymous(),
            ),
            IdentityToken::Anonymous,
        )
        .await
        .map_err(|e| format!("connect failed: {e:?}"))?;

    let event_handle = event_loop.spawn();
    session.wait_for_connection().await;
    println!("svdc-l1-opcua-client: session established");

    // The samples counter is captured by the callback closure so
    // the loop can exit cleanly when --samples N is set.
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counter_cb = Arc::clone(&counter);
    let target = cfg.samples_to_print;

    let subscription_id = session
        .create_subscription(
            Duration::from_millis(1000),
            10,
            30,
            0,
            0,
            true,
            DataChangeCallback::new(move |dv, item| {
                print_data_change(&dv, item);
                let n = counter_cb.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if let Some(t) = target {
                    if n >= t {
                        eprintln!("svdc-l1-opcua-client: reached --samples={t}; disconnecting");
                        // Disconnect from inside the callback would require
                        // a sync handle; instead we spawn-detach a closer.
                        std::process::exit(0);
                    }
                }
            }),
        )
        .await
        .map_err(|e| format!("create_subscription: {e:?}"))?;
    println!("svdc-l1-opcua-client: subscription id={subscription_id}");

    let items = monitored_item_requests(&cfg.mu_svid);
    let n_items = items.len();
    let _ = session
        .create_monitored_items(subscription_id, TimestampsToReturn::Both, items)
        .await
        .map_err(|e| format!("create_monitored_items: {e:?}"))?;
    println!(
        "svdc-l1-opcua-client: monitoring {n_items} items across {} channels",
        REFERENCE_CHANNELS.len()
    );

    // Graceful Ctrl-C handler: disconnect the session so the
    // server's session table reflects reality immediately.
    let session_c = session.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("svdc-l1-opcua-client: Ctrl-C received; disconnecting");
            let _ = session_c.disconnect().await;
        }
    });

    event_handle
        .await
        .map_err(|e| format!("event loop join: {e:?}"))?;
    Ok(())
}

/// Build one `MonitoredItemCreateRequest` per (channel, attribute)
/// pair. The first slice subscribes to `instMag.i` (raw Q-value)
/// and `q` (IEC 61850 quality) for each of the 8 reference
/// channels — enough to verify the L1 trail without flooding the
/// terminal with every variable type.
fn monitored_item_requests(mu_svid: &str) -> Vec<MonitoredItemCreateRequest> {
    REFERENCE_CHANNELS
        .iter()
        .flat_map(|ch| {
            let base = format!("Substations.Demo.{mu_svid}.ChannelRegistry.{ch}");
            [
                NodeId::new(SVDC_NS, format!("{base}.instMag.i")).into(),
                NodeId::new(SVDC_NS, format!("{base}.q")).into(),
            ]
        })
        .collect()
}

/// Pretty-print a single data-change event. The full node ID is
/// too long for the terminal, so we extract the channel + leaf
/// pair (e.g. `Ch00_Va.instMag.i = 4811 Good`).
fn print_data_change(dv: &DataValue, item: &MonitoredItem) {
    let node_id = item.item_to_monitor().node_id.to_string();
    let short = short_node_label(&node_id);
    let value = dv
        .value
        .as_ref()
        .map(|v| format!("{v:?}"))
        .unwrap_or_else(|| "—".to_string());
    let status = dv
        .status
        .as_ref()
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "?".to_string());
    println!("[L1] {short} = {value} ({status})");
}

/// Extract the channel + leaf pair from a full SVDC string node
/// ID. Best-effort: returns the last three dotted segments when
/// the leaf is `instMag.i` or `instMag.f`, otherwise the last two.
fn short_node_label(full: &str) -> String {
    let segments: Vec<&str> = full.rsplit('.').collect();
    let want = if matches!(segments.first().copied(), Some("i" | "f")) {
        3
    } else {
        2
    };
    segments
        .into_iter()
        .take(want)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_channels_match_publisher_layout() {
        assert_eq!(REFERENCE_CHANNELS.len(), 8);
        assert_eq!(REFERENCE_CHANNELS[0], "Ch00_Va");
        assert_eq!(REFERENCE_CHANNELS[7], "Ch07_In");
    }

    #[test]
    fn monitored_item_count_is_eight_channels_times_two_attrs() {
        let items = monitored_item_requests("MU01");
        assert_eq!(items.len(), 16, "8 channels × {{instMag.i, q}} = 16 items");
    }

    #[test]
    fn short_node_label_extracts_three_segments_for_instmag_i() {
        let full = "ns=2;s=Substations.Demo.MU01.ChannelRegistry.Ch00_Va.instMag.i".to_string();
        assert_eq!(short_node_label(&full), "Ch00_Va.instMag.i");
    }

    #[test]
    fn short_node_label_extracts_two_segments_for_q() {
        let full = "ns=2;s=Substations.Demo.MU01.ChannelRegistry.Ch00_Va.q".to_string();
        assert_eq!(short_node_label(&full), "Ch00_Va.q");
    }
}
