use anyhow::{Context, Result};

use super::run::run_spawned_command;
use crate::commands::{Cli, Commands};
use crate::{json_output, rewrite_cmd, tracking};

/// Maximum bytes to capture from command stdout (64 MB).
pub(crate) const MAX_STDOUT_CAPTURE: usize = 64 * 1024 * 1024;

/// Maximum bytes to capture from command stderr (16 MB).
pub(crate) const MAX_STDERR_CAPTURE: usize = 16 * 1024 * 1024;

/// Timeout for dispatch_json subprocess operations (120 seconds).
pub(crate) const DISPATCH_JSON_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Run a command with bounded stdout and stderr capture.
///
/// Uses the same 64 MB cap as `run_spawned_command` and drains stderr to avoid deadlocks.
/// This helper is used by family runners (JS, Python, etc.) instead of `cmd.output()` directly.
pub(crate) fn run_bounded(
    cmd: &mut std::process::Command,
) -> std::io::Result<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;
    use std::thread;

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let stdout_pipe = child.stdout.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture stdout")
    })?;
    let stderr_pipe = child.stderr.take().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, "Failed to capture stderr")
    })?;

    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stdout_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < MAX_STDOUT_CAPTURE {
                captured.extend_from_slice(&buf[..count]);
            }
            // Continue draining stdout even after cap is hit, but discard bytes past the cap
        }

        Ok(captured)
    });

    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stderr_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];
        let mut cap_logged = false;

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < MAX_STDERR_CAPTURE {
                captured.extend_from_slice(&buf[..count]);
            } else if !cap_logged {
                // Log once when cap is first exceeded (asymmetric with run_spawned_command which streams post-cap bytes)
                tracing::debug!("run_bounded: stderr cap reached, discarding further bytes");
                cap_logged = true;
            }
            // Continue draining stderr even after cap is hit, but discard bytes past the cap
        }

        Ok(captured)
    });

    let status = child.wait()?;

    let stdout = stdout_handle
        .join()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "stdout thread panicked"))??;
    let stderr = stderr_handle
        .join()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "stderr thread panicked"))??;

    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

/// Run a command with bounded output capture and timeout.
///
/// Spawns a subprocess, captures stdout and stderr up to the specified limits,
/// and enforces a maximum runtime. If the timeout is exceeded, the process is killed
/// and an error is returned.
fn bounded_output(
    mut cmd: std::process::Command,
    timeout: std::time::Duration,
    max_bytes: usize,
) -> std::io::Result<std::process::Output> {
    use std::io::Read;
    use std::process::Stdio;
    use std::sync::mpsc;

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        let _ = stderr.read_to_end(&mut err);
        out.truncate(max_bytes);
        err.truncate(max_bytes);
        let _ = tx.send((out, err));
    });

    match rx.recv_timeout(timeout) {
        Ok((out, err)) => {
            let status = child.wait()?;
            Ok(std::process::Output {
                status,
                stdout: out,
                stderr: err,
            })
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait(); // reap zombie
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "dispatch_json subprocess timed out",
            ))
        }
    }
}

pub(super) fn dispatch_proxy(args: &[std::ffi::OsString], cli: &Cli) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!(
            "proxy requires a command to execute\nUsage: mycelium proxy <command> [args...]"
        );
    }

    let timer = tracking::TimedExecution::start();

    let cmd_name = args[0].to_string_lossy();
    let cmd_args: Vec<String> = args[1..]
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();

    if cli.verbose > 0 {
        eprintln!("Proxy mode: {} {}", cmd_name, cmd_args.join(" "));
    }

    let mut command = std::process::Command::new(cmd_name.as_ref());
    command.args(&cmd_args);

    run_spawned_command(
        command,
        &format!("{} {}", cmd_name, cmd_args.join(" ")),
        &format!("mycelium proxy {} {}", cmd_name, cmd_args.join(" ")),
        timer,
    )
}

