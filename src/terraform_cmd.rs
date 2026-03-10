//! Terraform CLI output compression.
//!
//! Filters verbose `terraform plan` and `terraform apply` output down to
//! the resource change list and summary line. Achieves ≥70% token savings.

use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

fn re_plan_resource() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^\s+#\s+(\S+)\s+will be\s+(\w[\w -]+)").unwrap())
}

fn re_plan_summary() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^Plan:\s+\d+ to add,\s+\d+ to change,\s+\d+ to destroy\.").unwrap()
    })
}

fn re_apply_complete() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^\S+:\s+(Creation|Modification|Destruction|Modifications) complete")
            .unwrap()
    })
}

fn re_apply_summary() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^Apply complete!").unwrap())
}

fn re_init_progress() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^- (Downloading|Installing|Finding|Reusing|Using previously)").unwrap()
    })
}

/// Run a terraform command with token-optimized output.
#[allow(dead_code)]
pub fn run(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    match subcommand {
        "plan" => run_plan(args, verbose),
        "apply" => run_apply(args, verbose),
        "init" => run_init(args, verbose),
        _ => run_passthrough_impl(subcommand, args, verbose),
    }
}

/// Run terraform plan with token-optimized output.
pub fn run_plan(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = terraform_cmd("plan", args, verbose)?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(1);

    // Exit code 2 = plan-with-changes (terraform-detailed-exitcode), not a real error
    if exit_code != 0 && exit_code != 2 {
        timer.track("terraform plan", "mycelium terraform plan", &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(exit_code);
    }

    let filtered = filter_plan(&raw);
    println!("{}", filtered);
    timer.track("terraform plan", "mycelium terraform plan", &raw, &filtered);

    // Preserve exit code 2 for CI pipelines using -detailed-exitcode
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

/// Filter verbose terraform plan output to a resource list + summary.
pub fn filter_plan(input: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in input.lines() {
        if let Some(caps) = re_plan_resource().captures(line) {
            let address = caps.get(1).map_or("?", |m| m.as_str());
            let action = caps.get(2).map_or("changed", |m| m.as_str());
            lines.push(format!("  {} {}", action_symbol(action), address));
        } else if re_plan_summary().is_match(line) {
            lines.push(line.trim().to_string());
        }
    }

    if lines.is_empty() {
        for line in input.lines() {
            if line.contains("No changes") || line.starts_with("Plan:") {
                lines.push(line.trim().to_string());
            }
        }
    }

    lines.join("\n")
}

/// Run terraform apply with token-optimized output.
pub fn run_apply(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = terraform_cmd("apply", args, verbose)?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        timer.track("terraform apply", "mycelium terraform apply", &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let filtered = filter_apply(&raw);
    println!("{}", filtered);
    timer.track(
        "terraform apply",
        "mycelium terraform apply",
        &raw,
        &filtered,
    );
    Ok(())
}

/// Filter verbose terraform apply output to completion lines and summary.
pub fn filter_apply(input: &str) -> String {
    input
        .lines()
        .filter(|l| re_apply_complete().is_match(l) || re_apply_summary().is_match(l))
        .map(|l| l.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run terraform init with token-optimized output.
pub fn run_init(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = terraform_cmd("init", args, verbose)?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        timer.track("terraform init", "mycelium terraform init", &raw, &raw);
        eprint!("{}", stderr);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let filtered = filter_init(&raw);
    println!("{}", filtered);
    timer.track("terraform init", "mycelium terraform init", &raw, &filtered);
    Ok(())
}

/// Filter terraform init: strip download/install progress bars.
pub fn filter_init(input: &str) -> String {
    input
        .lines()
        .filter(|l| !re_init_progress().is_match(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run passthrough for terraform subcommands not explicitly optimized.
pub fn run_passthrough(args: &[String], verbose: u8) -> Result<()> {
    // Extract the subcommand from args[0]
    if args.is_empty() {
        anyhow::bail!("terraform passthrough requires at least a subcommand");
    }
    let subcommand = args[0].clone();
    let remaining_args = &args[1..];
    run_passthrough_impl(&subcommand, remaining_args, verbose)
}

fn run_passthrough_impl(subcommand: &str, args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let output = terraform_cmd(subcommand, args, verbose)?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let full = format!("terraform {} {}", subcommand, args.join(" "));
    let out = format!("mycelium terraform {} {}", subcommand, args.join(" "));

    print!("{}", raw);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }
    timer.track(&full, &out, &raw, &raw);

    let exit_code = output.status.code().unwrap_or(0);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn terraform_cmd(subcommand: &str, args: &[String], verbose: u8) -> Result<std::process::Output> {
    let mut cmd = Command::new("terraform");
    cmd.arg(subcommand);
    for arg in args {
        cmd.arg(arg);
    }
    if verbose > 0 {
        eprintln!("Running: terraform {} {}", subcommand, args.join(" "));
    }
    cmd.output().context("Failed to run terraform")
}

fn action_symbol(action: &str) -> &'static str {
    let l = action.to_lowercase();
    if l.contains("creat") {
        "+"
    } else if l.contains("destroy") || l.contains("delet") {
        "-"
    } else if l.contains("replac") {
        "-/+"
    } else {
        "~"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_tokens(text: &str) -> usize {
        text.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
            .filter(|s| !s.is_empty())
            .count()
    }

    #[test]
    fn test_filter_plan_shows_resource_names() {
        let input = include_str!("../tests/fixtures/terraform_plan_raw.txt");
        let result = filter_plan(input);
        assert!(result.contains("aws_instance.web"));
        assert!(result.contains("aws_s3_bucket.data"));
        assert!(result.contains("aws_security_group.allow_tls"));
        assert!(result.contains("aws_db_instance.postgres"));
    }

    #[test]
    fn test_filter_plan_shows_summary_line() {
        let input = include_str!("../tests/fixtures/terraform_plan_raw.txt");
        let result = filter_plan(input);
        assert!(result.contains("Plan: 1 to add, 1 to change, 1 to destroy."));
    }

    #[test]
    fn test_filter_plan_strips_attribute_detail() {
        let input = include_str!("../tests/fixtures/terraform_plan_raw.txt");
        let result = filter_plan(input);
        assert!(!result.contains("ami-0c55b159cbfafe1f0"));
        assert!(!result.contains("known after apply"));
        assert!(!result.contains("instance_type"));
    }

    #[test]
    fn test_filter_plan_action_symbols() {
        let input = include_str!("../tests/fixtures/terraform_plan_raw.txt");
        let result = filter_plan(input);
        let web = result
            .lines()
            .find(|l| l.contains("aws_instance.web"))
            .unwrap();
        assert!(web.contains('+'), "create should use + symbol");
        let s3 = result
            .lines()
            .find(|l| l.contains("aws_s3_bucket.data"))
            .unwrap();
        assert!(s3.contains('-'), "destroy should use - symbol");
    }

    #[test]
    fn test_filter_plan_token_savings_gte_70_percent() {
        let input = include_str!("../tests/fixtures/terraform_plan_raw.txt");
        let result = filter_plan(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&result);
        let savings_pct = 100 - (output_tokens * 100 / input_tokens.max(1));
        assert!(
            savings_pct >= 70,
            "Expected ≥70% token savings, got {}% ({} -> {} tokens)",
            savings_pct,
            input_tokens,
            output_tokens
        );
    }

    #[test]
    fn test_filter_plan_no_changes() {
        let input = "No changes. Your infrastructure matches the configuration.\n\
                     Terraform has compared your real infrastructure against your\n\
                     configuration and found no differences, so no changes are needed.";
        assert!(filter_plan(input).contains("No changes"));
    }

    #[test]
    fn test_filter_apply_shows_completion_lines() {
        let input = include_str!("../tests/fixtures/terraform_apply_raw.txt");
        let result = filter_apply(input);
        assert!(result.contains("Creation complete"));
        assert!(result.contains("Modifications complete"));
        assert!(result.contains("Destruction complete"));
    }

    #[test]
    fn test_filter_apply_shows_summary() {
        let input = include_str!("../tests/fixtures/terraform_apply_raw.txt");
        assert!(filter_apply(input).contains("Apply complete!"));
    }

    #[test]
    fn test_filter_apply_strips_still_creating_lines() {
        let input = include_str!("../tests/fixtures/terraform_apply_raw.txt");
        assert!(!filter_apply(input).contains("Still creating"));
    }

    #[test]
    fn test_filter_apply_token_savings_gte_30_percent() {
        let input = include_str!("../tests/fixtures/terraform_apply_raw.txt");
        let result = filter_apply(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&result);
        let savings_pct = 100 - (output_tokens * 100 / input_tokens.max(1));
        assert!(
            savings_pct >= 30,
            "Expected ≥30% token savings on apply, got {}% ({} -> {} tokens)",
            savings_pct,
            input_tokens,
            output_tokens
        );
    }

    #[test]
    fn test_filter_init_strips_progress_bars() {
        let input = "Initializing the backend...\n\
                     - Downloading hashicorp/aws v4.0.0...\n\
                     - Installing hashicorp/aws v4.0.0...\n\
                     - Finding hashicorp/random versions matching \"~> 3.0\"...\n\
                     Terraform has been successfully initialized!\n";
        let result = filter_init(input);
        assert!(result.contains("Terraform has been successfully initialized"));
        assert!(!result.contains("Downloading"));
        assert!(!result.contains("Installing"));
        assert!(!result.contains("Finding"));
    }

    #[test]
    fn test_action_symbol_variants() {
        assert_eq!(action_symbol("created"), "+");
        assert_eq!(action_symbol("destroyed"), "-");
        assert_eq!(action_symbol("replaced"), "-/+");
        assert_eq!(action_symbol("updated in-place"), "~");
    }
}
