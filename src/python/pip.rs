//! pip/uv package manager filter with JSON parsing and auto-detection of uv.
use crate::tracking;
use crate::utils::which_command;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::ffi::OsString;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
    #[serde(default)]
    latest_version: Option<String>,
}

/// Run pip (or uv) and filter output for list, outdated, and install subcommands.
#[allow(dead_code)]
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!(
            "mycelium pip: no subcommand specified\nSupported: list, outdated, install, uninstall, show"
        );
    }

    // Detect subcommand
    let subcommand = args.first().map(|s| s.as_str()).unwrap_or("");

    match subcommand {
        "list" => run_list(&args[1..], verbose),
        "outdated" => run_outdated(&args[1..], verbose),
        "install" => run_install(args, verbose),
        "uninstall" => run_uninstall(args, verbose),
        "show" => run_show(args, verbose),
        _ => anyhow::bail!(
            "mycelium pip: unsupported subcommand '{}'\nSupported: list, outdated, install, uninstall, show",
            subcommand
        ),
    }
}

/// List installed packages (pip list --format=json)
pub fn run_list(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Auto-detect uv vs pip
    let use_uv = which_command("uv").is_some();
    let base_cmd = if use_uv { "uv" } else { "pip" };

    if verbose > 0 && use_uv {
        eprintln!("Using uv (pip-compatible)");
    }

    let mut cmd = Command::new(base_cmd);

    if base_cmd == "uv" {
        cmd.arg("pip");
    }

    cmd.arg("list").arg("--format=json");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: {} pip list --format=json", base_cmd);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run {} pip list", base_cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let filtered = filter_pip_list(&stdout);
    println!("{}", filtered);

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    timer.track(
        &format!("{} list {}", base_cmd, args.join(" ")),
        &format!("mycelium {} list {}", base_cmd, args.join(" ")),
        &raw,
        &filtered,
    );

    Ok(())
}

/// Show outdated packages (pip list --outdated --format=json)
pub fn run_outdated(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Auto-detect uv vs pip
    let use_uv = which_command("uv").is_some();
    let base_cmd = if use_uv { "uv" } else { "pip" };

    if verbose > 0 && use_uv {
        eprintln!("Using uv (pip-compatible)");
    }

    let mut cmd = Command::new(base_cmd);

    if base_cmd == "uv" {
        cmd.arg("pip");
    }

    cmd.arg("list").arg("--outdated").arg("--format=json");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: {} pip list --outdated --format=json", base_cmd);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run {} pip list --outdated", base_cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let filtered = filter_pip_outdated(&stdout);
    println!("{}", filtered);

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    timer.track(
        &format!("{} list --outdated {}", base_cmd, args.join(" ")),
        &format!("mycelium {} list --outdated {}", base_cmd, args.join(" ")),
        &raw,
        &filtered,
    );

    Ok(())
}

/// Install packages (passthrough)
pub fn run_install(args: &[String], verbose: u8) -> Result<()> {
    run_passthrough("install", args, verbose)
}

/// Uninstall packages (passthrough)
pub fn run_uninstall(args: &[String], verbose: u8) -> Result<()> {
    run_passthrough("uninstall", args, verbose)
}

/// Show package info (passthrough)
pub fn run_show(args: &[String], verbose: u8) -> Result<()> {
    run_passthrough("show", args, verbose)
}

/// Run other pip subcommand with error message about supported commands
pub fn run_other(args: &[OsString], _verbose: u8) -> Result<()> {
    let subcommand = args
        .first()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(empty)".to_string());
    anyhow::bail!(
        "mycelium pip: unsupported subcommand '{}'\nSupported: list, outdated, install, uninstall, show",
        subcommand
    );
}

fn run_passthrough(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    // Auto-detect uv vs pip
    let use_uv = which_command("uv").is_some();
    let base_cmd = if use_uv { "uv" } else { "pip" };

    if verbose > 0 && use_uv {
        eprintln!("Using uv (pip-compatible)");
    }

    let mut cmd = Command::new(base_cmd);

    if base_cmd == "uv" {
        cmd.arg("pip");
    }

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: {} pip {}", base_cmd, args.join(" "));
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run {} pip {}", base_cmd, args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    print!("{}", stdout);
    eprint!("{}", stderr);

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    timer.track(
        &format!("{} {} {}", base_cmd, subcommand, args.join(" ")),
        &format!("mycelium {} {} {}", base_cmd, subcommand, args.join(" ")),
        &raw,
        &raw,
    );

    Ok(())
}