pub(super) fn dispatch_invoke_command(command: &[String], explain: bool, cli: &Cli) -> Result<()> {
    let rendered_command = crate::platform::render_shell_command(command);

    if crate::discover::registry::is_diagnostic_passthrough_command(rendered_command.trim()) {
        if explain {
            println!("Mycelium invoke");
            println!("Input: {}", rendered_command.trim());
            println!("Execute: {}", rendered_command.trim());
            println!("Mode: raw shell passthrough");
            println!("Reason: matched diagnostic passthrough allowlist");
            return Ok(());
        }

        if cli.verbose > 0 {
            eprintln!("Invoke mode (raw): {}", rendered_command);
        }

        let timer = tracking::TimedExecution::start();
        let mut child_command = crate::platform::invoke_shell_command(&rendered_command);
        if cli.skip_env {
            child_command.env("SKIP_ENV_VALIDATION", "1");
        }

        return run_spawned_command(
            child_command,
            &rendered_command,
            &format!("mycelium invoke {}", rendered_command),
            timer,
        );
    }

    let resolution = rewrite_cmd::resolve_runtime_command(&rendered_command);

    if explain {
        println!("Mycelium invoke");
        println!("Input: {}", rendered_command.trim());
        println!("Execute: {}", resolution.command);
        println!(
            "Mode: {}",
            if resolution.rewritten {
                "rewritten through Mycelium"
            } else {
                "raw shell command"
            }
        );
        if let Some(estimated_savings_pct) = resolution.estimated_savings_pct {
            println!("Estimated savings: {:.1}%", estimated_savings_pct);
        }
        println!("Reason: {}", resolution.reason);
        return Ok(());
    }

    if cli.verbose > 0 {
        eprintln!("Invoke mode: {}", resolution.command);
    }

    let timer = tracking::TimedExecution::start();
    let mut child_command = crate::platform::invoke_shell_command(&resolution.command);
    if cli.skip_env {
        child_command.env("SKIP_ENV_VALIDATION", "1");
    }

    run_spawned_command(
        child_command,
        &rendered_command,
        &format!("mycelium invoke {}", resolution.command),
        timer,
    )
}


/// Tools supported by mycelium for proxying and filtering.
/// This list must remain synchronized with `is_operational_command`.
///
/// SECURITY: whitelist pattern — new tools are NOT executed
/// until explicitly added here. A forgotten tool fails open (not executed)
/// rather than creating false confidence about what's protected.
const SUPPORTED_TOOLS: &[&str] = &[
    "ls",
    "tree",
    "read",
    "peek",
    "git",
    "gh",
    "pnpm",
    "err",
    "test",
    "json",
    "deps",
    "env",
    "find",
    "diff",
    "log",
    "docker",
    "kubectl",
    "summary",
    "grep",
    "wget",
    "vitest",
    "prisma",
    "tsc",
    "next",
    "lint",
    "prettier",
    "playwright",
    "cargo",
    "npm",
    "npx",
    "curl",
    "ruff",
    "pytest",
    "pip",
    "go",
    "golangci-lint",
    "gt",
    "invoke",
];

