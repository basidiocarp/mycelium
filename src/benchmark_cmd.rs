//! `mycelium benchmark` — measure token savings across available commands.
//!
//! Replaces `scripts/benchmark.sh` with a built-in command that works anywhere
//! without cloning the repo. Runs real commands, compares raw vs filtered output,
//! and reports token savings.

use anyhow::Result;
use colored::Colorize;
use std::process::Command;

use crate::tracking;

/// A single benchmark result.
struct BenchResult {
    #[allow(dead_code)]
    name: String,
    raw_tokens: usize,
    filtered_tokens: usize,
    status: BenchStatus,
}

enum BenchStatus {
    Good,
    Skip, // filtered >= raw
    Fail, // empty output
}

pub fn run(ci: bool) -> Result<()> {
    println!("{}", "Mycelium Benchmark".bold());
    println!();

    let mycelium = current_binary()?;
    let mut results: Vec<BenchResult> = Vec::new();

    // ── File commands ────────────────────────────────────────────────────────
    section("Files");
    bench(&mycelium, &mut results, "ls", "ls -la", &["ls"]);
    bench(
        &mycelium,
        &mut results,
        "read",
        "cat src/main.rs",
        &["read", "src/main.rs"],
    );

    // ── Git commands ─────────────────────────────────────────────────────────
    if is_git_repo() {
        section("Git");
        bench(
            &mycelium,
            &mut results,
            "git status",
            "git status",
            &["git", "status"],
        );
        bench(
            &mycelium,
            &mut results,
            "git log",
            "git log -10",
            &["git", "log", "-n", "10"],
        );
        bench(
            &mycelium,
            &mut results,
            "git diff",
            "git diff HEAD~1",
            &["git", "diff"],
        );
    }

    // ── Grep ─────────────────────────────────────────────────────────────────
    bench(
        &mycelium,
        &mut results,
        "grep",
        "grep -rn fn src/",
        &["grep", "fn", "src/"],
    );

    // ── Cargo (if Cargo.toml exists) ─────────────────────────────────────────
    if std::path::Path::new("Cargo.toml").exists() {
        section("Cargo");
        bench(
            &mycelium,
            &mut results,
            "cargo build",
            "cargo build 2>&1",
            &["cargo", "build"],
        );
        bench(
            &mycelium,
            &mut results,
            "cargo test",
            "cargo test 2>&1",
            &["cargo", "test"],
        );
        bench(
            &mycelium,
            &mut results,
            "cargo clippy",
            "cargo clippy 2>&1",
            &["cargo", "clippy"],
        );
    }

    // ── JSON ─────────────────────────────────────────────────────────────────
    if std::path::Path::new("package.json").exists() {
        section("Data");
        bench(
            &mycelium,
            &mut results,
            "json",
            "cat package.json",
            &["json", "package.json"],
        );
    } else if std::path::Path::new("Cargo.toml").exists() {
        section("Data");
        bench(&mycelium, &mut results, "deps", "cat Cargo.toml", &["deps"]);
    }

    // ── Environment ──────────────────────────────────────────────────────────
    bench(&mycelium, &mut results, "env", "env", &["env"]);

    // ── Docker (if available) ────────────────────────────────────────────────
    if command_exists("docker") {
        section("Docker");
        bench(
            &mycelium,
            &mut results,
            "docker ps",
            "docker ps",
            &["docker", "ps"],
        );
        bench(
            &mycelium,
            &mut results,
            "docker images",
            "docker images",
            &["docker", "images"],
        );
    }

    // ── GitHub CLI (if available) ────────────────────────────────────────────
    if command_exists("gh") && is_git_repo() {
        section("GitHub");
        bench(
            &mycelium,
            &mut results,
            "gh pr list",
            "gh pr list",
            &["gh", "pr", "list"],
        );
    }

    // ── Summary ──────────────────────────────────────────────────────────────
    println!();
    println!("{}", "═".repeat(72));
    print_summary(&results, ci)
}

