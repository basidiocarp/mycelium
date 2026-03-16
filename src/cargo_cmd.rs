//! Cargo command proxy that executes cargo subcommands and applies token-saving filters.
use crate::cargo_filters::{
    filter_cargo_build, filter_cargo_clippy, filter_cargo_install, filter_cargo_nextest,
};
use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum CargoCommand {
    Build,
    Test,
    Clippy,
    Check,
    Install,
    Nextest,
}

pub fn run(cmd: CargoCommand, args: &[String], verbose: u8) -> Result<()> {
    match cmd {
        CargoCommand::Build => run_build(args, verbose),
        CargoCommand::Test => run_test(args, verbose),
        CargoCommand::Clippy => run_clippy(args, verbose),
        CargoCommand::Check => run_check(args, verbose),
        CargoCommand::Install => run_install(args, verbose),
        CargoCommand::Nextest => run_nextest(args, verbose),
    }
}

/// Reconstruct args with `--` separator preserved from the original command line.
/// Clap strips `--` from parsed args, but cargo subcommands need it to separate
/// their own flags from test runner flags (e.g. `cargo test -- --nocapture`).
fn restore_double_dash(args: &[String]) -> Vec<String> {
    let raw_args: Vec<String> = std::env::args().collect();
    restore_double_dash_with_raw(args, &raw_args)
}

/// Testable version that takes raw_args explicitly.
fn restore_double_dash_with_raw(args: &[String], raw_args: &[String]) -> Vec<String> {
    if args.is_empty() {
        return args.to_vec();
    }

    // Find `--` in the original command line
    let sep_pos = match raw_args.iter().position(|a| a == "--") {
        Some(pos) => pos,
        None => return args.to_vec(),
    };

    // Count how many of our parsed args appeared before `--` in the original.
    // Args before `--` are positional (e.g. test name), args after are flags.
    let args_before_sep = raw_args[..sep_pos]
        .iter()
        .filter(|a| args.contains(a))
        .count();

    let mut result = Vec::with_capacity(args.len() + 1);
    result.extend_from_slice(&args[..args_before_sep]);
    result.push("--".to_string());
    result.extend_from_slice(&args[args_before_sep..]);
    result
}

/// Generic cargo command runner with filtering
fn run_cargo_filtered<F>(subcommand: &str, args: &[String], verbose: u8, filter_fn: F) -> Result<()>
where
    F: Fn(&str) -> String,
{
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("cargo");
    cmd.arg(subcommand);

    let restored_args = restore_double_dash(args);
    for arg in &restored_args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: cargo {} {}", subcommand, restored_args.join(" "));
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run cargo {}", subcommand))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let exit_code = output
        .status
        .code()
        .unwrap_or(if output.status.success() { 0 } else { 1 });
    let filtered = crate::hyphae::route_or_filter(
        &format!("cargo {} {}", subcommand, restored_args.join(" ")),
        &raw,
        filter_fn,
    );

    if let Some(hint) = crate::tee::tee_and_hint(&raw, &format!("cargo_{}", subcommand), exit_code)
    {
        println!("{}\n{}", filtered, hint);
    } else {
        println!("{}", filtered);
    }

    timer.track(
        &format!("cargo {} {}", subcommand, restored_args.join(" ")),
        &format!("mycelium cargo {} {}", subcommand, restored_args.join(" ")),
        &raw,
        &filtered,
    );

    if !output.status.success() {
        std::process::exit(exit_code);
    }

    Ok(())
}

fn run_build(args: &[String], verbose: u8) -> Result<()> {
    run_cargo_filtered("build", args, verbose, filter_cargo_build)
}

