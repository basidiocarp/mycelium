//! Passthrough execution helpers for GitHub CLI commands.
//!
//! These functions execute gh commands without filtering, tracking metrics.

use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

/// Pass any `gh` arguments through without filtering, tracking metrics.
pub fn run_passthrough_gh(args: &[&str], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut command = Command::new("gh");
    for arg in args {
        command.arg(arg);
    }

    let status = command.status().context("Failed to run gh (passthrough)")?;

    let args_str = args.join(" ");
    timer.track_passthrough(
        &format!("gh {}", args_str),
        &format!("mycelium gh {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Pass through a command with base args + extra args, tracking as passthrough.
pub(crate) fn run_passthrough_with_extra(
    cmd: &str,
    base_args: &[&str],
    extra_args: &[String],
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut command = Command::new(cmd);
    for arg in base_args {
        command.arg(arg);
    }
    for arg in extra_args {
        command.arg(arg);
    }

    let status =
        command
            .status()
            .context(format!("Failed to run {} {}", cmd, base_args.join(" ")))?;

    let full_cmd = format!(
        "{} {} {}",
        cmd,
        base_args.join(" "),
        tracking::args_display(&extra_args.iter().map(|s| s.into()).collect::<Vec<_>>())
    );
    timer.track_passthrough(&full_cmd, &format!("mycelium {} (passthrough)", full_cmd));

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Public wrapper for run_passthrough, accessible from sibling modules.
pub(crate) fn run_passthrough_fn(cmd: &str, subcommand: &str, args: &[String]) -> Result<()> {
    run_passthrough(cmd, subcommand, args)
}

pub(super) fn run_passthrough(cmd: &str, subcommand: &str, args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut command = Command::new(cmd);
    command.arg(subcommand);
    for arg in args {
        command.arg(arg);
    }

    let status = command
        .status()
        .context(format!("Failed to run {} {}", cmd, subcommand))?;

    let args_str = tracking::args_display(&args.iter().map(|s| s.into()).collect::<Vec<_>>());
    timer.track_passthrough(
        &format!("{} {} {}", cmd, subcommand, args_str),
        &format!("mycelium {} {} {} (passthrough)", cmd, subcommand, args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