fn section(name: &str) {
    println!();
    println!("── {} ──", name.bold());
}

fn bench(
    mycelium: &str,
    results: &mut Vec<BenchResult>,
    name: &str,
    raw_cmd: &str,
    mycelium_args: &[&str],
) {
    let raw_output = run_shell(raw_cmd);
    let filtered_output = run_mycelium(mycelium, mycelium_args);

    let raw_tokens = estimate_tokens(&raw_output);
    let filtered_tokens = estimate_tokens(&filtered_output);

    let (icon, status) = if filtered_output.is_empty() && !raw_output.is_empty() {
        ("✗".red().to_string(), BenchStatus::Fail)
    } else if filtered_tokens >= raw_tokens && raw_tokens > 0 {
        ("!".yellow().to_string(), BenchStatus::Skip)
    } else {
        ("✓".green().to_string(), BenchStatus::Good)
    };

    let savings = if raw_tokens > 0 {
        format!(
            "-{}%",
            (raw_tokens.saturating_sub(filtered_tokens)) * 100 / raw_tokens
        )
    } else {
        "--".to_string()
    };

    println!(
        "  {} {:<20} {:>6} → {:>6}  ({})",
        icon,
        name,
        raw_tokens,
        filtered_tokens,
        savings.dimmed()
    );

    results.push(BenchResult {
        name: name.to_string(),
        raw_tokens,
        filtered_tokens,
        status,
    });
}

fn print_summary(results: &[BenchResult], ci: bool) -> Result<()> {
    let total = results.len();
    let good = results
        .iter()
        .filter(|r| matches!(r.status, BenchStatus::Good))
        .count();
    let skip = results
        .iter()
        .filter(|r| matches!(r.status, BenchStatus::Skip))
        .count();
    let fail = results
        .iter()
        .filter(|r| matches!(r.status, BenchStatus::Fail))
        .count();

    let total_raw: usize = results.iter().map(|r| r.raw_tokens).sum();
    let total_filtered: usize = results
        .iter()
        .map(|r| match r.status {
            BenchStatus::Good => r.filtered_tokens,
            _ => r.raw_tokens, // no savings for skip/fail
        })
        .sum();

    let savings_pct = if total_raw > 0 {
        (total_raw.saturating_sub(total_filtered)) * 100 / total_raw
    } else {
        0
    };

    println!();
    println!(
        "  {} {} good  {} {} skip  {} {} fail    {}/{}",
        "✓".green(),
        good,
        "!".yellow(),
        skip,
        "✗".red(),
        fail,
        good,
        total
    );
    println!(
        "  Tokens: {} → {}  (-{}%)",
        total_raw, total_filtered, savings_pct
    );
    println!();

    // Track the benchmark run itself
    let raw_summary = format!("benchmark: {total} tests, {total_raw} raw tokens");
    let filtered_summary = format!(
        "benchmark: {good}/{total} good, {total_filtered} filtered tokens (-{savings_pct}%)"
    );
    let timer = tracking::TimedExecution::start();
    timer.track(
        "benchmark",
        "mycelium benchmark",
        &raw_summary,
        &filtered_summary,
    );

    // CI mode: fail if less than 80% of tests show savings
    if ci {
        let good_pct = if total > 0 { good * 100 / total } else { 0 };
        if good_pct < 80 {
            anyhow::bail!(
                "Benchmark failed: {}% of tests show savings (minimum 80%)",
                good_pct
            );
        }
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn current_binary() -> Result<String> {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| anyhow::anyhow!("Cannot resolve current binary: {e}"))
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn run_shell(cmd: &str) -> String {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn run_mycelium(binary: &str, args: &[&str]) -> String {
    Command::new(binary)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_git_repo() -> bool {
    std::path::Path::new(".git").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
    }

    #[test]
    fn test_current_binary_resolves() {
        assert!(current_binary().is_ok());
    }
}
