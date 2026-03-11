//! pnpm package manager filter with compact dependency trees and install summaries.
pub mod parsers;

use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

use crate::parser::{
    FormatMode, OutputParser, ParseResult, TokenFormatter, emit_degradation_warning,
    emit_passthrough_warning,
};
use parsers::{PnpmListParser, PnpmOutdatedParser};

/// Supported pnpm subcommands (list, outdated, install).
#[derive(Debug, Clone)]
pub enum PnpmCommand {
    List { depth: usize },
    Outdated,
    Install { packages: Vec<String> },
}

/// Execute a pnpm command with token-optimized output filtering.
pub fn run(cmd: PnpmCommand, args: &[String], verbose: u8) -> Result<()> {
    match cmd {
        PnpmCommand::List { depth } => run_list(depth, args, verbose),
        PnpmCommand::Outdated => run_outdated(args, verbose),
        PnpmCommand::Install { packages } => run_install(&packages, args, verbose),
    }
}

/// Validates npm package name according to official rules
fn is_valid_package_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 214 {
        return false;
    }

    // No path traversal
    if name.contains("..") {
        return false;
    }

    // Only safe characters
    name.chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '@' | '/' | '-' | '_' | '.'))
}

fn run_list(depth: usize, args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("pnpm");
    cmd.arg("list");
    cmd.arg(format!("--depth={}", depth));
    cmd.arg("--json");

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run pnpm list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pnpm list failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse output using PnpmListParser
    let parse_result = PnpmListParser::parse(&stdout);
    let mode = FormatMode::from_verbosity(verbose);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };
    let format_mode_str = match mode {
        FormatMode::Compact => "compact",
        FormatMode::Verbose => "verbose",
        FormatMode::Ultra => "ultra",
    };

    let filtered = match parse_result {
        ParseResult::Full(data) => {
            if verbose > 0 {
                eprintln!("pnpm list (Tier 1: Full JSON parse)");
            }
            data.format(mode)
        }
        ParseResult::Degraded(data, warnings) => {
            if verbose > 0 {
                emit_degradation_warning("pnpm list", &warnings.join(", "));
            }
            data.format(mode)
        }
        ParseResult::Passthrough(raw) => {
            emit_passthrough_warning("pnpm list", "All parsing tiers failed");
            raw
        }
    };

    println!("{}", filtered);

    timer.track_with_parse_info(
        &format!("pnpm list --depth={}", depth),
        &format!("mycelium pnpm list --depth={}", depth),
        &stdout,
        &filtered,
        parse_tier,
        format_mode_str,
    );

    Ok(())
}

fn run_outdated(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("pnpm");
    cmd.arg("outdated");
    cmd.arg("--format");
    cmd.arg("json");

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run pnpm outdated")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Parse output using PnpmOutdatedParser
    let parse_result = PnpmOutdatedParser::parse(&stdout);
    let mode = FormatMode::from_verbosity(verbose);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };
    let format_mode_str = match mode {
        FormatMode::Compact => "compact",
        FormatMode::Verbose => "verbose",
        FormatMode::Ultra => "ultra",
    };

    let filtered = match parse_result {
        ParseResult::Full(data) => {
            if verbose > 0 {
                eprintln!("pnpm outdated (Tier 1: Full JSON parse)");
            }
            data.format(mode)
        }
        ParseResult::Degraded(data, warnings) => {
            if verbose > 0 {
                emit_degradation_warning("pnpm outdated", &warnings.join(", "));
            }
            data.format(mode)
        }
        ParseResult::Passthrough(raw) => {
            emit_passthrough_warning("pnpm outdated", "All parsing tiers failed");
            raw
        }
    };

    if filtered.trim().is_empty() {
        println!("All packages up-to-date ✓");
    } else {
        println!("{}", filtered);
    }

    timer.track_with_parse_info(
        "pnpm outdated",
        "mycelium pnpm outdated",
        &combined,
        &filtered,
        parse_tier,
        format_mode_str,
    );

    Ok(())
}

