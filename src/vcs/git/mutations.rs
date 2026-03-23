//! Commit, push, pull, and fetch handlers for git command proxy.
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

pub(super) fn build_commit_command(args: &[String], global_args: &[String]) -> Command {
    let mut cmd = super::git_cmd(global_args);
    cmd.arg("commit");
    for arg in args {
        cmd.arg(arg);
    }
    cmd
}

pub(super) fn run_commit(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let original_cmd = format!("git commit {}", args.join(" "));

    if verbose > 0 {
        eprintln!("{}", original_cmd);
    }

    let output = build_commit_command(args, global_args)
        .output()
        .context("Failed to run git commit")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_output = format!("{}\n{}", stdout, stderr);

    if output.status.success() {
        // Extract commit hash from output like "[main abc1234] message"
        let compact = if let Some(line) = stdout.lines().next() {
            if let Some(hash_start) = line.find(' ') {
                let hash = line[1..hash_start].split(' ').next_back().unwrap_or("");
                if !hash.is_empty() && hash.len() >= 7 {
                    format!("ok ✓ {}", &hash[..7.min(hash.len())])
                } else {
                    "ok ✓".to_string()
                }
            } else {
                "ok ✓".to_string()
            }
        } else {
            "ok ✓".to_string()
        };

        println!("{}", compact);

        timer.track(&original_cmd, "mycelium git commit", &raw_output, &compact);
    } else if stderr.contains("nothing to commit") || stdout.contains("nothing to commit") {
        println!("ok (nothing to commit)");
        timer.track(
            &original_cmd,
            "mycelium git commit",
            &raw_output,
            "ok (nothing to commit)",
        );
    } else {
        eprintln!("FAILED: git commit");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout);
        }
    }

    Ok(())
}

pub(super) fn run_push(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git push");
    }

    let mut cmd = super::git_cmd(global_args);
    cmd.arg("push");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git push")?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = format!("{}{}", stdout, stderr);

    if output.status.success() {
        let compact = if stderr.contains("Everything up-to-date") {
            "ok (up-to-date)".to_string()
        } else {
            let mut result = String::new();
            for line in stderr.lines() {
                if line.contains("->") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        result = format!("ok ✓ {}", parts[parts.len() - 1]);
                        break;
                    }
                }
            }
            if !result.is_empty() {
                result
            } else {
                "ok ✓".to_string()
            }
        };

        println!("{}", compact);

        timer.track(
            &format!("git push {}", args.join(" ")),
            &format!("mycelium git push {}", args.join(" ")),
            &raw,
            &compact,
        );
    } else {
        eprintln!("FAILED: git push");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout);
        }
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

pub(super) fn run_pull(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git pull");
    }

    let mut cmd = super::git_cmd(global_args);
    cmd.arg("pull");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git pull")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw_output = format!("{}\n{}", stdout, stderr);

    if output.status.success() {
        let compact =
            if stdout.contains("Already up to date") || stdout.contains("Already up-to-date") {
                "ok (up-to-date)".to_string()
            } else {
                // Count files changed
                let mut files = 0;
                let mut insertions = 0;
                let mut deletions = 0;

                for line in stdout.lines() {
                    if line.contains("file") && line.contains("changed") {
                        // Parse "3 files changed, 10 insertions(+), 2 deletions(-)"
                        for part in line.split(',') {
                            let part = part.trim();
                            if part.contains("file") {
                                files = part
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse().ok())
                                    .unwrap_or(0);
                            } else if part.contains("insertion") {
                                insertions = part
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse().ok())
                                    .unwrap_or(0);
                            } else if part.contains("deletion") {
                                deletions = part
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse().ok())
                                    .unwrap_or(0);
                            }
                        }
                    }
                }

                if files > 0 {
                    format!("ok ✓ {} files +{} -{}", files, insertions, deletions)
                } else {
                    "ok ✓".to_string()
                }
            };

        println!("{}", compact);

        timer.track(
            &format!("git pull {}", args.join(" ")),
            &format!("mycelium git pull {}", args.join(" ")),
            &raw_output,
            &compact,
        );
    } else {
        eprintln!("FAILED: git pull");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout);
        }
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

pub(super) fn run_fetch(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git fetch");
    }

    let mut cmd = super::git_cmd(global_args);
    cmd.arg("fetch");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git fetch")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}{}", stdout, stderr);

    if !output.status.success() {
        eprintln!("FAILED: git fetch");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        std::process::exit(output.status.code().unwrap_or(1));
    }

    // Count new refs from stderr (git fetch outputs to stderr)
    let new_refs: usize = stderr
        .lines()
        .filter(|l| l.contains("->") || l.contains("[new"))
        .count();

    let msg = if new_refs > 0 {
        format!("ok fetched ({} new refs)", new_refs)
    } else {
        "ok fetched".to_string()
    };

    println!("{}", msg);
    timer.track("git fetch", "mycelium git fetch", &raw, &msg);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_single_message() {
        let args = vec!["-m".to_string(), "fix: typo".to_string()];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "-m", "fix: typo"]);
    }

    #[test]
    fn test_commit_multiple_messages() {
        let args = vec![
            "-m".to_string(),
            "feat: add multi-paragraph support".to_string(),
            "-m".to_string(),
            "This allows git commit -m \"title\" -m \"body\".".to_string(),
        ];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            cmd_args,
            vec![
                "commit",
                "-m",
                "feat: add multi-paragraph support",
                "-m",
                "This allows git commit -m \"title\" -m \"body\"."
            ]
        );
    }

    // #327: git commit -am "msg" must pass -am through to git
    #[test]
    fn test_commit_am_flag() {
        let args = vec!["-am".to_string(), "quick fix".to_string()];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "-am", "quick fix"]);
    }

    #[test]
    fn test_commit_amend() {
        let args = vec![
            "--amend".to_string(),
            "-m".to_string(),
            "new msg".to_string(),
        ];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "--amend", "-m", "new msg"]);
    }

    #[test]
    fn test_commit_signed_flag() {
        let args = vec!["-S".to_string(), "-m".to_string(), "signed".to_string()];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "-S", "-m", "signed"]);
    }

    #[test]
    fn test_commit_signed_amend_flag() {
        let args = vec![
            "--amend".to_string(),
            "-S".to_string(),
            "-m".to_string(),
            "re-signed".to_string(),
        ];
        let cmd = build_commit_command(&args, &[]);
        let cmd_args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(cmd_args, vec!["commit", "--amend", "-S", "-m", "re-signed"]);
    }
}
