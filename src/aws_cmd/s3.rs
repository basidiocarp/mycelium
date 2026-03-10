//! AWS S3 – Simple Storage Service handler.

use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::generic::MAX_ITEMS;

/// AWS S3 – Simple Storage Service.
///
/// `args` is the operation and its flags, e.g. `["ls"]`.
pub fn run_s3(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("ls") => run_s3_ls(&args[1..], verbose),
        _ => super::generic::run_generic("s3", args, verbose),
    }
}

fn run_s3_ls(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // s3 ls doesn't support --output json, run as-is and filter text
    let mut cmd = Command::new("aws");
    cmd.args(["s3", "ls"]);
    for arg in extra_args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: aws s3 ls {}", extra_args.join(" "));
    }

    let output = cmd.output().context("Failed to run aws s3 ls")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track("aws s3 ls", "mycelium aws s3 ls", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let filtered = filter_s3_ls(&raw);
    println!("{}", filtered);

    timer.track("aws s3 ls", "mycelium aws s3 ls", &raw, &filtered);
    Ok(())
}

fn filter_s3_ls(output: &str) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let total = lines.len();
    let mut result: Vec<&str> = lines.iter().take(MAX_ITEMS + 10).copied().collect();

    if total > MAX_ITEMS + 10 {
        result.truncate(MAX_ITEMS + 10);
        result.push(""); // will be replaced
        return format!(
            "{}\n... +{} more items",
            result[..result.len() - 1].join("\n"),
            total - MAX_ITEMS - 10
        );
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_s3_ls_basic() {
        let output = "2024-01-01 bucket1\n2024-01-02 bucket2\n2024-01-03 bucket3\n";
        let result = filter_s3_ls(output);
        assert!(result.contains("bucket1"));
        assert!(result.contains("bucket3"));
    }

    #[test]
    fn test_filter_s3_ls_overflow() {
        let mut lines = Vec::new();
        for i in 1..=50 {
            lines.push(format!("2024-01-01 bucket{}", i));
        }
        let input = lines.join("\n");
        let result = filter_s3_ls(&input);
        assert!(result.contains("... +20 more items"));
    }
}
