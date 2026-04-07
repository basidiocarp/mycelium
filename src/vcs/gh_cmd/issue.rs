//! GitHub CLI issue sub-command handlers.

use crate::filter::FilterResult;
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::has_json_flag;
use super::parsers::{GhIssueListParser, GhIssueViewParser};
use super::passthrough::{run_passthrough, run_passthrough_with_extra};

pub(super) fn dispatch_issue(args: &[String], verbose: u8, ultra_compact: bool) -> Result<()> {
    if args.is_empty() {
        return run_passthrough("gh", "issue", args);
    }

    match args[0].as_str() {
        "list" if should_passthrough_issue_list(&args[1..]) => {
            run_passthrough_with_extra("gh", &["issue", "list"], &args[1..])
        }
        "list" => list_issues(&args[1..], verbose, ultra_compact),
        "view" => view_issue(&args[1..], verbose),
        _ => run_passthrough("gh", "issue", args),
    }
}

pub fn should_passthrough_issue_list(args: &[String]) -> bool {
    has_json_flag(args)
}

pub fn should_passthrough_issue_view(extra_args: &[String]) -> bool {
    extra_args.iter().any(|a| a == "--comments") || has_json_flag(extra_args)
}

fn list_issues(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(["issue", "list", "--json", "number,title,state,author"]);

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh issue list")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            "gh issue list",
            "mycelium gh issue list",
            &stderr,
            &stderr,
            3,
            "compact",
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let mode = if ultra_compact {
        FormatMode::Ultra
    } else {
        FormatMode::Compact
    };
    let format_mode_str = if ultra_compact { "ultra" } else { "compact" };

    let parse_result = GhIssueListParser::parse(&raw);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };

    let filter_result = match parse_result {
        ParseResult::Full(list) => {
            let out = list.format(mode);
            FilterResult::full(&raw, out)
        }
        ParseResult::Degraded(list, _) => {
            let out = list.format(mode);
            FilterResult::degraded(&raw, out)
        }
        ParseResult::Passthrough(raw_out) => FilterResult::passthrough(&raw_out),
    };

    let validated = crate::hyphae::validate_filter_output(&raw, filter_result);
    print!("{}", validated.output);

    timer.track_with_parse_info(
        "gh issue list",
        "mycelium gh issue list",
        &raw,
        &validated.output,
        parse_tier,
        format_mode_str,
    );
    Ok(())
}

fn view_issue(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (issue_number, extra_args) = match super::extract_identifier_and_extra_args(args) {
        Some(result) => result,
        None => return Err(anyhow::anyhow!("Issue number required")),
    };

    if should_passthrough_issue_view(&extra_args) {
        return super::passthrough::run_passthrough_with_extra(
            "gh",
            &["issue", "view", &issue_number],
            &extra_args,
        );
    }

    let mut cmd = Command::new("gh");
    cmd.args([
        "issue",
        "view",
        &issue_number,
        "--json",
        "number,title,state,author,body,url",
    ]);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh issue view")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            &format!("gh issue view {}", issue_number),
            &format!("mycelium gh issue view {}", issue_number),
            &stderr,
            &stderr,
            3,
            "compact",
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let cmd_label = format!("gh issue view {}", issue_number);

    // Route through Hyphae for large issues (many comments can be huge).
    // Small/medium output is filtered locally with validation.
    let filtered = crate::hyphae::route_or_filter(&cmd_label, &raw, |r| format_issue_view(r));
    print!("{}", filtered.output);

    timer.track_with_parse_info(
        &cmd_label,
        &format!("mycelium gh issue view {}", issue_number),
        &raw,
        &filtered.output,
        parse_tier_from_raw(&raw),
        "compact",
    );
    Ok(())
}

/// Format gh issue view JSON output into a compact summary.
fn format_issue_view(raw: &str) -> FilterResult {
    let parse_result = GhIssueViewParser::parse(raw);
    match parse_result {
        ParseResult::Full(detail) => FilterResult::full(raw, detail.format_compact()),
        ParseResult::Degraded(detail, _) => FilterResult::degraded(raw, detail.format_compact()),
        ParseResult::Passthrough(raw_out) => FilterResult::passthrough(&raw_out),
    }
}

/// Determine parse tier from raw output (for tracking).
fn parse_tier_from_raw(raw: &str) -> u8 {
    match GhIssueViewParser::parse(raw) {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{should_passthrough_issue_list, should_passthrough_issue_view};

    #[test]
    fn test_should_passthrough_issue_list_json_template_web() {
        assert!(should_passthrough_issue_list(&["--json".into()]));
        assert!(should_passthrough_issue_list(&[
            "--json=number,title".into()
        ]));
        assert!(should_passthrough_issue_list(&[
            "--jq".into(),
            ".title".into()
        ]));
        assert!(should_passthrough_issue_list(&["--jq=.title".into()]));
        assert!(should_passthrough_issue_list(&[
            "--template".into(),
            "{{.title}}".into()
        ]));
        assert!(should_passthrough_issue_list(&[
            "--template={{.title}}".into()
        ]));
        assert!(should_passthrough_issue_list(&["--web".into()]));
    }

    #[test]
    fn test_should_passthrough_issue_view_comments_and_formatting() {
        assert!(should_passthrough_issue_view(&["--comments".into()]));
        assert!(should_passthrough_issue_view(&["--json".into()]));
        assert!(should_passthrough_issue_view(&["--json=body".into()]));
        assert!(should_passthrough_issue_view(&[
            "--jq".into(),
            ".body".into()
        ]));
        assert!(should_passthrough_issue_view(&["--jq=.body".into()]));
        assert!(should_passthrough_issue_view(&[
            "--template".into(),
            "{{.body}}".into()
        ]));
        assert!(should_passthrough_issue_view(&[
            "--template={{.body}}".into()
        ]));
        assert!(should_passthrough_issue_view(&["--web".into()]));
    }
}
