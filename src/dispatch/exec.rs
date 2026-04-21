use anyhow::{Context, Result};

use crate::commands::{Cli, Commands};
use crate::{json_output, rewrite_cmd, tracking};

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
            captured.extend_from_slice(&buf[..count]);
            let mut out = std::io::stdout().lock();
            out.write_all(&buf[..count])?;
            out.flush()?;
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

    timer.track(tracked_input, tracked_output, &full_output, &full_output);

    if !status.success() {
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Returns true for commands that are invoked via the hook pipeline
/// (i.e., commands that process rewritten shell commands).
/// Meta commands (init, gain, verify, etc.) are excluded because
/// they are run directly by the user, not through the hook.
///
/// SECURITY: whitelist pattern — new commands are NOT integrity-checked
/// until explicitly added here. A forgotten command fails open (no check)
/// rather than creating false confidence about what's protected.
pub fn is_operational_command(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Ls { .. }
            | Commands::Tree { .. }
            | Commands::Read { .. }
            | Commands::Peek { .. }
            | Commands::Git { .. }
            | Commands::Gh { .. }
            | Commands::Pnpm { .. }
            | Commands::Err { .. }
            | Commands::Test { .. }
            | Commands::Json { .. }
            | Commands::Deps { .. }
            | Commands::Env { .. }
            | Commands::Find { .. }
            | Commands::Diff { .. }
            | Commands::Log { .. }
            | Commands::Docker { .. }
            | Commands::Kubectl { .. }
            | Commands::Summary { .. }
            | Commands::Grep { .. }
            | Commands::Wget { .. }
            | Commands::Vitest { .. }
            | Commands::Prisma { .. }
            | Commands::Tsc { .. }
            | Commands::Next { .. }
            | Commands::Lint { .. }
            | Commands::Prettier { .. }
            | Commands::Playwright { .. }
            | Commands::Cargo { .. }
            | Commands::Npm { .. }
            | Commands::Npx { .. }
            | Commands::Curl { .. }
            | Commands::Ruff { .. }
            | Commands::Pytest { .. }
            | Commands::Pip { .. }
            | Commands::Go { .. }
            | Commands::GolangciLint { .. }
            | Commands::Gt { .. }
            | Commands::Invoke { .. }
    )
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
        let allowed_tools = [
            "git", "gh", "cargo", "npm", "pnpm", "yarn", "npx", "ls", "grep", "tree",
            "cat", "find", "docker", "kubectl", "curl", "wget", "vitest", "pytest",
            "ruff", "go", "tsc", "next", "prettier", "playwright", "prisma",
        ];

        let base_name = std::path::Path::new(tool_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(tool_name);

        if allowed_tools.iter().any(|t| base_name == *t) {
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

    let envelope = match filtered_result {
        Ok(output) if output.status.success() || !output.stdout.is_empty() => {
            let filtered = String::from_utf8_lossy(&output.stdout).to_string();
            json_output::wrap_output(
                &original_cmd,
                &format!("mycelium {original_cmd}"),
                &filtered,
                &raw_output,
                project_path.as_deref(),
                Some(&rewrite_resolution),
            )
        }
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            json_output::wrap_error(&stderr, exit_code)
        }
        Err(e) => json_output::wrap_error(&e.to_string(), 1),
    };

    let _ = cli;

    println!("{envelope}");
    Ok(())
}
