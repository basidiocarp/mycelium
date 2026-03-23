//! PR checks and status subcommand handlers.

use crate::{tracking, utils::truncate};
use anyhow::{Context, Result};
use std::process::Command;

use crate::vcs::gh_cmd::{extract_optional_identifier_and_extra_args, run_passthrough_with_extra};

pub fn should_passthrough_pr_checks(extra_args: &[String]) -> bool {
    extra_args.iter().any(|a| {
        a == "--json"
            || a == "--jq"
            || a == "--template"
            || a == "--web"
            || a == "--watch"
            || a == "--fail-fast"
    })
}

pub fn pr_checks(args: &[String], _verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (pr_number, extra_args) = extract_optional_identifier_and_extra_args(args);
    let base_args = if let Some(ref pr_number) = pr_number {
        vec!["pr", "checks", pr_number.as_str()]
    } else {
        vec!["pr", "checks"]
    };

    if should_passthrough_pr_checks(&extra_args) {
        return run_passthrough_with_extra("gh", &base_args, &extra_args);
    }

    let mut cmd = Command::new("gh");
    cmd.args(&base_args);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr checks")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let pr_label = pr_number
            .as_deref()
            .map(|n| format!("gh pr checks {}", n))
            .unwrap_or_else(|| "gh pr checks".to_string());
        let mycelium_label = pr_number
            .as_deref()
            .map(|n| format!("mycelium gh pr checks {}", n))
            .unwrap_or_else(|| "mycelium gh pr checks".to_string());
        timer.track(&pr_label, &mycelium_label, &stderr, &stderr);
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

    let pr_label = pr_number
        .as_deref()
        .map(|n| format!("gh pr checks {}", n))
        .unwrap_or_else(|| "gh pr checks".to_string());
    let mycelium_label = pr_number
        .as_deref()
        .map(|n| format!("mycelium gh pr checks {}", n))
        .unwrap_or_else(|| "mycelium gh pr checks".to_string());
    timer.track(&pr_label, &mycelium_label, &raw, &filtered);
    Ok(())
}

pub fn pr_status(args: &[String], _verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if should_passthrough_pr_status(args) {
        return run_passthrough_with_extra("gh", &["pr", "status"], args);
    }

    let mut cmd = Command::new("gh");
    cmd.args([
        "pr",
        "status",
        "--json",
        "currentBranch,createdBy,reviewDecision,statusCheckRollup",
    ]);
    for arg in args {
        cmd.arg(arg);
    }

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

fn should_passthrough_pr_status(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--jq" || a == "--template" || a == "--json")
}

#[cfg(test)]
mod tests {
    use super::{should_passthrough_pr_checks, should_passthrough_pr_status};

    #[test]
    fn test_should_passthrough_pr_checks_json_template_watch_web() {
        assert!(should_passthrough_pr_checks(&["--json".into()]));
        assert!(should_passthrough_pr_checks(&[
            "--jq".into(),
            ".state".into()
        ]));
        assert!(should_passthrough_pr_checks(&[
            "--template".into(),
            "{{.state}}".into()
        ]));
        assert!(should_passthrough_pr_checks(&["--web".into()]));
        assert!(should_passthrough_pr_checks(&["--watch".into()]));
        assert!(should_passthrough_pr_checks(&["--fail-fast".into()]));
    }

    #[test]
    fn test_should_passthrough_pr_checks_default() {
        assert!(!should_passthrough_pr_checks(&[]));
    }

    #[test]
    fn test_should_passthrough_pr_status_json_template() {
        assert!(should_passthrough_pr_status(&["--json".into()]));
        assert!(should_passthrough_pr_status(&[
            "--jq".into(),
            ".createdBy".into()
        ]));
        assert!(should_passthrough_pr_status(&[
            "--template".into(),
            "{{.createdBy}}".into()
        ]));
    }

    #[test]
    fn test_should_passthrough_pr_status_default() {
        assert!(!should_passthrough_pr_status(&[]));
    }
}
