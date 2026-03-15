//! Log handler for git command proxy.
use crate::tracking;
use crate::vcs::git_filters::filter_log_output;
use anyhow::{Context, Result};

pub(super) fn run_log(
    args: &[String],
    _max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Load config to get log_max_commits setting
    let config_max_commits = crate::config::Config::load()
        .ok()
        .and_then(|c| c.filters.git.as_ref().map(|g| g.log_max_commits))
        .unwrap_or(15);

    let mut cmd = super::git_cmd(global_args);
    cmd.arg("log");

    // Check if user provided format flags
    let has_format_flag = args.iter().any(|arg| {
        arg.starts_with("--oneline") || arg.starts_with("--pretty") || arg.starts_with("--format")
    });

    // Check if user provided limit flag
    let has_limit_flag = args
        .iter()
        .any(|arg| arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit()));

    // Apply Mycelium defaults only if user didn't specify them
    if !has_format_flag {
        cmd.args(["--pretty=format:%h %s (%ar) <%an>"]);
    }

    let limit = if !has_limit_flag {
        cmd.arg(format!("-{}", config_max_commits));
        config_max_commits
    } else {
        // Extract limit from args if provided
        args.iter()
            .find(|arg| {
                arg.starts_with('-') && arg.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
            })
            .and_then(|arg| arg[1..].parse::<usize>().ok())
            .unwrap_or(config_max_commits)
    };

    // Pass all user arguments
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git log")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr);
        // Propagate git's exit code
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if verbose > 0 {
        eprintln!("Git log output:");
    }

    // Post-process: truncate long messages, cap lines
    let filtered = filter_log_output(&stdout, limit);
    println!("{}", filtered);

    timer.track(
        &format!("git log {}", args.join(" ")),
        &format!("mycelium git log {}", args.join(" ")),
        &stdout,
        &filtered,
    );

    Ok(())
}
