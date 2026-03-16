//! Diff and show handlers for git command proxy.
use crate::tracking;
use crate::vcs::git_filters::{compact_diff, is_blob_show_arg};
use anyhow::{Context, Result};

pub(super) fn run_diff(
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Check if user wants stat output
    let wants_stat = args
        .iter()
        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");

    // Check if user wants compact diff (default Mycelium behavior)
    let wants_compact = !args.iter().any(|arg| arg == "--no-compact");

    if wants_stat || !wants_compact {
        // User wants stat or explicitly no compacting - pass through directly
        let mut cmd = super::git_cmd(global_args);
        cmd.arg("diff");
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().context("Failed to run git diff")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}", stderr);
            std::process::exit(output.status.code().unwrap_or(1));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("{}", stdout.trim());

        timer.track(
            &format!("git diff {}", args.join(" ")),
            &format!("mycelium git diff {} (passthrough)", args.join(" ")),
            &stdout,
            &stdout,
        );

        return Ok(());
    }

    // Default Mycelium behavior: stat first, then compacted diff
    let mut cmd = super::git_cmd(global_args);
    cmd.arg("diff").arg("--stat");

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run git diff")?;
    let stat_stdout = String::from_utf8_lossy(&output.stdout);

    if verbose > 0 {
        eprintln!("Git diff summary:");
    }

    // Print stat summary first
    println!("{}", stat_stdout.trim());

    // Now get actual diff but compact it
    let mut diff_cmd = super::git_cmd(global_args);
    diff_cmd.arg("diff");
    for arg in args {
        diff_cmd.arg(arg);
    }

    let diff_output = diff_cmd.output().context("Failed to run git diff")?;
    let diff_stdout = String::from_utf8_lossy(&diff_output.stdout);

    let mut final_output = stat_stdout.to_string();
    if !diff_stdout.is_empty() {
        println!("\n--- Changes ---");
        let compacted = crate::hyphae::route_or_filter(
            &format!("git diff {}", args.join(" ")),
            &diff_stdout,
            |d| compact_diff(d, max_lines.unwrap_or(500)),
        );
        println!("{}", compacted);
        final_output.push_str("\n--- Changes ---\n");
        final_output.push_str(&compacted);
    }

    timer.track(
        &format!("git diff {}", args.join(" ")),
        &format!("mycelium git diff {}", args.join(" ")),
        &format!("{}\n{}", stat_stdout, diff_stdout),
        &final_output,
    );

    Ok(())
}

pub(super) fn run_show(
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // If user wants --stat or --format only, pass through
    let wants_stat_only = args
        .iter()
        .any(|arg| arg == "--stat" || arg == "--numstat" || arg == "--shortstat");

    let wants_format = args
        .iter()
        .any(|arg| arg.starts_with("--pretty") || arg.starts_with("--format"));

    // `git show rev:path` prints a blob, not a commit diff. In this mode we should
    // pass through directly to avoid duplicated output from compact-show steps.
    let wants_blob_show = args.iter().any(|arg| is_blob_show_arg(arg));

    if wants_stat_only || wants_format || wants_blob_show {
        let mut cmd = super::git_cmd(global_args);
        cmd.arg("show");
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().context("Failed to run git show")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("{}", stderr);
            std::process::exit(output.status.code().unwrap_or(1));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if wants_blob_show {
            print!("{}", stdout);
        } else {
            println!("{}", stdout.trim());
        }

        timer.track(
            &format!("git show {}", args.join(" ")),
            &format!("mycelium git show {} (passthrough)", args.join(" ")),
            &stdout,
            &stdout,
        );

        return Ok(());
    }

    // Get raw output for tracking
    let mut raw_cmd = super::git_cmd(global_args);
    raw_cmd.arg("show");
    for arg in args {
        raw_cmd.arg(arg);
    }
    let raw_output = raw_cmd
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    // Step 1: one-line commit summary
    let mut summary_cmd = super::git_cmd(global_args);
    summary_cmd.args([
        "show",
        "--no-patch",
        "--pretty=format:%h %s (%ar) <%an>%n%b",
    ]);
    for arg in args {
        summary_cmd.arg(arg);
    }
    let summary_output = summary_cmd.output().context("Failed to run git show")?;
    if !summary_output.status.success() {
        let stderr = String::from_utf8_lossy(&summary_output.stderr);
        eprintln!("{}", stderr);
        std::process::exit(summary_output.status.code().unwrap_or(1));
    }
    let summary = String::from_utf8_lossy(&summary_output.stdout).to_string();
    let summary = summary.trim_end().to_string();
    println!("{}", summary.trim());

    // Step 2: --stat summary
    let mut stat_cmd = super::git_cmd(global_args);
    stat_cmd.args(["show", "--stat", "--pretty=format:"]);
    for arg in args {
        stat_cmd.arg(arg);
    }
    let stat_output = stat_cmd.output().context("Failed to run git show --stat")?;
    let stat_stdout = String::from_utf8_lossy(&stat_output.stdout);
    let stat_text = stat_stdout.trim();
    if !stat_text.is_empty() {
        println!("{}", stat_text);
    }

    // Step 3: compacted diff
    let mut diff_cmd = super::git_cmd(global_args);
    diff_cmd.args(["show", "--pretty=format:"]);
    for arg in args {
        diff_cmd.arg(arg);
    }
    let diff_output = diff_cmd.output().context("Failed to run git show (diff)")?;
    let diff_stdout = String::from_utf8_lossy(&diff_output.stdout);
    let diff_text = diff_stdout.trim();

    let mut final_output = summary.to_string();
    if !diff_text.is_empty() {
        if verbose > 0 {
            println!("\n--- Changes ---");
        }
        let compacted = crate::hyphae::route_or_filter(
            &format!("git show {}", args.join(" ")),
            diff_text,
            |d| compact_diff(d, max_lines.unwrap_or(500)),
        );
        println!("{}", compacted);
        final_output.push_str(&format!("\n{}", compacted));
    }

    timer.track(
        &format!("git show {}", args.join(" ")),
        &format!("mycelium git show {}", args.join(" ")),
        &raw_output,
        &final_output,
    );

    Ok(())
}