/// Filter pip list JSON output
fn filter_pip_list(output: &str) -> String {
    let packages: Vec<Package> = match serde_json::from_str(output) {
        Ok(p) => p,
        Err(e) => {
            return format!("pip list (JSON parse failed: {})", e);
        }
    };

    if packages.is_empty() {
        return "pip list: No packages installed".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!("pip list: {} packages\n", packages.len()));
    result.push_str("═══════════════════════════════════════\n");

    // Group by first letter for easier scanning
    let mut by_letter: std::collections::HashMap<char, Vec<&Package>> =
        std::collections::HashMap::new();

    for pkg in &packages {
        let first_char = pkg.name.chars().next().unwrap_or('?').to_ascii_lowercase();
        by_letter.entry(first_char).or_default().push(pkg);
    }

    let mut letters: Vec<_> = by_letter.keys().collect();
    letters.sort();

    for letter in letters {
        let pkgs = &by_letter[letter];
        result.push_str(&format!("\n[{}]\n", letter.to_uppercase()));

        for pkg in pkgs.iter().take(10) {
            result.push_str(&format!("  {} ({})\n", pkg.name, pkg.version));
        }

        if pkgs.len() > 10 {
            result.push_str(&format!("  ... +{} more\n", pkgs.len() - 10));
        }
    }

    result.trim().to_string()
}

/// Filter pip outdated JSON output
fn filter_pip_outdated(output: &str) -> String {
    let packages: Vec<Package> = match serde_json::from_str(output) {
        Ok(p) => p,
        Err(e) => {
            return format!("pip outdated (JSON parse failed: {})", e);
        }
    };

    if packages.is_empty() {
        return "✓ pip outdated: All packages up to date".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!("pip outdated: {} packages\n", packages.len()));
    result.push_str("═══════════════════════════════════════\n");

    for (i, pkg) in packages.iter().take(20).enumerate() {
        let latest = pkg.latest_version.as_deref().unwrap_or("unknown");
        result.push_str(&format!(
            "{}. {} ({} → {})\n",
            i + 1,
            pkg.name,
            pkg.version,
            latest
        ));
    }

    if packages.len() > 20 {
        result.push_str(&format!("\n... +{} more packages\n", packages.len() - 20));
    }

    result.push_str("\nhint: Run `pip install --upgrade <package>` to update\n");

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_pip_list() {
        let output = r#"[
  {"name": "requests", "version": "2.31.0"},
  {"name": "pytest", "version": "7.4.0"},
  {"name": "rich", "version": "13.0.0"}
]"#;

        let result = filter_pip_list(output);
        assert!(result.contains("3 packages"));
        assert!(result.contains("requests"));
        assert!(result.contains("2.31.0"));
        assert!(result.contains("pytest"));
    }

    #[test]
    fn test_filter_pip_list_empty() {
        let output = "[]";
        let result = filter_pip_list(output);
        assert!(result.contains("No packages installed"));
    }

    #[test]
    fn test_filter_pip_outdated_none() {
        let output = "[]";
        let result = filter_pip_outdated(output);
        assert!(result.contains("✓"));
        assert!(result.contains("All packages up to date"));
    }

    #[test]
    fn test_filter_pip_outdated_some() {
        let output = r#"[
  {"name": "requests", "version": "2.31.0", "latest_version": "2.32.0"},
  {"name": "pytest", "version": "7.4.0", "latest_version": "8.0.0"}
]"#;

        let result = filter_pip_outdated(output);
        assert!(result.contains("2 packages"));
        assert!(result.contains("requests"));
        assert!(result.contains("2.31.0 → 2.32.0"));
        assert!(result.contains("pytest"));
        assert!(result.contains("7.4.0 → 8.0.0"));
    }
}
