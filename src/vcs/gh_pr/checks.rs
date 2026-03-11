//! PR checks and status subcommand handlers.

use crate::{tracking, utils::truncate};
use anyhow::{Context, Result};
use std::process::Command;

pub fn pr_checks(args: &[String], _verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (pr_number, extra_args) = match crate::vcs::gh_cmd::extract_identifier_and_extra_args(args)
    {
        Some(result) => result,
        None => return Err(anyhow::anyhow!("PR number required")),
    };

    let mut cmd = Command::new("gh");
    cmd.args(["pr", "checks", &pr_number]);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr checks")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track(
            &format!("gh pr checks {}", pr_number),
            &format!("mycelium gh pr checks {}", pr_number),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    let mut failed_checks = Vec::new();

    for line in stdout.lines() {
        if line.contains('✓') || line.contains("pass") {
            passed += 1;
        } else if line.contains('✗') || line.contains("fail") {
            failed += 1;
            failed_checks.push(line.trim().to_string());
        } else if line.contains('*') || line.contains("pending") {
            pending += 1;
        }
    }

    let mut filtered = String::new();

    let line = "CI Checks Summary:\n";
    filtered.push_str(line);
    print!("{}", line);

    let line = format!("  Passed: {}\n", passed);
    filtered.push_str(&line);
    print!("{}", line);

    let line = format!("  Failed: {}\n", failed);
    filtered.push_str(&line);
    print!("{}", line);

    if pending > 0 {
        let line = format!("  Pending: {}\n", pending);
        filtered.push_str(&line);
        print!("{}", line);
    }

    if !failed_checks.is_empty() {
        let line = "\n  Failed checks:\n";
        filtered.push_str(line);
        print!("{}", line);
        for check in failed_checks {
            let line = format!("    {}\n", check);
            filtered.push_str(&line);
            print!("{}", line);
        }
    }

    timer.track(
        &format!("gh pr checks {}", pr_number),
        &format!("mycelium gh pr checks {}", pr_number),
        &raw,
        &filtered,
    );
    Ok(())
}

pub fn pr_status(_verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args([
        "pr",
        "status",
        "--json",
        "currentBranch,createdBy,reviewDecision,statusCheckRollup",
    ]);

    let output = cmd.output().context("Failed to run gh pr status")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track("gh pr status", "mycelium gh pr status", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse gh pr status output")?;

    let mut filtered = String::new();

    if let Some(created_by) = json["createdBy"].as_array() {
        let line = format!("Your PRs ({}):\n", created_by.len());
        filtered.push_str(&line);
        print!("{}", line);
        for pr in created_by.iter().take(5) {
            let number = pr["number"].as_i64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("???");
            let reviews = pr["reviewDecision"].as_str().unwrap_or("PENDING");
            let line = format!("  #{} {} [{}]\n", number, truncate(title, 50), reviews);
            filtered.push_str(&line);
            print!("{}", line);
        }
    }

    timer.track("gh pr status", "mycelium gh pr status", &raw, &filtered);
    Ok(())
}
