//! Token-optimized proxy for Graphite (gt) CLI stacking workflow commands.
mod filters;
use filters::{
    filter_gt_create, filter_gt_log_entries, filter_gt_restack, filter_gt_submit, filter_gt_sync,
    filter_identity,
};

use crate::tracking;
use crate::utils::strip_ansi;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

fn run_gt_filtered(
    subcmd: &[&str],
    args: &[String],
    verbose: u8,
    tee_label: &str,
    filter_fn: fn(&str) -> String,
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gt");
    for part in subcmd {
        cmd.arg(part);
    }
    for arg in args {
        cmd.arg(arg);
    }

    let subcmd_str = subcmd.join(" ");
    if verbose > 0 {
        eprintln!("Running: gt {} {}", subcmd_str, args.join(" "));
    }

    let cmd_output = cmd.output().with_context(|| {
        format!(
            "Failed to run gt {}. Is gt (Graphite) installed?",
            subcmd_str
        )
    })?;

    let stdout = String::from_utf8_lossy(&cmd_output.stdout);
    let stderr = String::from_utf8_lossy(&cmd_output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let exit_code = cmd_output.status.code().unwrap_or(1);

    let clean = strip_ansi(stdout.trim());
    let output = if verbose > 0 {
        clean.clone()
    } else {
        filter_fn(&clean)
    };

    if let Some(hint) = crate::tee::tee_and_hint(&raw, tee_label, exit_code) {
        println!("{}\n{}", output, hint);
    } else {
        println!("{}", output);
    }

    if !stderr.trim().is_empty() {
        eprintln!("{}", stderr.trim());
    }

    let label = if args.is_empty() {
        format!("gt {}", subcmd_str)
    } else {
        format!("gt {} {}", subcmd_str, args.join(" "))
    };
    let mycelium_label = format!("mycelium {}", label);
    timer.track(&label, &mycelium_label, &raw, &output);

    if !cmd_output.status.success() {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Execute `gt log` with email stripping and entry truncation.
pub fn run_log(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(|s| s.as_str()) {
        Some("short") => run_gt_filtered(
            &["log", "short"],
            &args[1..],
            verbose,
            "gt_log_short",
            filter_identity,
        ),
        Some("long") => run_gt_filtered(
            &["log", "long"],
            &args[1..],
            verbose,
            "gt_log_long",
            filter_gt_log_entries,
        ),
        _ => run_gt_filtered(&["log"], args, verbose, "gt_log", filter_gt_log_entries),
    }
}

/// Execute `gt submit` and summarize pushed branches and PR actions.
pub fn run_submit(args: &[String], verbose: u8) -> Result<()> {
    run_gt_filtered(&["submit"], args, verbose, "gt_submit", filter_gt_submit)
}

/// Execute `gt sync` and report synced/deleted branch counts.
pub fn run_sync(args: &[String], verbose: u8) -> Result<()> {
    run_gt_filtered(&["sync"], args, verbose, "gt_sync", filter_gt_sync)
}

/// Execute `gt restack` and report restacked branch count.
pub fn run_restack(args: &[String], verbose: u8) -> Result<()> {
    run_gt_filtered(&["restack"], args, verbose, "gt_restack", filter_gt_restack)
}

/// Execute `gt create` and report the created branch name.
pub fn run_create(args: &[String], verbose: u8) -> Result<()> {
    run_gt_filtered(&["create"], args, verbose, "gt_create", filter_gt_create)
}

/// Execute `gt branch` with identity passthrough (no filtering).
pub fn run_branch(args: &[String], verbose: u8) -> Result<()> {
    run_gt_filtered(&["branch"], args, verbose, "gt_branch", filter_identity)
}

/// Handle unrecognized `gt` subcommands, routing known git commands to Mycelium filters.
pub fn run_other(args: &[OsString], verbose: u8) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("gt: no subcommand specified");
    }

    let subcommand = args[0].to_string_lossy();
    let rest: Vec<String> = args[1..]
        .iter()
        .map(|a| a.to_string_lossy().into())
        .collect();

    // gt passes unknown subcommands to git, so "gt status" = "git status".
    // Route known git commands to Mycelium's git filters for token savings.
    match subcommand.as_ref() {
        "status" => crate::git::run(crate::git::GitCommand::Status, &rest, None, verbose, &[]),
        "diff" => crate::git::run(crate::git::GitCommand::Diff, &rest, None, verbose, &[]),
        "show" => crate::git::run(crate::git::GitCommand::Show, &rest, None, verbose, &[]),
        "add" => crate::git::run(crate::git::GitCommand::Add, &rest, None, verbose, &[]),
        "push" => crate::git::run(crate::git::GitCommand::Push, &rest, None, verbose, &[]),
        "pull" => crate::git::run(crate::git::GitCommand::Pull, &rest, None, verbose, &[]),
        "fetch" => crate::git::run(crate::git::GitCommand::Fetch, &rest, None, verbose, &[]),
        "stash" => {
            let stash_sub = rest.first().cloned();
            let stash_args = rest.get(1..).unwrap_or(&[]);
            crate::git::run(
                crate::git::GitCommand::Stash {
                    subcommand: stash_sub,
                },
                stash_args,
                None,
                verbose,
                &[],
            )
        }
        "worktree" => crate::git::run(crate::git::GitCommand::Worktree, &rest, None, verbose, &[]),
        _ => passthrough_gt(&subcommand, &rest, verbose),
    }
}

fn passthrough_gt(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gt");
    cmd.arg(subcommand);
    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: gt {} {}", subcommand, args.join(" "));
    }

    let status = cmd
        .status()
        .with_context(|| format!("Failed to run gt {}", subcommand))?;

    let args_str = if args.is_empty() {
        subcommand.to_string()
    } else {
        format!("{} {}", subcommand, args.join(" "))
    };
    timer.track_passthrough(
        &format!("gt {}", args_str),
        &format!("mycelium gt {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
