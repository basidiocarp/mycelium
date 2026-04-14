//! Stash handler for git command proxy.
use crate::tracking;
use crate::vcs::git_filters::compact_diff;
use anyhow::{Context, Result};

pub(super) fn run_stash(
    subcommand: Option<&str>,
    args: &[String],
    verbose: u8,
    global_args: &[String],
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git stash {:?}", subcommand);
    }

    match subcommand {
        Some("list") => {
            // Use --format to get structured output directly instead of
            // regex-parsing the human-readable "stash@{N}: WIP on branch: hash msg" format.
            let output = super::git_cmd(global_args)
                .args(["stash", "list", "--format=%gd: %s"])
                .output()
                .context("Failed to run git stash list")?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let raw = stdout.to_string();

            if stdout.trim().is_empty() {
                let msg = "No stashes";
                println!("{}", msg);
                timer.track("git stash list", "mycelium git stash list", &raw, msg);
                return Ok(());
            }

            // Output is already compact: "stash@{0}: commit message"
            let filtered = stdout.trim().to_string();
            println!("{}", filtered);
            timer.track("git stash list", "mycelium git stash list", &raw, &filtered);
        }
        Some("show") => {
            let mut cmd = super::git_cmd(global_args);
            cmd.args(["stash", "show", "-p"]);
            for arg in args {
                cmd.arg(arg);
            }
            let output = cmd.output().context("Failed to run git stash show")?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let raw = stdout.to_string();

            let filtered = if stdout.trim().is_empty() {
                let msg = "Empty stash";
                println!("{}", msg);
                msg.to_string()
            } else {
                let compacted = compact_diff(&stdout, 100);
                println!("{}", compacted);
                compacted
            };

            timer.track("git stash show", "mycelium git stash show", &raw, &filtered);
        }
        Some("pop") | Some("apply") | Some("drop") | Some("push") => {
            let sub = subcommand.unwrap();
            let mut cmd = super::git_cmd(global_args);
            cmd.args(["stash", sub]);
            for arg in args {
                cmd.arg(arg);
            }
            let output = cmd.output().context("Failed to run git stash")?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);

            let msg = if output.status.success() {
                let msg = format!("ok stash {}", sub);
                println!("{}", msg);
                msg
            } else {
                eprintln!("FAILED: git stash {}", sub);
                if !stderr.trim().is_empty() {
                    eprintln!("{}", stderr);
                }
                combined.clone()
            };

            timer.track(
                &format!("git stash {}", sub),
                &format!("mycelium git stash {}", sub),
                &combined,
                &msg,
            );

            if !output.status.success() {
                std::process::exit(output.status.code().unwrap_or(1));
            }
        }
        _ => {
            // Default: git stash (push)
            let mut cmd = super::git_cmd(global_args);
            cmd.arg("stash");
            for arg in args {
                cmd.arg(arg);
            }
            let output = cmd.output().context("Failed to run git stash")?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);

            let msg = if output.status.success() {
                if stdout.contains("No local changes") {
                    let msg = "ok (nothing to stash)";
                    println!("{}", msg);
                    msg.to_string()
                } else {
                    let msg = "ok stashed";
                    println!("{}", msg);
                    msg.to_string()
                }
            } else {
                eprintln!("FAILED: git stash");
                if !stderr.trim().is_empty() {
                    eprintln!("{}", stderr);
                }
                combined.clone()
            };

            timer.track("git stash", "mycelium git stash", &combined, &msg);

            if !output.status.success() {
                std::process::exit(output.status.code().unwrap_or(1));
            }
        }
    }

    Ok(())
}
