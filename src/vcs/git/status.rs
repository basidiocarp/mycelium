//! Status, add, branch, and worktree handlers for git command proxy.
use crate::tracking;
use crate::vcs::git_filters::{
    filter_branch_output, filter_status_with_args, filter_worktree_list, format_status_output,
};
use anyhow::{Context, Result};

pub(super) fn run_status(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // If user provided flags, apply minimal filtering
    if !args.is_empty() {
        let output = super::git_cmd(global_args)
            .arg("status")
            .args(args)
            .output()
            .context("Failed to run git status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if verbose > 0 || !stderr.is_empty() {
            eprint!("{}", stderr);
        }

        // Apply minimal filtering: strip ANSI, remove hints, empty lines
        let filtered = filter_status_with_args(&stdout);
        print!("{}", filtered);

        timer.track(
            &format!("git status {}", args.join(" ")),
            &format!("mycelium git status {}", args.join(" ")),
            &stdout,
            &filtered,
        );

        return Ok(());
    }

    // Default Mycelium compact mode (no args provided)
    // Get raw git status for tracking
    let raw_output = super::git_cmd(global_args)
        .args(["status"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let output = super::git_cmd(global_args)
        .args(["status", "--porcelain", "-b"])
        .output()
        .context("Failed to run git status")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let formatted = if !stderr.is_empty() && stderr.contains("not a git repository") {
        "Not a git repository".to_string()
    } else {
        format_status_output(&stdout)
    };

    println!("{}", formatted);

    // Track for statistics
    timer.track("git status", "mycelium git status", &raw_output, &formatted);

    Ok(())
}

pub(super) fn run_add(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = super::git_cmd(global_args);
    cmd.arg("add");

    // Pass all arguments directly to git (flags like -A, -p, --all, etc.)
    if args.is_empty() {
        cmd.arg(".");
    } else {
        for arg in args {
            cmd.arg(arg);
        }
    }

    let output = cmd.output().context("Failed to run git add")?;

    if verbose > 0 {
        eprintln!("git add executed");
    }

    let raw_output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if output.status.success() {
        // Count what was added
        let status_output = super::git_cmd(global_args)
            .args(["diff", "--cached", "--stat", "--shortstat"])
            .output()
            .context("Failed to check staged files")?;

        let stat = String::from_utf8_lossy(&status_output.stdout);
        let compact = if stat.trim().is_empty() {
            "ok (nothing to add)".to_string()
        } else {
            // Parse "1 file changed, 5 insertions(+)" format
            let short = stat.lines().last().unwrap_or("").trim();
            if short.is_empty() {
                "ok ✓".to_string()
            } else {
                format!("ok ✓ {}", short)
            }
        };

        println!("{}", compact);

        timer.track(
            &format!("git add {}", args.join(" ")),
            &format!("mycelium git add {}", args.join(" ")),
            &raw_output,
            &compact,
        );
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        eprintln!("FAILED: git add");
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr);
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout);
        }
        // Propagate git's exit code
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

pub(super) fn run_branch(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git branch");
    }

    // Detect write operations: delete, rename, copy
    let has_action_flag = args
        .iter()
        .any(|a| a == "-d" || a == "-D" || a == "-m" || a == "-M" || a == "-c" || a == "-C");

    // Detect list-mode flags
    let has_list_flag = args.iter().any(|a| {
        a == "-a"
            || a == "--all"
            || a == "-r"
            || a == "--remotes"
            || a == "--list"
            || a == "--merged"
            || a == "--no-merged"
            || a == "--contains"
            || a == "--no-contains"
    });

    // Detect positional arguments (not flags) — indicates branch creation
    let has_positional_arg = args.iter().any(|a| !a.starts_with('-'));

    // Write operation: action flags, or positional args without list flags (= branch creation)
    if has_action_flag || (has_positional_arg && !has_list_flag) {
        let mut cmd = super::git_cmd(global_args);
        cmd.arg("branch");
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().context("Failed to run git branch")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let msg = if output.status.success() {
            "ok ✓"
        } else {
            &combined
        };

        timer.track(
            &format!("git branch {}", args.join(" ")),
            &format!("mycelium git branch {}", args.join(" ")),
            &combined,
            msg,
        );

        if output.status.success() {
            println!("ok ✓");
        } else {
            eprintln!("FAILED: git branch {}", args.join(" "));
            if !stderr.trim().is_empty() {
                eprintln!("{}", stderr);
            }
            if !stdout.trim().is_empty() {
                eprintln!("{}", stdout);
            }
            std::process::exit(output.status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // List mode: show compact branch list
    let mut cmd = super::git_cmd(global_args);
    cmd.arg("branch");
    if !has_list_flag {
        cmd.arg("-a");
    }
    cmd.arg("--no-color");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git branch")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = stdout.to_string();

    let filtered = filter_branch_output(&stdout);
    println!("{}", filtered);

    timer.track(
        &format!("git branch {}", args.join(" ")),
        &format!("mycelium git branch {}", args.join(" ")),
        &raw,
        &filtered,
    );

    Ok(())
}

pub(super) fn run_worktree(args: &[String], verbose: u8, global_args: &[String]) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git worktree list");
    }

    // If args contain "add", "remove", "prune" etc., pass through
    let has_action = args.iter().any(|a| {
        a == "add" || a == "remove" || a == "prune" || a == "lock" || a == "unlock" || a == "move"
    });

    if has_action {
        let mut cmd = super::git_cmd(global_args);
        cmd.arg("worktree");
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().context("Failed to run git worktree")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let msg = if output.status.success() {
            "ok ✓"
        } else {
            &combined
        };

        timer.track(
            &format!("git worktree {}", args.join(" ")),
            &format!("mycelium git worktree {}", args.join(" ")),
            &combined,
            msg,
        );

        if output.status.success() {
            println!("ok ✓");
        } else {
            eprintln!("FAILED: git worktree {}", args.join(" "));
            if !stderr.trim().is_empty() {
                eprintln!("{}", stderr);
            }
            std::process::exit(output.status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Default: list mode
    let output = super::git_cmd(global_args)
        .args(["worktree", "list"])
        .output()
        .context("Failed to run git worktree list")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = stdout.to_string();

    let filtered = filter_worktree_list(&stdout);
    println!("{}", filtered);
    timer.track(
        "git worktree list",
        "mycelium git worktree",
        &raw,
        &filtered,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Regression test: `git branch <name>` must create, not list.
    /// Before fix, positional args fell into list mode which added `-a`,
    /// turning creation into a pattern-filtered listing (silent no-op).
    #[test]
    #[ignore] // Integration test: requires git repo
    fn test_branch_creation_not_swallowed() {
        let branch = "test-mycelium-create-branch-regression";
        // Create branch via run_branch
        run_branch(&[branch.to_string()], 0, &[]).expect("run_branch should succeed");
        // Verify it exists
        let output = Command::new("git")
            .args(["branch", "--list", branch])
            .output()
            .expect("git branch --list should work");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(branch),
            "Branch '{}' was not created. run_branch silently swallowed the creation.",
            branch
        );
        // Cleanup
        let _ = Command::new("git").args(["branch", "-d", branch]).output();
    }

    /// Regression test: `git branch <name> <commit>` must create from commit.
    #[test]
    #[ignore] // Integration test: requires git repo
    fn test_branch_creation_from_commit() {
        let branch = "test-mycelium-create-from-commit";
        run_branch(&[branch.to_string(), "HEAD".to_string()], 0, &[])
            .expect("run_branch with start-point should succeed");
        let output = Command::new("git")
            .args(["branch", "--list", branch])
            .output()
            .expect("git branch --list should work");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(branch),
            "Branch '{}' was not created from commit.",
            branch
        );
        let _ = Command::new("git").args(["branch", "-d", branch]).output();
    }
}
