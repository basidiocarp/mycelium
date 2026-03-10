//! Shared helpers for AWS CLI output compression.
//!
//! `run_generic` handles any unrecognised service/operation.
//! `run_aws_json` is the low-level executor used by every service handler.

use crate::json_cmd;
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

pub(super) const MAX_ITEMS: usize = 20;
pub(super) const JSON_COMPRESS_DEPTH: usize = 4;

/// Returns true for operations that return structured JSON (describe-*, list-*, get-*).
///
/// Mutating/transfer operations (s3 cp, s3 sync, s3 mb, etc.) emit plain text progress
/// and do not accept --output json, so we must not inject it for them.
pub(super) fn is_structured_operation(args: &[String]) -> bool {
    let op = args.first().map(|s| s.as_str()).unwrap_or("");
    op.starts_with("describe-") || op.starts_with("list-") || op.starts_with("get-")
}

/// Generic passthrough strategy: force `--output json` for structured read ops, then compress.
///
/// Used for any AWS service or operation that doesn't have a specialised handler.
/// `args` is the operation + its flags (same convention as the per-service entry points).
pub fn run_generic(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    let full_sub = if args.is_empty() {
        subcommand.to_string()
    } else {
        format!("{} {}", subcommand, args.join(" "))
    };

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("aws");
    cmd.arg(subcommand);

    let mut has_output_flag = false;
    for arg in args {
        if arg == "--output" {
            has_output_flag = true;
        }
        cmd.arg(arg);
    }

    // Only inject --output json for structured read operations.
    // Mutating/transfer operations (s3 cp, s3 sync, s3 mb, cloudformation deploy…)
    // emit plain-text progress and reject --output json.
    if !has_output_flag && is_structured_operation(args) {
        cmd.args(["--output", "json"]);
    }

    if verbose > 0 {
        eprintln!("Running: aws {}", full_sub);
    }

    let output = cmd.output().context("Failed to run aws CLI")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        timer.track(
            &format!("aws {}", full_sub),
            &format!("mycelium aws {}", full_sub),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let filtered = match json_cmd::filter_json_string(&raw, JSON_COMPRESS_DEPTH) {
        Ok(schema) => {
            println!("{}", schema);
            schema
        }
        Err(_) => {
            // Fallback: print raw (maybe not JSON)
            print!("{}", raw);
            raw.clone()
        }
    };

    timer.track(
        &format!("aws {}", full_sub),
        &format!("mycelium aws {}", full_sub),
        &raw,
        &filtered,
    );

    Ok(())
}

/// Execute an AWS CLI command, forcing `--output json`, and return raw stdout/stderr/status.
///
/// Strips any existing `--output` flag from `extra_args` to avoid conflicts.
pub(super) fn run_aws_json(
    sub_args: &[&str],
    extra_args: &[String],
    verbose: u8,
) -> Result<(String, String, std::process::ExitStatus)> {
    let mut cmd = Command::new("aws");
    for arg in sub_args {
        cmd.arg(arg);
    }

    // Replace --output table/text with --output json
    let mut skip_next = false;
    for arg in extra_args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--output" {
            skip_next = true;
            continue;
        }
        cmd.arg(arg);
    }
    cmd.args(["--output", "json"]);

    let cmd_desc = format!("aws {}", sub_args.join(" "));
    if verbose > 0 {
        eprintln!("Running: {}", cmd_desc);
    }

    let output = cmd
        .output()
        .context(format!("Failed to run {}", cmd_desc))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        eprintln!("{}", stderr.trim());
    }

    Ok((stdout, stderr, output.status))
}
