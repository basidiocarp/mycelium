//! Routes parsed CLI commands to their specialized handler modules.
mod content_router;
mod exec;
mod families;
mod routes;

use anyhow::Result;
use spore::logging::{SpanContext, subprocess_span, tool_span, workflow_span};
use tracing::warn;

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
    let context = fallback_span_context(&args[0]);
    let _workflow_span = workflow_span("fallback_dispatch", &context).entered();

    let plugin_path = {
        let _tool_span = tool_span("plugin_discovery", &context).entered();
        plugin::find_plugin(&args[0])
    };

    if let Some(plugin_path) = plugin_path {
        let output = {
            let _subprocess_span = subprocess_span(&raw_command, &context).entered();
            std::process::Command::new(&args[0])
                .args(&args[1..])
                .output()
        };
        match output {
            Ok(raw_output) => {
                let stdout = String::from_utf8_lossy(&raw_output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&raw_output.stderr).to_string();

                if raw_output.status.success() {
                    match plugin::run_plugin(&plugin_path, &stdout) {
                        Ok(filtered) => {
                            timer.track(
                                &raw_command,
                                &format!("mycelium plugin: {}", raw_command),
                                &stdout,
                                &filtered,
                            );
                            tracking::record_parse_failure_silent(
                                &raw_command,
                                &error_message,
                                true,
                                None,
                            );
                            print!("{filtered}");
                            if !stderr.is_empty() {
                                eprint!("{stderr}");
                            }
                            return Ok(());
                        }
                        Err(error) => {
                            warn!(
                                plugin = %plugin_path.display(),
                                status = %raw_output.status,
                                stderr = %stderr.trim(),
                                "Plugin filtering failed; replaying captured raw output: {error}"
                            );
                            eprintln!("[mycelium: plugin {:?} failed: {}]", plugin_path, error);
                        }
                    }
                } else {
                    warn!(
                        plugin = %plugin_path.display(),
                        exit_code = raw_output.status.code().unwrap_or(-1),
                        stderr = %stderr.trim(),
                        "Skipping plugin because the underlying command exited non-zero"
                    );
                }

                timer.track_passthrough(
                    &raw_command,
                    &format!("mycelium fallback: {}", raw_command),
                );
                tracking::record_parse_failure_silent(&raw_command, &error_message, true, None);
                replay_captured_output(&stdout, &stderr);
                if !raw_output.status.success() {
                    std::process::exit(raw_output.status.code().unwrap_or(1));
                }
                return Ok(());
            }
            Err(error) => {
                warn!(
                    plugin = %plugin_path.display(),
                    "Plugin fallback capture failed; not rerunning command: {error}"
                );
                tracking::record_parse_failure_silent(&raw_command, &error_message, false, None);
                eprintln!("[mycelium: plugin raw capture failed: {}]", error);
                parse_error.exit();
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

fn fallback_span_context(command: &str) -> SpanContext {
    let context = SpanContext::for_app("mycelium").with_tool(command.to_string());
    match std::env::current_dir() {
        Ok(path) => context.with_workspace_root(path.display().to_string()),
        Err(_) => context,
    }
}

fn replay_captured_output(stdout: &str, stderr: &str) {
    if !stdout.is_empty() {
        print!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
}

/// Dispatch a parsed CLI command to its handler module.
pub fn dispatch(cli: Cli) -> Result<()> {
    #[cfg(unix)]
    if let crate::commands::Commands::ServeSocket { compact } = cli.command {
        return crate::socket_server::run_socket_server(compact);
    }

    if cli.json {
        return exec::dispatch_json(cli);
    }

    if is_operational_command(&cli.command) {
        integrity::runtime_check()?;
    }

    routes::dispatch_command(cli)
}
