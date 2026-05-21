//! SVDC daemon entry point.
//!
//! Resolves the `--ui` / `--no-ui` / `--ui-bind` toggle from CLI and
//! env per ADR-0005, then either runs the Operator Console (axum +
//! maud) on its own tokio runtime or stays headless.
//!
//! OWNER: shared. ADR-0005 wiring authored by Antigravity; tokio
//! runtime + async dispatch refined by Claude (WBS-9.1a).
//! NFR-10: English-only.

use std::env;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

fn print_help() {
    println!("SVDC Daemon (Sampled Value Data Concentrator)");
    println!();
    println!("USAGE:");
    println!("    svdc [FLAGS] [OPTIONS]");
    println!();
    println!("FLAGS:");
    println!("    -h, --help              Prints help information");
    println!("        --ui                Explicitly enables the Operator Console (default)");
    println!("        --no-ui             Disables the Operator Console (runs headless)");
    println!();
    println!("OPTIONS:");
    println!("        --ui-bind <addr>           Bind address for the Operator Console [default: 127.0.0.1:8080]");
    println!("        --operational-config <p>   Path to the SVDC-local operational state TOML");
    println!(
        "                                   (calibration, etc.). Loaded on startup; auto-saved"
    );
    println!("                                   on every operator change. Created if absent.");
    println!("        --audit-log <p>            Path to the append-only audit JSONL file.");
    println!("                                   Loaded on startup; each subsequent operator");
    println!("                                   action appends one line. Created if absent.");
    println!("        --ingress-udp <addr:port>  Bind a UDP listener (multicast or unicast) and");
    println!("                                   feed received SV payloads into the data plane.");
    println!("                                   See docs/simulator-runbook.md (ADR-0015).");
    println!("                                   Example: --ingress-udp 239.0.0.1:9100");
    println!("        --enable-l0-demo           Spawn the L0 reference in-process subscriber");
    println!("                                   (svdc-subscribe::InProcessSubscriber). Prints");
    println!("                                   tick summaries to stdout. See");
    println!("                                   docs/northbound-simulators.md (ADR-0016).");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    SVDC_UI=1                      Enables the Operator Console");
    println!("    SVDC_NO_UI=1                   Disables the Operator Console");
    println!("    SVDC_UI_BIND                   Bind address for the Operator Console");
    println!("    SVDC_OPERATIONAL_CONFIG        Path equivalent of --operational-config");
    println!("    SVDC_AUDIT_LOG                 Path equivalent of --audit-log");
    println!("    SVDC_INGRESS_UDP               Path equivalent of --ingress-udp");
    println!("    SVDC_ENABLE_L0_DEMO=1          Path equivalent of --enable-l0-demo");
}

#[derive(Debug)]
struct Config {
    ui_enabled: bool,
    ui_bind: SocketAddr,
    operational_path: Option<PathBuf>,
    audit_log_path: Option<PathBuf>,
    ingress_udp: Option<SocketAddr>,
    enable_l0_demo: bool,
}

#[derive(Debug)]
enum ConfigError {
    HelpRequested,
    MutuallyExclusive(&'static str),
    MissingValue(&'static str),
    UnknownArg(String),
    BadAddr(String),
}

fn resolve_config(args: &[String]) -> Result<Config, ConfigError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(ConfigError::HelpRequested);
    }

