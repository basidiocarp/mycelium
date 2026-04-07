//! Routes parsed CLI commands to their specialized handler modules.
mod exec;
mod families;
mod routes;

use anyhow::Result;

use crate::commands::{Cli, MYCELIUM_META_COMMANDS};
use crate::{integrity, plugin, tracking, utils};

pub use exec::is_operational_command;

/// Handle Clap parse failures: fall back to raw execution for non-meta commands.
pub fn run_fallback(parse_error: clap::Error) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        parse_error.exit();
    }

    if MYCELIUM_META_COMMANDS.contains(&args[0].as_str()) {
        parse_error.exit();
    }

    let raw_command = args.join(" ");
    let error_message = utils::strip_ansi(&parse_error.to_string());
    let timer = tracking::TimedExecution::start();

    if let Some(plugin_path) = plugin::find_plugin(&args[0]) {
        match std::process::Command::new(&args[0])
            .args(&args[1..])
            .output()
        {
            Ok(raw_output) => {
                let raw = String::from_utf8_lossy(&raw_output.stdout).to_string();
                match plugin::run_plugin(&plugin_path, &raw) {
                    Ok(filtered) => {
                        timer.track(
                            &raw_command,
                            &format!("mycelium plugin: {}", raw_command),
                            &raw,
                            &filtered,
                        );
                        tracking::record_parse_failure_silent(
                            &raw_command,
                            &error_message,
                            true,
                            None,
                        );
                        print!("{}", filtered);
                        return Ok(());
                    }
                    Err(error) => {
                        eprintln!("[mycelium: plugin {:?} failed: {}]", plugin_path, error);
                    }
                }
            }
            Err(error) => {
                eprintln!("[mycelium: plugin raw capture failed: {}]", error);
            }
        }
    }

    let status = std::process::Command::new(&args[0])
        .args(&args[1..])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(status) => {
            timer.track_passthrough(&raw_command, &format!("mycelium fallback: {}", raw_command));
            tracking::record_parse_failure_silent(&raw_command, &error_message, true, None);
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Err(error) => {
            tracking::record_parse_failure_silent(&raw_command, &error_message, false, None);
            eprintln!("[mycelium: fallback failed: {}]", error);
            parse_error.exit();
        }
    }

    Ok(())
}

/// Dispatch a parsed CLI command to its handler module.
pub fn dispatch(cli: Cli) -> Result<()> {
    if cli.json {
        return exec::dispatch_json(cli);
    }

    if is_operational_command(&cli.command) {
        integrity::runtime_check()?;
    }

    routes::dispatch_command(cli)
}