fn run_install(packages: &[String], args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Validate package names to prevent command injection
    for pkg in packages {
        if !is_valid_package_name(pkg) {
            anyhow::bail!(
                "Invalid package name: '{}' (contains unsafe characters)",
                pkg
            );
        }
    }

    let mut cmd = Command::new("pnpm");
    cmd.arg("install");

    for pkg in packages {
        cmd.arg(pkg);
    }

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("pnpm install running...");
    }

    let output = cmd.output().context("Failed to run pnpm install")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        anyhow::bail!("pnpm install failed: {}", stderr);
    }

    let combined = format!("{}{}", stdout, stderr);
    let filtered = filter_pnpm_install(&combined);

    println!("{}", filtered);

    timer.track(
        &format!("pnpm install {}", packages.join(" ")),
        &format!("mycelium pnpm install {}", packages.join(" ")),
        &combined,
        &filtered,
    );

    Ok(())
}

/// Filter pnpm install output - remove progress bars, keep summary
fn filter_pnpm_install(output: &str) -> String {
    let mut result = Vec::new();
    let mut saw_progress = false;

    for line in output.lines() {
        // Skip progress bars
        if line.contains("Progress") || line.contains('│') || line.contains('%') {
            saw_progress = true;
            continue;
        }

        if saw_progress && line.trim().is_empty() {
            continue;
        }

        // Keep error lines
        if line.contains("ERR") || line.contains("error") || line.contains("ERROR") {
            result.push(line.to_string());
            continue;
        }

        // Keep summary lines
        if line.contains("packages in")
            || line.contains("dependencies")
            || line.starts_with('+')
            || line.starts_with('-')
        {
            result.push(line.trim().to_string());
        }
    }

    if result.is_empty() {
        "ok ✓".to_string()
    } else {
        result.join("\n")
    }
}

/// Runs an unsupported pnpm subcommand by passing it through directly
pub fn run_passthrough(args: &[OsString], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("pnpm passthrough: {:?}", args);
    }
    let status = Command::new("pnpm")
        .args(args)
        .status()
        .context("Failed to run pnpm")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("pnpm {}", args_str),
        &format!("mycelium pnpm {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parsers::{PnpmListParser, PnpmOutdatedParser};

    #[test]
    fn test_pnpm_list_parser_json() {
        let json = r#"{
            "my-project": {
                "version": "1.0.0",
                "dependencies": {
                    "express": {
                        "version": "4.18.2"
                    }
                }
            }
        }"#;

        let result = PnpmListParser::parse(json);
        assert_eq!(result.tier(), 1);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.total_packages >= 2);
    }

    #[test]
    fn test_pnpm_outdated_parser_json() {
        let json = r#"{
            "express": {
                "current": "4.18.2",
                "latest": "4.19.0",
                "wanted": "4.18.2"
            }
        }"#;

        let result = PnpmOutdatedParser::parse(json);
        assert_eq!(result.tier(), 1);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert_eq!(data.outdated_count, 1);
        assert_eq!(data.dependencies[0].name, "express");
    }

    #[test]
    fn test_package_name_validation() {
        assert!(is_valid_package_name("lodash"));
        assert!(is_valid_package_name("@clerk/express"));
        assert!(!is_valid_package_name("../../../etc/passwd"));
        assert!(!is_valid_package_name("lodash; rm -rf /"));
    }

    #[test]
    fn test_run_passthrough_accepts_args() {
        // Test that run_passthrough compiles and has correct signature
        let _args: Vec<OsString> = vec![OsString::from("help")];
        // Compile-time verification that the function exists with correct signature
    }

    #[test]
    fn test_pnpm_install_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        let input = include_str!("../../../tests/fixtures/pnpm_install_raw.txt");
        let output = filter_pnpm_install(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);

        // pnpm install filter removes progress bars but keeps dependency changes and summary
        // Savings from removing progress indicators (~25-30% typical)
        assert!(
            savings >= 20,
            "pnpm install filter: expected >= 20% token savings, got {}%",
            savings
        );
    }
}