    let mut cli_ui: Option<bool> = None;
    let mut cli_bind: Option<String> = None;
    let mut cli_op_path: Option<PathBuf> = None;
    let mut cli_audit_path: Option<PathBuf> = None;
    let mut cli_ingress_udp: Option<String> = None;
    let mut cli_enable_l0_demo: Option<bool> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ui" => {
                if matches!(cli_ui, Some(false)) {
                    return Err(ConfigError::MutuallyExclusive(
                        "CLI options --ui and --no-ui are mutually exclusive",
                    ));
                }
                cli_ui = Some(true);
            }
            "--no-ui" => {
                if matches!(cli_ui, Some(true)) {
                    return Err(ConfigError::MutuallyExclusive(
                        "CLI options --ui and --no-ui are mutually exclusive",
                    ));
                }
                cli_ui = Some(false);
            }
            "--ui-bind" => {
                i += 1;
                if i >= args.len() {
                    return Err(ConfigError::MissingValue("--ui-bind requires an address"));
                }
                cli_bind = Some(args[i].clone());
            }
            "--operational-config" => {
                i += 1;
                if i >= args.len() {
                    return Err(ConfigError::MissingValue(
                        "--operational-config requires a path",
                    ));
                }
                cli_op_path = Some(PathBuf::from(&args[i]));
            }
            "--audit-log" => {
                i += 1;
                if i >= args.len() {
                    return Err(ConfigError::MissingValue("--audit-log requires a path"));
                }
                cli_audit_path = Some(PathBuf::from(&args[i]));
            }
            "--ingress-udp" => {
                i += 1;
                if i >= args.len() {
                    return Err(ConfigError::MissingValue(
                        "--ingress-udp requires an addr:port",
                    ));
                }
                cli_ingress_udp = Some(args[i].clone());
            }
            "--enable-l0-demo" => {
                cli_enable_l0_demo = Some(true);
            }
            other => return Err(ConfigError::UnknownArg(other.to_string())),
        }
        i += 1;
    }

    let env_ui = env::var("SVDC_UI").ok().filter(|v| v == "1").map(|_| true);
    let env_no_ui = env::var("SVDC_NO_UI")
        .ok()
        .filter(|v| v == "1")
        .map(|_| true);
    let env_bind = env::var("SVDC_UI_BIND").ok();

    if env_ui.is_some() && env_no_ui.is_some() {
        return Err(ConfigError::MutuallyExclusive(
            "Environment variables SVDC_UI and SVDC_NO_UI are mutually exclusive",
        ));
    }

    let ui_enabled = match (cli_ui, env_ui.is_some(), env_no_ui.is_some()) {
        (Some(v), _, _) => v,
        (None, _, true) => false,
        _ => true,
    };

    let addr_str = cli_bind
        .or(env_bind)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let ui_bind = addr_str
        .parse::<SocketAddr>()
        .map_err(|_| ConfigError::BadAddr(addr_str))?;

    let operational_path =
        cli_op_path.or_else(|| env::var("SVDC_OPERATIONAL_CONFIG").ok().map(PathBuf::from));
    let audit_log_path =
        cli_audit_path.or_else(|| env::var("SVDC_AUDIT_LOG").ok().map(PathBuf::from));
    let ingress_udp = match cli_ingress_udp.or_else(|| env::var("SVDC_INGRESS_UDP").ok()) {
        Some(s) => Some(
            s.parse::<SocketAddr>()
                .map_err(|_| ConfigError::BadAddr(s))?,
        ),
        None => None,
    };

    let env_enable_l0_demo = env::var("SVDC_ENABLE_L0_DEMO")
        .ok()
        .filter(|v| v == "1")
        .map(|_| true);
    let enable_l0_demo = cli_enable_l0_demo.or(env_enable_l0_demo).unwrap_or(false);

    Ok(Config {
        ui_enabled,
        ui_bind,
        operational_path,
        audit_log_path,
        ingress_udp,
        enable_l0_demo,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let cfg = match resolve_config(&args) {
        Ok(c) => c,
        Err(ConfigError::HelpRequested) => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Err(ConfigError::MutuallyExclusive(msg)) => {
            eprintln!("Error: {msg}.");
            return ExitCode::FAILURE;
        }
        Err(ConfigError::MissingValue(msg)) => {
            eprintln!("Error: {msg}.");
            return ExitCode::FAILURE;
        }
        Err(ConfigError::UnknownArg(arg)) => {
            eprintln!("Error: unknown CLI argument '{arg}'. Use --help for usage.");
            return ExitCode::FAILURE;
        }
        Err(ConfigError::BadAddr(addr)) => {
            eprintln!("Error: '{addr}' is not a valid socket address.");
            return ExitCode::FAILURE;
        }
    };

    println!("svdc: initializing core data plane...");

    // Wire SVDC-local operational state to its config file (per ADR-0007).
    // Calibration triples (and future operator-tunable settings) load
    // from this file on startup and auto-save on every mutation. The
    // SCD is intentionally NOT touched by this code path.
    if let Some(path) = cfg.operational_path.as_ref() {
        match svdc_console::operational::global().configure_persistence(path.clone()) {
            Ok(n) => println!(
                "svdc: operational state loaded from {} ({n} override(s))",
                path.display()
            ),
            Err(e) => {
                eprintln!(
                    "Error: could not configure operational persistence ({}): {e}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        }
    }

    // Wire the audit log to its on-disk JSONL file. Existing records
    // replay into the in-memory ring so /api/audit shows operator
    // history from before the restart.
    if let Some(path) = cfg.audit_log_path.as_ref() {
        match svdc_console::audit::global().configure_persistence(path.clone()) {
            Ok(n) => println!(
                "svdc: audit log persisted to {} ({n} historical record(s) replayed)",
                path.display()
            ),
            Err(e) => {
                eprintln!(
                    "Error: could not configure audit persistence ({}): {e}",
                    path.display()
                );
                return ExitCode::FAILURE;
            }
        }
    }

    if !cfg.ui_enabled {
        println!("svdc: Operator Console disabled (headless mode).");
        println!("svdc: Phase 0 skeleton — no protection path yet; daemon would idle here.");
        return ExitCode::SUCCESS;
    }

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("svdc-console")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Error: failed to start tokio runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Pre-flight: bind the operator-console TCP port synchronously
    // before entering the async runtime so a port-in-use failure
    // surfaces immediately with a clear diagnostic. We close the
    // probe listener and let `start_console` re-bind — the race
    // window is short and the alternative (handing the listener
    // across an async boundary) would couple `svdc-bin` to
    // tokio-net internals.
    if let Err(e) = std::net::TcpListener::bind(cfg.ui_bind) {
        report_bind_error("Operator Console", "--ui-bind", cfg.ui_bind, &e);
        return ExitCode::FAILURE;
    }
    if let Some(addr) = cfg.ingress_udp {
        if let Err(e) = std::net::UdpSocket::bind(addr) {
            report_bind_error("Ingress UDP", "--ingress-udp", addr, &e);
            return ExitCode::FAILURE;
        }
    }

    // Phase 0 ingress (ADR-0015): if --ingress-udp is set, spawn a
    // tokio task that receives L2-stripped SV payloads, decodes
    // them, aligns them, and pushes the resulting TickRecords into
    // the same TickBuffer the UI exposes via
    // svdc_console::dataplane::global().
    let ingress_udp = cfg.ingress_udp;
    let enable_l0_demo = cfg.enable_l0_demo;
    let bind = cfg.ui_bind;
    let result = rt.block_on(async move {
        if let Some(addr) = ingress_udp {
            if let Err(e) = spawn_udp_ingress(addr) {
                report_bind_error("Ingress UDP", "--ingress-udp", addr, &e);
                return Err(std::io::Error::other(format!("ingress-udp: {e}")));
            }
            println!("svdc: --ingress-udp {addr} bound; live feed active");
        }
        if enable_l0_demo {
            spawn_l0_demo();
        }
        println!("svdc: Operator Console enabled at http://{bind}");
        svdc_console::start_console(bind).await
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            report_bind_error("Operator Console", "--ui-bind", cfg.ui_bind, &e);
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("svdc: Operator Console error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Print a port-in-use diagnostic that survives non-English OS
/// locales (the localized `os error 10048` text on Windows is
/// otherwise the only signal the operator gets). Hints at the two
/// most common fixes: kill a stale process, or pass an override
/// flag.
fn report_bind_error(label: &str, flag: &str, addr: std::net::SocketAddr, err: &std::io::Error) {
    if err.kind() == ErrorKind::AddrInUse {
        eprintln!("svdc: {label} cannot bind to {addr} — port already in use.");
        eprintln!("Hint: another svdc instance (or another process) is holding the port.");
        eprintln!("  - On Windows:   Get-Process svdc -ErrorAction SilentlyContinue | Stop-Process -Force");
        eprintln!("  - On Linux/mac: pkill svdc");
        eprintln!("Or pass {flag} <addr:port> to bind elsewhere.");
    } else {
        eprintln!("svdc: {label} cannot bind to {addr}: {err}");
        eprintln!("Hint: pass {flag} <addr:port> to bind elsewhere.");
    }
}

/// Spawn the UDP ingress task. Binds the socket synchronously so
/// bind failures surface as errors (not silent in a background
/// task); the receive loop then runs on its own tokio task and the
/// aligner runs on a blocking thread that consumes the ingress
/// ring.
fn spawn_udp_ingress(addr: std::net::SocketAddr) -> std::io::Result<()> {
    use std::sync::Arc;
    use svdc_aligner::Aligner;
    use svdc_ingress::{Decoder, IngressFrame, IngressRing, Subscriber, UdpSubscriber};

    let mut sub = UdpSubscriber::bind(addr, Some(std::time::Duration::from_millis(250)))?;
    let pipeline = svdc_console::dataplane::global();
    pipeline.mark_external_feed(true);

    let ring = Arc::new(IngressRing::new(4096));
    let decoder = Decoder;

    // Producer: blocking receive loop on its own OS thread (the
    // recv socket is blocking; spawn_blocking would tie up tokio
    // workers).
    let producer_ring = Arc::clone(&ring);
    std::thread::Builder::new()
        .name("svdc-ingress-udp".to_string())
        .spawn(move || loop {
            match sub.next_frame() {
                Ok((bytes, ts)) => match decoder.decode_l2_stripped(&bytes) {
                    Ok(samples) => {
                        let frame = IngressFrame {
                            timestamp: ts,
                            samples,
                        };
                        if producer_ring.push(frame).is_err() {
                            tracing::warn!(
                                "ingress ring full; dropping frame (consumer falling behind)"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            "ingress: decode failure (likely non-SV traffic on the port): {e}"
                        );
                    }
                },
                Err(_) => {
                    // Recv timeout — let the loop tick so the
                    // thread can be torn down at process exit.
                    continue;
                }
            }
        })?;

    // Consumer: aligner drains the ring into the same TickBuffer
    // the UI shares.
    let consumer_ring = ring;
    let pipe = svdc_console::dataplane::global();
    std::thread::Builder::new()
        .name("svdc-aligner".to_string())
        .spawn(move || {
            // Bin period: 1/4800 s. Phase 2 derives from the
            // first frame's smpRate; Phase 0 hardcodes the demo
            // rate.
            let mut aligner = Aligner::new(208_333);
            loop {
                match consumer_ring.pop() {
                    Some(frame) => {
                        // PR D: record every distinct svID we see
                        // before the aligner consumes the frame.
                        for asdu in &frame.samples {
                            pipe.note_mu_observed(&asdu.sv_id);
                        }
                        for tick in aligner.process_frame(frame) {
                            pipe.buffer.push(tick);
                            pipe.record_external_tick();
                        }
                    }
                    None => {
                        // Empty ring — short sleep to avoid spin.
                        std::thread::sleep(std::time::Duration::from_micros(200));
                    }
                }
            }
        })?;

    Ok(())
}

/// Spawn the L0 reference in-process subscriber demo (ADR-0016).
///
/// Subscribes to the shared `TickBuffer` via
/// `svdc-subscribe::InProcessSubscriber`, then loops every 100 ms
/// calling `read_since()` and printing a one-line summary of each
/// freshly arrived tick to stdout. This is the reference consumer a
/// real EBP relay (or any zero-network-hop subscriber) would build
/// on — except a relay would feed each tick into a protection
/// algorithm instead of printing it.
///
/// Output format (one line per tick, every 10th tick by default to
/// keep stdout readable at 4800 Hz):
///
/// ```text
/// svdc-l0-demo: tick_id=480  ts=1717603200000000000 ch0=4811 ch4=22987 lag=0
/// ```
///
/// `lag` reports the number of fresh ticks delivered in this
/// `read_since` batch minus one — i.e. how far behind the consumer
/// was when the read drained.
fn spawn_l0_demo() {
    use svdc_subscribe::{ChannelSet, InProcessSubscriber, Subscriber};

    let pipeline = svdc_console::dataplane::global();
    let buffer = std::sync::Arc::clone(&pipeline.buffer);
    let factory = InProcessSubscriber::new(buffer);
    let pipe_for_task = std::sync::Arc::clone(&pipeline);

    tokio::spawn(async move {
        let mut subscription = factory.subscribe(ChannelSet::all());
        pipe_for_task.mark_l0_demo_active(true);
        println!("svdc-l0-demo: subscribed (cursor = 0)");
        let mut emitted: u64 = 0;
        let print_every: u64 = 10;
        loop {
            let batch = subscription.read_since();
            let batch_len = batch.len();
            for tick in batch {
                pipe_for_task.record_l0_demo_tick(tick.tick_id);
                if emitted % print_every == 0 {
                    let live = tick.live_samples();
                    let ch0 = live.first().map(|s| s.value_q).unwrap_or(0);
                    let ch4 = live.get(4).map(|s| s.value_q).unwrap_or(0);
                    let lag = batch_len.saturating_sub(1);
                    println!(
                        "svdc-l0-demo: tick_id={} ts={} ch0={} ch4={} lag={}",
                        tick.tick_id, tick.ts_utc_ns, ch0, ch4, lag
                    );
                }
                emitted = emitted.wrapping_add(1);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        let mut v = vec!["svdc".to_string()];
        v.extend(extra.iter().map(|s| s.to_string()));
        v
    }

    #[test]
    fn default_enables_ui_on_loopback_8080() {
        let cfg = resolve_config(&args(&[])).unwrap();
        assert!(cfg.ui_enabled);
        assert_eq!(cfg.ui_bind.to_string(), "127.0.0.1:8080");
    }

    #[test]
    fn no_ui_disables() {
        let cfg = resolve_config(&args(&["--no-ui"])).unwrap();
        assert!(!cfg.ui_enabled);
    }

    #[test]
    fn ui_bind_override() {
        let cfg = resolve_config(&args(&["--ui-bind", "0.0.0.0:9090"])).unwrap();
        assert_eq!(cfg.ui_bind.to_string(), "0.0.0.0:9090");
    }

    #[test]
    fn mutually_exclusive_flags() {
        let r = resolve_config(&args(&["--ui", "--no-ui"]));
        assert!(matches!(r, Err(ConfigError::MutuallyExclusive(_))));
    }

    #[test]
    fn bad_addr_rejected() {
        let r = resolve_config(&args(&["--ui-bind", "not-an-address"]));
        assert!(matches!(r, Err(ConfigError::BadAddr(_))));
    }

    #[test]
    fn operational_config_path_captured() {
        let cfg = resolve_config(&args(&[
            "--operational-config",
            "/var/svdc/operational.toml",
        ]))
        .unwrap();
        assert_eq!(
            cfg.operational_path.map(|p| p.display().to_string()),
            Some("/var/svdc/operational.toml".to_string())
        );
    }

    #[test]
    fn operational_config_missing_value_errors() {
        let r = resolve_config(&args(&["--operational-config"]));
        assert!(matches!(r, Err(ConfigError::MissingValue(_))));
    }

    #[test]
    fn audit_log_path_captured() {
        let cfg = resolve_config(&args(&["--audit-log", "/var/svdc/audit.jsonl"])).unwrap();
        assert_eq!(
            cfg.audit_log_path.map(|p| p.display().to_string()),
            Some("/var/svdc/audit.jsonl".to_string())
        );
    }

    #[test]
    fn audit_log_missing_value_errors() {
        let r = resolve_config(&args(&["--audit-log"]));
        assert!(matches!(r, Err(ConfigError::MissingValue(_))));
    }

    #[test]
    fn ingress_udp_addr_parsed() {
        let cfg = resolve_config(&args(&["--ingress-udp", "239.0.0.1:9100"])).unwrap();
        assert_eq!(
            cfg.ingress_udp.map(|a| a.to_string()),
            Some("239.0.0.1:9100".to_string())
        );
    }

    #[test]
    fn ingress_udp_bad_addr_rejected() {
        let r = resolve_config(&args(&["--ingress-udp", "not-an-address"]));
        assert!(matches!(r, Err(ConfigError::BadAddr(_))));
    }

    #[test]
    fn ingress_udp_missing_value_errors() {
        let r = resolve_config(&args(&["--ingress-udp"]));
        assert!(matches!(r, Err(ConfigError::MissingValue(_))));
    }

    #[test]
    fn default_has_no_ingress_udp() {
        let cfg = resolve_config(&args(&[])).unwrap();
        assert!(cfg.ingress_udp.is_none());
    }

    #[test]
    fn enable_l0_demo_defaults_off() {
        let cfg = resolve_config(&args(&[])).unwrap();
        assert!(!cfg.enable_l0_demo);
    }

    #[test]
    fn enable_l0_demo_flag_turns_it_on() {
        let cfg = resolve_config(&args(&["--enable-l0-demo"])).unwrap();
        assert!(cfg.enable_l0_demo);
    }

    #[test]
    fn enable_l0_demo_composes_with_ingress_udp() {
        let cfg = resolve_config(&args(&[
            "--ingress-udp",
            "239.0.0.1:9100",
            "--enable-l0-demo",
        ]))
        .unwrap();
        assert!(cfg.enable_l0_demo);
        assert_eq!(
            cfg.ingress_udp.map(|a| a.to_string()),
            Some("239.0.0.1:9100".to_string())
        );
    }
}
