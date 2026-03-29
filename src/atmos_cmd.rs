//! Atmos CLI output compression.
//!
//! Core Atmos flows are implemented here instead of relying on fallback shell plugins.

use crate::terraform_cmd;
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

pub fn run_terraform(args: &[String], verbose: u8) -> Result<()> {
    if args.is_empty() {
        return run_passthrough(&["terraform".to_string()], verbose);
    }

    match args[0].as_str() {
        "plan" => run_terraform_plan(&args[1..], verbose),
        "apply" => run_terraform_apply(&args[1..], verbose),
        "init" => run_terraform_init(&args[1..], verbose),
        _ => {
            let mut full_args = vec!["terraform".to_string()];
            full_args.extend_from_slice(args);
            run_passthrough(&full_args, verbose)
        }
    }
}

pub fn run_describe(args: &[String], verbose: u8) -> Result<()> {
    run_structured("describe", args, verbose)
}

pub fn run_validate(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(&build_subcommand_args("validate", args), verbose)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);
    let filtered = filter_validate(&raw);
    let original_cmd = format!("atmos validate {}", args.join(" "));
    let mycelium_cmd = format!("mycelium atmos validate {}", args.join(" "));

    println!("{}", filtered);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &filtered);

    let exit_code = output.status.code().unwrap_or(1);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

pub fn run_workflow(args: &[String], verbose: u8) -> Result<()> {
    run_structured("workflow", args, verbose)
}

pub fn run_version(args: &[String], verbose: u8) -> Result<()> {
    let mut full_args = vec!["version".to_string()];
    full_args.extend_from_slice(args);
    run_passthrough(&full_args, verbose)
}

pub fn run_passthrough(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(args, verbose)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);

    print!("{}", stdout);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    let args_display = args.join(" ");
    let original_cmd = format!("atmos {}", args_display);
    let mycelium_cmd = format!("mycelium atmos {}", args_display);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &raw);

    let exit_code = output.status.code().unwrap_or(1);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn run_terraform_plan(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(
        &build_subcommand_args("terraform", &prepend("plan", args)),
        verbose,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);
    let exit_code = output.status.code().unwrap_or(1);
    let original_cmd = format!("atmos terraform plan {}", args.join(" "));
    let mycelium_cmd = format!("mycelium atmos terraform plan {}", args.join(" "));

    if exit_code != 0 && exit_code != 2 {
        timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(exit_code);
    }

    let filtered = terraform_cmd::filter_plan(&stdout);
    println!("{}", filtered);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &filtered);

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn run_terraform_apply(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(
        &build_subcommand_args("terraform", &prepend("apply", args)),
        verbose,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);
    let exit_code = output.status.code().unwrap_or(1);
    let original_cmd = format!("atmos terraform apply {}", args.join(" "));
    let mycelium_cmd = format!("mycelium atmos terraform apply {}", args.join(" "));

    if exit_code != 0 {
        timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(exit_code);
    }

    let filtered = terraform_cmd::filter_apply(&stdout);
    println!("{}", filtered);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &filtered);
    Ok(())
}

fn run_terraform_init(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(
        &build_subcommand_args("terraform", &prepend("init", args)),
        verbose,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);
    let exit_code = output.status.code().unwrap_or(1);
    let original_cmd = format!("atmos terraform init {}", args.join(" "));
    let mycelium_cmd = format!("mycelium atmos terraform init {}", args.join(" "));

    if exit_code != 0 {
        timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(exit_code);
    }

    let filtered = terraform_cmd::filter_init(&stdout);
    println!("{}", filtered);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &filtered);
    Ok(())
}

fn run_structured(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = atmos_output(&build_subcommand_args(subcommand, args), verbose)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let raw = format!("{}{}", stdout, stderr);
    let filtered = filter_structured_output(&raw, 60);
    let original_cmd = format!("atmos {} {}", subcommand, args.join(" "));
    let mycelium_cmd = format!("mycelium atmos {} {}", subcommand, args.join(" "));

    println!("{}", filtered);
    timer.track(original_cmd.trim(), mycelium_cmd.trim(), &raw, &filtered);

    let exit_code = output.status.code().unwrap_or(1);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn atmos_output(args: &[String], verbose: u8) -> Result<std::process::Output> {
    let mut cmd = Command::new("atmos");
    cmd.args(args);
    if verbose > 0 {
        eprintln!("Running: atmos {}", args.join(" "));
    }
    cmd.output().context("Failed to run atmos")
}

fn build_subcommand_args(subcommand: &str, args: &[String]) -> Vec<String> {
    let mut full_args = vec![subcommand.to_string()];
    full_args.extend_from_slice(args);
    full_args
}

fn prepend(prefix: &str, args: &[String]) -> Vec<String> {
    let mut full_args = vec![prefix.to_string()];
    full_args.extend_from_slice(args);
    full_args
}

fn filter_structured_output(input: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() <= max_lines {
        return input.trim().to_string();
    }

    let mut result = lines[..max_lines].join("\n");
    result.push_str(&format!(
        "\n... ({} more lines truncated)",
        lines.len() - max_lines
    ));
    result
}

fn filter_validate(input: &str) -> String {
    let important: Vec<&str> = input
        .lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            lower.contains("error")
                || lower.contains("warning")
                || lower.contains("failed")
                || lower.contains("invalid")
        })
        .collect();

    if important.is_empty() {
        "✓ Validation passed".to_string()
    } else {
        important.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_structured_output_truncates_large_documents() {
        let input = (1..=65)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let output = filter_structured_output(&input, 60);
        assert!(output.contains("line 60"));
        assert!(output.contains("... (5 more lines truncated)"));
        assert!(!output.contains("line 61"));
    }

    #[test]
    fn test_filter_validate_success() {
        assert_eq!(filter_validate("all good\n"), "✓ Validation passed");
    }

    #[test]
    fn test_filter_validate_errors_only() {
        let input = "prefix\nError: bad config\nwarning: deprecated\nsuffix";
        let output = filter_validate(input);
        assert!(output.contains("Error: bad config"));
        assert!(output.contains("warning: deprecated"));
        assert!(!output.contains("prefix"));
        assert!(!output.contains("suffix"));
    }

    #[test]
    fn test_build_subcommand_args() {
        let args = build_subcommand_args("describe", &["stack".into(), "prod".into()]);
        assert_eq!(args, vec!["describe", "stack", "prod"]);
    }
}
