use anyhow::{Context, Result};

use super::content_router::ContentRouter;
use crate::commands::{Cli, Commands};
use crate::{json_output, rewrite_cmd, tracking};

/// Maximum bytes to capture from command stdout (64 MB).
pub(crate) const MAX_STDOUT_CAPTURE: usize = 64 * 1024 * 1024;

/// Run a command with bounded stdout capture.
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

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            captured.extend_from_slice(&buf[..count]);
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

pub(super) fn run_spawned_command(
    mut command: std::process::Command,
    tracked_input: &str,
    tracked_output: &str,
    timer: tracking::TimedExecution,
) -> Result<()> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::thread;

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute command")?;

    let stdout_pipe = child
        .stdout
        .take()
        .context("Failed to capture child stdout")?;
    let stderr_pipe = child
        .stderr
        .take()
        .context("Failed to capture child stderr")?;

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

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            captured.extend_from_slice(&buf[..count]);
            let mut err = std::io::stderr().lock();
            err.write_all(&buf[..count])?;
            err.flush()?;
        }

        Ok(captured)
    });

    let status = child.wait().context("Failed waiting for command")?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;

    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let full_output = format!("{}{}", stdout, stderr);

    // Route raw stdout through hyphae for chunked storage before any ContentRouter filtering.
    // This ensures hyphae stores the complete unfiltered command output.
    let hyphae_output = if crate::hyphae::is_available() {
        let result = crate::hyphae::route_or_filter(&tracked_input, &stdout, |r| {
            crate::filter::FilterResult::full(r, r.to_string())
        });
        result.output
    } else {
        // Hyphae not available — use raw output for display if no ContentRouter filter applies
        stdout.to_string()
    };

    // Apply content-aware routing to stdout for display filtering.
    // Built-in (compiled) filter runs first and always takes precedence.
    let router = ContentRouter::default();
    let routed_stdout = router.route(&stdout);

    // Apply TOML declarative filter as a fallback when the built-in filter
    // produced no transformation. Load filters once here to avoid repeated
    // disk reads inside tight loops; cwd is captured at command-dispatch time.
    let filtered_for_display = if routed_stdout == stdout.as_ref() {
        // Built-in filter was a no-op — consult TOML filters.
        let (project_filters, user_filters) = crate::filters::load_all_declarative_filters();
        if let Some(matched) =
            crate::filters::find_matching_filter(tracked_input, &project_filters, &user_filters)
        {
            let toml_result = matched.apply(&stdout);
            toml_result.output
        } else {
            routed_stdout
        }
    } else {
        routed_stdout
    };

    // Choose what to print: if hyphae handled it, use hyphae's summary; otherwise use the
    // ContentRouter-filtered text for display.
    let output_to_print = if crate::hyphae::is_available() {
        hyphae_output
    } else {
        filtered_for_display.to_string()
    };

    // Print the final output after all filtering is complete.
    // Stderr is already streamed live by the capture thread above.
    print!("{output_to_print}");

    // Append MYCELIUM_EXPLAIN annotation if enabled and command was rewritten
    if std::env::var("MYCELIUM_EXPLAIN").is_ok() {
        let resolution = rewrite_cmd::resolve_runtime_command(tracked_input);
        if resolution.rewritten {
            if !output_to_print.is_empty() {
                println!();
            }
            print!(
                "[mycelium] filter: {} | savings: ~{:.0}%",
                resolution.source,
                resolution.estimated_savings_pct.unwrap_or(0.0)
            );
        }
    }

    // Track using the ContentRouter-filtered text (not hyphae's summary output) for accurate token tracking.
    // This ensures token savings are computed against the actual filtered output users see, not hyphae chunk summaries.
    let final_full = format!(
        "{}{}",
        filtered_for_display,
        String::from_utf8_lossy(&stderr_bytes)
    );
    timer.track(tracked_input, tracked_output, &full_output, &final_full);

    if !status.success() {
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
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
        Commands::Ls { .. } => "ls",
        Commands::Tree { .. } => "tree",
        Commands::Read { .. } => "read",
        Commands::Peek { .. } => "peek",
        Commands::Git { .. } => "git",
        Commands::Gh { .. } => "gh",
        Commands::Gt { .. } => "gt",
        Commands::Cargo { .. } => "cargo",
        Commands::Tsc { .. } => "tsc",
        Commands::Next { .. } => "next",
        Commands::Go { .. } => "go",
        Commands::Lint { .. } => "lint",
        Commands::Prettier { .. } => "prettier",
        Commands::Format { .. } => return false, // not operational
        Commands::Ruff { .. } => "ruff",
        Commands::Mypy { .. } => return false, // not operational
        Commands::GolangciLint { .. } => "golangci-lint",
        Commands::Test { .. } => "test",
        Commands::Vitest { .. } => "vitest",
        Commands::Playwright { .. } => "playwright",
        Commands::Pytest { .. } => "pytest",
        Commands::Pnpm { .. } => "pnpm",
        Commands::Pip { .. } => "pip",
        Commands::Npm { .. } => "npm",
        Commands::Npx { .. } => "npx",
        Commands::Psql { .. } => return false, // not operational
        Commands::Prisma { .. } => "prisma",
        Commands::Curl { .. } => "curl",
        Commands::Wget { .. } => "wget",
        Commands::Docker { .. } => "docker",
        Commands::Kubectl { .. } => "kubectl",
        Commands::Terraform { .. } => return false, // not operational
        Commands::Aws { .. } => return false,       // not operational
        Commands::Atmos { .. } => return false,     // not operational
        Commands::Json { .. } => "json",
        Commands::Log { .. } => "log",
        Commands::Err { .. } => "err",
        Commands::Summary { .. } => "summary",
        Commands::Env { .. } => "env",
        Commands::Deps { .. } => "deps",
        Commands::Gain { .. } => return false, // not operational
        #[cfg(unix)]
        Commands::ServeSocket { .. } => return false, // not operational
        Commands::Discover { .. } => return false, // not operational
        Commands::Learn { .. } => return false, // not operational
        Commands::Context { .. } => return false, // not operational
        Commands::Init { .. } => return false, // not operational
        Commands::Config { .. } => return false, // not operational
        Commands::Doctor => return false,      // not operational
        Commands::Verify => return false,      // not operational
        Commands::SelfUpdate { .. } => return false, // not operational
        Commands::Completions { .. } => return false, // not operational
        Commands::Proxy { .. } => return false, // not operational (handled separately)
        Commands::Invoke { .. } => "invoke",
        Commands::Benchmark { .. } => return false, // not operational
        Commands::Plugin { .. } => return false,    // not operational
        Commands::Wc { .. } => return false,        // not operational
        Commands::ParseHealth { .. } => return false, // not operational
        Commands::CcEconomics { .. } => return false, // not operational
        Commands::HookAudit { .. } => return false, // not operational
        Commands::Rewrite { .. } => return false,   // not operational
        Commands::Explain { .. } => return false,   // not operational
        Commands::Find { .. } => "find",
        Commands::Grep { .. } => "grep",
        Commands::Diff { .. } => "diff",
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
        let test_cases: Vec<(&str, Commands)> = vec![
            ("ls", Commands::Ls { args: vec![] }),
            ("tree", Commands::Tree { args: vec![] }),
            ("npm", Commands::Npm { args: vec![] }),
            ("npm", Commands::Npm { args: vec![] }),
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
            let raw_result = std::process::Command::new(tool_name)
                .args(&args[1..])
                .output();
            match raw_result {
                Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
                Err(_) => String::new(),
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mycelium_exe = std::env::current_exe().context("Failed to locate mycelium executable")?;
    let filtered_result = std::process::Command::new(&mycelium_exe)
        .args(&args)
        .output();

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
