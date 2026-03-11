//! GitHub CLI workflow run sub-command handlers.

use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::parsers::GhRunListParser;
use super::passthrough::{run_passthrough, run_passthrough_with_extra};

pub(super) fn run_workflow(args: &[String], verbose: u8, ultra_compact: bool) -> Result<()> {
    if args.is_empty() {
        return run_passthrough("gh", "run", args);
    }

    match args[0].as_str() {
        "list" => list_runs(&args[1..], verbose, ultra_compact),
        "view" => view_run(&args[1..], verbose),
        _ => run_passthrough("gh", "run", args),
    }
}

fn list_runs(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args([
        "run",
        "list",
        "--json",
        "databaseId,name,status,conclusion,createdAt",
    ]);
    cmd.arg("--limit").arg("10");

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh run list")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            "gh run list",
            "mycelium gh run list",
            &stderr,
            &stderr,
            3,
            "compact",
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let mode = if ultra_compact {
        FormatMode::Ultra
    } else {
        FormatMode::Compact
    };
    let format_mode_str = if ultra_compact { "ultra" } else { "compact" };

    let parse_result = GhRunListParser::parse(&raw);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };

    let filtered = match parse_result {
        ParseResult::Full(list) | ParseResult::Degraded(list, _) => {
            let out = list.format(mode);
            println!("{}", out);
            out
        }
        ParseResult::Passthrough(raw_out) => {
            print!("{}", raw_out);
            raw_out
        }
    };

    timer.track_with_parse_info(
        "gh run list",
        "mycelium gh run list",
        &raw,
        &filtered,
        parse_tier,
        format_mode_str,
    );
    Ok(())
}

/// Check if run view args should bypass filtering and pass through directly.
/// Flags like --log-failed, --log, and --json produce output that the filter
/// would incorrectly strip.
fn should_passthrough_run_view(extra_args: &[String]) -> bool {
    extra_args
        .iter()
        .any(|a| a == "--log-failed" || a == "--log" || a == "--json")
}

fn view_run(args: &[String], _verbose: u8) -> Result<()> {
    let (run_id, extra_args) = match super::extract_identifier_and_extra_args(args) {
        Some(result) => result,
        None => return Err(anyhow::anyhow!("Run ID required")),
    };

    // Pass through when user requests logs or JSON — the filter would strip them
    if should_passthrough_run_view(&extra_args) {
        return run_passthrough_with_extra("gh", &["run", "view", &run_id], &extra_args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(["run", "view", &run_id]);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh run view")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            &format!("gh run view {}", run_id),
            &format!("mycelium gh run view {}", run_id),
            &stderr,
            &stderr,
            3,
            "compact",
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    // Parse output and show only failures
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut in_jobs = false;

    let mut filtered = String::new();

    let line = format!("Workflow Run #{}\n", run_id);
    filtered.push_str(&line);
    print!("{}", line);

    for line in stdout.lines() {
        if line.contains("JOBS") {
            in_jobs = true;
        }

        if in_jobs {
            if line.contains('✓') || line.contains("success") {
                // Skip successful jobs in compact mode
                continue;
            }
            if line.contains('✗') || line.contains("fail") {
                let formatted = format!("  FAIL: {}\n", line.trim());
                filtered.push_str(&formatted);
                print!("{}", formatted);
            }
        } else if line.contains("Status:") || line.contains("Conclusion:") {
            let formatted = format!("  {}\n", line.trim());
            filtered.push_str(&formatted);
            print!("{}", formatted);
        }
    }

    timer.track_with_parse_info(
        &format!("gh run view {}", run_id),
        &format!("mycelium gh run view {}", run_id),
        &raw,
        &filtered,
        1,
        "compact",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_view_passthrough_log_failed() {
        assert!(should_passthrough_run_view(&["--log-failed".into()]));
    }

    #[test]
    fn test_run_view_passthrough_log() {
        assert!(should_passthrough_run_view(&["--log".into()]));
    }

    #[test]
    fn test_run_view_passthrough_json() {
        assert!(should_passthrough_run_view(&[
            "--json".into(),
            "jobs".into()
        ]));
    }

    #[test]
    fn test_run_view_no_passthrough_empty() {
        assert!(!should_passthrough_run_view(&[]));
    }

    #[test]
    fn test_run_view_no_passthrough_other_flags() {
        assert!(!should_passthrough_run_view(&["--web".into()]));
    }
}