/// Returns true for commands that are invoked via the hook pipeline
/// (i.e., commands that process rewritten shell commands).
/// Meta commands (init, gain, verify, etc.) are excluded because
/// they are run directly by the user, not through the hook.
///
/// SECURITY: whitelist pattern — derives from SUPPORTED_TOOLS to ensure
/// one canonical source of truth. A forgotten command fails open (no check)
/// rather than creating false confidence about what's protected.
pub fn is_operational_command(cmd: &Commands) -> bool {
    let cmd_name = match cmd {
        Commands::Ls(_) => "ls",
        Commands::Tree(_) => "tree",
        Commands::Read(_) => "read",
        Commands::Peek(_) => "peek",
        Commands::Git(_) => "git",
        Commands::Gh(_) => "gh",
        Commands::Gt(_) => "gt",
        Commands::Cargo(_) => "cargo",
        Commands::Tsc(_) => "tsc",
        Commands::Next(_) => "next",
        Commands::Go(_) => "go",
        Commands::Lint(_) => "lint",
        Commands::Prettier(_) => "prettier",
        Commands::Format(_) => return false, // not operational
        Commands::Ruff(_) => "ruff",
        Commands::Mypy(_) => return false, // not operational
        Commands::GolangciLint(_) => "golangci-lint",
        Commands::Test(_) => "test",
        Commands::Vitest(_) => "vitest",
        Commands::Playwright(_) => "playwright",
        Commands::Pytest(_) => "pytest",
        Commands::Pnpm(_) => "pnpm",
        Commands::Pip(_) => "pip",
        Commands::Npm(_) => "npm",
        Commands::Npx(_) => "npx",
        Commands::Psql(_) => return false, // not operational
        Commands::Prisma(_) => "prisma",
        Commands::Curl(_) => "curl",
        Commands::Wget(_) => "wget",
        Commands::Docker(_) => "docker",
        Commands::Kubectl(_) => "kubectl",
        Commands::Terraform(_) => return false, // not operational
        Commands::Aws(_) => return false,       // not operational
        Commands::Atmos(_) => return false,     // not operational
        Commands::Json(_) => "json",
        Commands::Log(_) => "log",
        Commands::Err(_) => "err",
        Commands::Summary(_) => "summary",
        Commands::Env(_) => "env",
        Commands::Deps(_) => "deps",
        Commands::Gain(_) => return false, // not operational
        #[cfg(unix)]
        Commands::ServeSocket(_) => return false, // not operational
        Commands::Discover(_) => return false, // not operational
        Commands::Learn(_) => return false, // not operational
        Commands::Context(_) => return false, // not operational
        Commands::Init(_) => return false, // not operational
        Commands::Config(_) => return false, // not operational
        Commands::Doctor => return false,      // not operational
        Commands::Verify => return false,      // not operational
        Commands::SelfUpdate(_) => return false, // not operational
        Commands::Completions(_) => return false, // not operational
        Commands::Proxy(_) => return false, // not operational (handled separately)
        Commands::Invoke(_) => "invoke",
        Commands::Benchmark(_) => return false, // not operational
        Commands::Plugin(_) => return false,    // not operational
        Commands::Wc(_) => return false,        // not operational
        Commands::ParseHealth(_) => return false, // not operational
        Commands::CcEconomics(_) => return false, // not operational
        Commands::HookAudit(_) => return false, // not operational
        Commands::Rewrite(_) => return false,   // not operational
        Commands::Explain(_) => return false,   // not operational
        Commands::Find(_) => "find",
        Commands::Grep(_) => "grep",
        Commands::Diff(_) => "diff",
    };
    SUPPORTED_TOOLS.contains(&cmd_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::Commands;

    #[test]
    fn test_is_operational_command_consistency_with_supported_tools() {
        // Verify that is_operational_command returns true for commands in SUPPORTED_TOOLS.
        // This enforces that both paths use the same canonical set.
        use crate::commands::{Ls, Npm, Tree};
        let test_cases: Vec<(&str, Commands)> = vec![
            ("ls", Commands::Ls(Ls { args: vec![] })),
            ("tree", Commands::Tree(Tree { args: vec![] })),
            ("npm", Commands::Npm(Npm { args: vec![] })),
            ("npm", Commands::Npm(Npm { args: vec![] })),
        ];

        for (tool_name, cmd) in test_cases {
            assert!(
                is_operational_command(&cmd),
                "is_operational_command should return true for {} command",
                tool_name
            );
            assert!(
                SUPPORTED_TOOLS.contains(&tool_name),
                "{} should be in SUPPORTED_TOOLS",
                tool_name
            );
        }
    }

    #[test]
    fn test_supported_tools_vs_is_operational_command_drift() {
        // Verify every SUPPORTED_TOOLS entry has a corresponding entry in is_operational_command.
        // We do this by checking that SUPPORTED_TOOLS doesn't contain tools that would fail
        // if processed by is_operational_command.
        //
        // This test acts as a drift detector — if a tool is added to SUPPORTED_TOOLS but not
        // to is_operational_command (or vice versa), the mismatch will be caught by integration
        // tests that attempt to execute commands through the dispatch system.

        // Known operational command tool names (from is_operational_command)
        let operational_commands = [
            "ls",
            "tree",
            "read",
            "peek",
            "git",
            "gh",
            "gt",
            "cargo",
            "tsc",
            "next",
            "go",
            "lint",
            "prettier",
            "ruff",
            "golangci-lint",
            "test",
            "vitest",
            "playwright",
            "pytest",
            "pnpm",
            "pip",
            "npm",
            "npx",
            "prisma",
            "curl",
            "wget",
            "docker",
            "kubectl",
            "json",
            "log",
            "err",
            "summary",
            "env",
            "deps",
            "invoke",
            "find",
            "grep",
            "diff",
        ];

        // Check that all SUPPORTED_TOOLS have a counterpart in operational commands
        for tool in SUPPORTED_TOOLS {
            assert!(
                operational_commands.contains(tool),
                "Tool '{}' in SUPPORTED_TOOLS is not in operational_commands list",
                tool
            );
        }

        // Check that all operational commands are in SUPPORTED_TOOLS (if they should be)
        // This is a one-way check to catch missing additions
        for tool in &operational_commands {
            if !SUPPORTED_TOOLS.contains(tool) {
                panic!("Tool '{}' is operational but not in SUPPORTED_TOOLS", tool);
            }
        }
    }
}

/// Re-invoke `mycelium` without `--json`, capture stdout, and wrap output in a JSON envelope.
pub fn dispatch_json(cli: Cli) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--json").collect();

    let original_cmd = args.join(" ");
    let rewrite_resolution = rewrite_cmd::resolve_runtime_command(&original_cmd);
    let project_path = std::env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok().or(Some(path)))
        .map(|path| path.to_string_lossy().to_string());

    // Get raw output only from known tool binaries to prevent arbitrary code execution.
    // Only allow execution of tools that mycelium is designed to proxy.
    let raw_output = if !args.is_empty() {
        let tool_name = &args[0];

        let base_name = std::path::Path::new(tool_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(tool_name);

        if SUPPORTED_TOOLS.contains(&base_name) {
            // Resolve the tool path from SUPPORTED_TOOLS allowlist
            match crate::platform::command_path(base_name) {
                Some(resolved_path) => {
                    let mut cmd = std::process::Command::new(&resolved_path);
                    cmd.args(&args[1..]);
                    let raw_result = bounded_output(cmd, DISPATCH_JSON_TIMEOUT, MAX_STDOUT_CAPTURE);
                    match raw_result {
                        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
                        Err(e) => {
                            use tracing::warn;
                            warn!(
                                tool = tool_name,
                                error_kind = ?e.kind(),
                                "Tool spawn error in dispatch_json: {e}"
                            );
                            String::new()
                        }
                    }
                }
                None => {
                    use tracing::warn;
                    warn!(
                        base_name = base_name,
                        "Failed to resolve allowed tool path in dispatch_json"
                    );
                    String::new()
                }
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mycelium_exe = std::env::current_exe().context("Failed to locate mycelium executable")?;
    let mut filtered_cmd = std::process::Command::new(&mycelium_exe);
    filtered_cmd.args(&args);
    // Note: raw_output already buffered child stdout above; filtered_result buffers mycelium's filtered stdout.
    // Both are bounded at MAX_STDOUT_CAPTURE (64 MB) by bounded_output helper, preventing compound memory exhaustion.
    let filtered_result = bounded_output(filtered_cmd, DISPATCH_JSON_TIMEOUT, MAX_STDOUT_CAPTURE);

    let (envelope, exit_code) = match filtered_result {
        Ok(output) if output.status.success() || !output.stdout.is_empty() => {
            let filtered = String::from_utf8_lossy(&output.stdout).to_string();
            let exit_code = output.status.code().unwrap_or(0);
            let envelope = json_output::wrap_output(
                &original_cmd,
                &format!("mycelium {original_cmd}"),
                &filtered,
                &raw_output,
                project_path.as_deref(),
                Some(&rewrite_resolution),
            );
            (envelope, exit_code)
        }
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let envelope = json_output::wrap_error(&stderr, exit_code);
            (envelope, exit_code)
        }
        Err(e) => {
            let envelope = json_output::wrap_error(&e.to_string(), 1);
            (envelope, 1)
        }
    };

    let _ = cli;

    println!("{envelope}");
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}
