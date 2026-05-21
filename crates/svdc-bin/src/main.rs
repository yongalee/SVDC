/* SVDC Binary CLI Entrypoint
   OWNER: antigravity
   Agent: antigravity-subagent-ui-spec
   NFR-10: English-only comments and identifiers
*/

use std::env;
use std::process;

fn print_help() {
    println!("SVDC Daemon (Sampled Value Data Concentrator)");
    println!();
    println!("USAGE:");
    println!("    svdc [FLAGS] [OPTIONS]");
    println!();
    println!("FLAGS:");
    println!("    -h, --help       Prints help information");
    println!("        --ui         Explicitly enables the web-based Operator Console (default)");
    println!("        --no-ui      Disables the web-based Operator Console (runs headless)");
    println!();
    println!("OPTIONS:");
    println!("        --ui-bind <addr>  Sets the bind address for the Operator Console [default: 127.0.0.1:8080]");
    println!();
    println!("ENVIRONMENT VARIABLES:");
    println!("    SVDC_UI=1        Enables the Operator Console");
    println!("    SVDC_NO_UI=1     Disables the Operator Console");
    println!("    SVDC_UI_BIND     Sets the bind address for the Operator Console");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Help request check
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        process::exit(0);
    }

    // CLI parse state
    let mut cli_ui: Option<bool> = None;
    let mut cli_bind: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ui" => {
                if let Some(false) = cli_ui {
                    eprintln!("Error: CLI options --ui and --no-ui are mutually exclusive.");
                    process::exit(1);
                }
                cli_ui = Some(true);
            }
            "--no-ui" => {
                if let Some(true) = cli_ui {
                    eprintln!("Error: CLI options --ui and --no-ui are mutually exclusive.");
                    process::exit(1);
                }
                cli_ui = Some(false);
            }
            "--ui-bind" => {
                if i + 1 < args.len() {
                    cli_bind = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: Option --ui-bind requires a bind address value.");
                    process::exit(1);
                }
            }
            other => {
                eprintln!("Error: Unknown CLI argument '{}'. Use --help for usage.", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Env parse state
    let env_ui = env::var("SVDC_UI").ok().and_then(|v| if v == "1" { Some(true) } else { None });
    let env_no_ui = env::var("SVDC_NO_UI").ok().and_then(|v| if v == "1" { Some(true) } else { None });
    let env_bind = env::var("SVDC_UI_BIND").ok();

    if env_ui.is_some() && env_no_ui.is_some() {
        eprintln!("Error: Environment variables SVDC_UI and SVDC_NO_UI are mutually exclusive.");
        process::exit(1);
    }

    // Resolve UI flag: CLI > Env > Default
    let ui_enabled = if let Some(cli_val) = cli_ui {
        cli_val
    } else if let Some(_) = env_ui {
        true
    } else if let Some(_) = env_no_ui {
        false
    } else {
        true // default on
    };

    // Resolve UI bind address: CLI > Env > Default
    let ui_bind_addr = if let Some(cli_addr) = cli_bind {
        cli_addr
    } else if let Some(env_addr) = env_bind {
        env_addr
    } else {
        "127.0.0.1:8080".to_string() // default loopback
    };

    println!("svdc: initializing core data plane...");
    
    if ui_enabled {
        println!("svdc: Operator Console enabled.");
        svdc_console::start_console(&ui_bind_addr);
        println!("svdc: Operator Console initialized.");
    } else {
        println!("svdc: Operator Console disabled (headless mode).");
    }

    println!("svdc: Phase 0 skeleton fully operational under CLI rules.");
}
