//! PR create, merge, diff, and generic action subcommand handlers.

use crate::vcs::gh_cmd::run_passthrough_with_extra;
use crate::{tracking, utils::ok_confirmation, vcs::git_filters};
use anyhow::{Context, Result};
use std::process::Command;

pub fn pr_create(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(["pr", "create"]);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr create")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        timer.track("gh pr create", "mycelium gh pr create", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let url = stdout.trim();
    let pr_num = url.rsplit('/').next().unwrap_or("");

    let detail = if !pr_num.is_empty() && pr_num.chars().all(|c| c.is_ascii_digit()) {
        format!("#{} {}", pr_num, url)
    } else {
        url.to_string()
    };

    let filtered = ok_confirmation("created", &detail);
    println!("{}", filtered);

    timer.track("gh pr create", "mycelium gh pr create", &stdout, &filtered);
    Ok(())
}

pub fn pr_merge(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(["pr", "merge"]);
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr merge")?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        timer.track("gh pr merge", "mycelium gh pr merge", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let pr_num = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .unwrap_or("");

    let detail = if !pr_num.is_empty() {
        format!("#{}", pr_num)
    } else {
        String::new()
    };

    let filtered = ok_confirmation("merged", &detail);
    println!("{}", filtered);

    let raw = if !stdout.trim().is_empty() {
        stdout
    } else {
        detail.clone()
    };

    timer.track("gh pr merge", "mycelium gh pr merge", &raw, &filtered);
    Ok(())
}

pub fn pr_diff(args: &[String], _verbose: u8) -> Result<()> {
    // --no-compact: pass full diff through (gh CLI doesn't know this flag, strip it)
    let no_compact = args.iter().any(|a| a == "--no-compact");
    let gh_args: Vec<String> = args
        .iter()
        .filter(|a| *a != "--no-compact")
        .cloned()
        .collect();

    if no_compact {
        return run_passthrough_with_extra("gh", &["pr", "diff"], &gh_args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(["pr", "diff"]);
    for arg in gh_args.iter() {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr diff")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track("gh pr diff", "mycelium gh pr diff", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let filtered = if raw.trim().is_empty() {
        let msg = "No diff\n";
        print!("{}", msg);
        msg.to_string()
    } else {
        let compacted = git_filters::compact_diff(&raw, 500);
        println!("{}", compacted);
        compacted
    };

    timer.track("gh pr diff", "mycelium gh pr diff", &raw, &filtered);
    Ok(())
}

/// Generic PR action handler for comment/edit
pub fn pr_action(action: &str, args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let subcmd = &args[0];

    let mut cmd = Command::new("gh");
    cmd.arg("pr");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .context(format!("Failed to run gh pr {}", subcmd))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track(
            &format!("gh pr {}", subcmd),
            &format!("mycelium gh pr {}", subcmd),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let pr_num = args[1..]
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(|s| format!("#{}", s))
        .unwrap_or_default();

    let filtered = ok_confirmation(action, &pr_num);
    println!("{}", filtered);

    let raw = if !stdout.trim().is_empty() {
        stdout
    } else {
        pr_num.clone()
    };

    timer.track(
        &format!("gh pr {}", subcmd),
        &format!("mycelium gh pr {}", subcmd),
        &raw,
        &filtered,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::ok_confirmation;

    #[test]
    fn test_ok_confirmation_pr_create() {
        let result = ok_confirmation("created", "#42 https://github.com/foo/bar/pull/42");
        assert!(result.contains("ok created"));
        assert!(result.contains("#42"));
    }

    #[test]
    fn test_ok_confirmation_pr_merge() {
        let result = ok_confirmation("merged", "#42");
        assert_eq!(result, "ok merged #42");
    }

    #[test]
    fn test_ok_confirmation_pr_comment() {
        let result = ok_confirmation("commented", "#42");
        assert_eq!(result, "ok commented #42");
    }

    #[test]
    fn test_ok_confirmation_pr_edit() {
        let result = ok_confirmation("edited", "#42");
        assert_eq!(result, "ok edited #42");
    }
}
