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
use std::net::SocketAddr;
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
    println!("        --ui-bind <addr>    Bind address for the Operator Console [default: 127.0.0.1:8080]");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    SVDC_UI=1               Enables the Operator Console");
    println!("    SVDC_NO_UI=1            Disables the Operator Console");
    println!("    SVDC_UI_BIND            Bind address for the Operator Console");
}

#[derive(Debug)]
struct Config {
    ui_enabled: bool,
    ui_bind: SocketAddr,
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

    Ok(Config {
        ui_enabled,
        ui_bind,
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

    let bind = cfg.ui_bind;
    let result = rt.block_on(async move {
        println!("svdc: Operator Console enabled at http://{bind}");
        svdc_console::start_console(bind).await
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("svdc: Operator Console error: {e}");
            ExitCode::FAILURE
        }
    }
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
}