fn run_test(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let show_passing = crate::config::Config::load()
        .ok()
        .and_then(|c| c.filters.cargo.as_ref().map(|cf| cf.test_show_passing))
        .unwrap_or(false);

    let restored = restore_double_dash(args);
    let mut all_args: Vec<&str> = vec!["test"];
    let restored_refs: Vec<&str> = restored.iter().map(String::as_str).collect();
    all_args.extend_from_slice(&restored_refs);

    if verbose > 0 {
        eprintln!("Running: cargo {}", all_args.join(" "));
    }

    let result = crate::streaming::execute_streaming("cargo", &all_args, |line| {
        if !show_passing && line.contains("... ok") {
            None
        } else if line.starts_with("test result:")
            || line.contains("FAILED")
            || line.contains("error")
            || line.is_empty()
            || !line.starts_with("test ")
        {
            Some(line.to_string())
        } else {
            None
        }
    })?;

    if let Some(hint) = crate::tee::tee_and_hint(&result.raw, "cargo_test", result.exit_code) {
        println!("{}", hint);
    }

    // Route through Hyphae for very large test output
    let output = crate::hyphae::route_or_filter(
        &format!("cargo test {}", restored.join(" ")),
        &result.raw,
        |_| result.filtered.clone(),
    );

    timer.track(
        &format!("cargo test {}", restored.join(" ")),
        &format!("mycelium cargo test {}", restored.join(" ")),
        &result.raw,
        &output,
    );

    if result.exit_code != 0 {
        std::process::exit(result.exit_code);
    }

    Ok(())
}

fn run_clippy(args: &[String], verbose: u8) -> Result<()> {
    run_cargo_filtered("clippy", args, verbose, filter_cargo_clippy)
}

fn run_check(args: &[String], verbose: u8) -> Result<()> {
    run_cargo_filtered("check", args, verbose, filter_cargo_build)
}

fn run_install(args: &[String], verbose: u8) -> Result<()> {
    run_cargo_filtered("install", args, verbose, filter_cargo_install)
}

fn run_nextest(args: &[String], verbose: u8) -> Result<()> {
    run_cargo_filtered("nextest", args, verbose, filter_cargo_nextest)
}

/// Runs an unsupported cargo subcommand by passing it through directly
pub fn run_passthrough(args: &[OsString], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("cargo passthrough: {:?}", args);
    }
    let status = Command::new("cargo")
        .args(args)
        .status()
        .context("Failed to run cargo")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("cargo {}", args_str),
        &format!("mycelium cargo {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_double_dash_with_separator() {
        // mycelium cargo test -- --nocapture → clap gives ["--nocapture"]
        let args: Vec<String> = vec!["--nocapture".into()];
        let raw = vec![
            "mycelium".into(),
            "cargo".into(),
            "test".into(),
            "--".into(),
            "--nocapture".into(),
        ];
        let result = restore_double_dash_with_raw(&args, &raw);
        assert_eq!(result, vec!["--", "--nocapture"]);
    }

    #[test]
    fn test_restore_double_dash_with_test_name() {
        // mycelium cargo test my_test -- --nocapture → clap gives ["my_test", "--nocapture"]
        let args: Vec<String> = vec!["my_test".into(), "--nocapture".into()];
        let raw = vec![
            "mycelium".into(),
            "cargo".into(),
            "test".into(),
            "my_test".into(),
            "--".into(),
            "--nocapture".into(),
        ];
        let result = restore_double_dash_with_raw(&args, &raw);
        assert_eq!(result, vec!["my_test", "--", "--nocapture"]);
    }

    #[test]
    fn test_restore_double_dash_without_separator() {
        // mycelium cargo test my_test → no --, args unchanged
        let args: Vec<String> = vec!["my_test".into()];
        let raw = vec![
            "mycelium".into(),
            "cargo".into(),
            "test".into(),
            "my_test".into(),
        ];
        let result = restore_double_dash_with_raw(&args, &raw);
        assert_eq!(result, vec!["my_test"]);
    }

    #[test]
    fn test_restore_double_dash_empty_args() {
        let args: Vec<String> = vec![];
        let raw = vec!["mycelium".into(), "cargo".into(), "test".into()];
        let result = restore_double_dash_with_raw(&args, &raw);
        assert!(result.is_empty());
    }

    #[test]
    fn test_restore_double_dash_clippy() {
        // mycelium cargo clippy -- -D warnings → clap gives ["-D", "warnings"]
        let args: Vec<String> = vec!["-D".into(), "warnings".into()];
        let raw = vec![
            "mycelium".into(),
            "cargo".into(),
            "clippy".into(),
            "--".into(),
            "-D".into(),
            "warnings".into(),
        ];
        let result = restore_double_dash_with_raw(&args, &raw);
        assert_eq!(result, vec!["--", "-D", "warnings"]);
    }
}
