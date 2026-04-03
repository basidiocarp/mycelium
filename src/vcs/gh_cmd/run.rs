//! GitHub CLI workflow run sub-command handlers.

use crate::filter::FilterResult;
use crate::parser::{
    FormatMode, OutputParser, ParseResult, TokenFormatter, emit_passthrough_warning,
};
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::has_json_flag;
use super::parsers::{GhRunListParser, GhRunViewParser};
use super::passthrough::{run_passthrough, run_passthrough_with_extra};

pub(super) fn run_workflow(args: &[String], verbose: u8, ultra_compact: bool) -> Result<()> {
    if args.is_empty() {
        return run_passthrough("gh", "run", args);
    }

    match args[0].as_str() {
        "list" if should_passthrough_run_list(&args[1..]) => {
            run_passthrough_with_extra("gh", &["run", "list"], &args[1..])
        }
        "list" => list_runs(&args[1..], verbose, ultra_compact),
        "view" => view_run(&args[1..], verbose),
        _ => run_passthrough("gh", "run", args),
    }
}

fn should_passthrough_run_list(args: &[String]) -> bool {
    has_json_flag(args)
}

fn list_runs(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    if should_passthrough_run_list(args) {
        return run_passthrough_with_extra("gh", &["run", "list"], args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args([
        "run",
        "list",
        "--json",
        "databaseId,name,status,conclusion,createdAt",
    ]);
    cmd.arg("--limit").arg("10");

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh run list")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            "gh run list",
            "mycelium gh run list",
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

    let parse_result = GhRunListParser::parse(&raw);
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
        "gh run list",
        "mycelium gh run list",
        &raw,
        &validated.output,
        parse_tier,
        format_mode_str,
    );
    Ok(())
}

struct RunViewAnalysis {
    filtered: String,
    parse_tier: u8,
    _filter_result: FilterResult,
}

fn filter_run_view_output(raw: &str, run_id: Option<&str>) -> RunViewAnalysis {
    match GhRunViewParser::parse(raw) {
        ParseResult::Full(mut summary) => {
            summary.run_id = run_id.map(ToOwned::to_owned);
            let filtered = summary.format_compact();
            let filter_result = FilterResult::full(raw, filtered.clone());
            RunViewAnalysis {
                filtered,
                parse_tier: 1,
                _filter_result: filter_result,
            }
        }
        ParseResult::Degraded(mut summary, warnings) => {
            summary.run_id = run_id.map(ToOwned::to_owned);
            emit_passthrough_warning("gh run view", &warnings.join(", "));
            let filtered = summary.format_compact();
            let filter_result = FilterResult::degraded(raw, filtered.clone());
            RunViewAnalysis {
                filtered,
                parse_tier: 2,
                _filter_result: filter_result,
            }
        }
        ParseResult::Passthrough(raw_out) => {
            emit_passthrough_warning("gh run view", "No parseable summary lines found");
            let filter_result = FilterResult::passthrough(&raw_out);
            RunViewAnalysis {
                filtered: raw_out,
                parse_tier: 3,
                _filter_result: filter_result,
            }
        }
    }
}

/// Check if run view args should bypass filtering and pass through directly.
/// Flags like --log-failed, --log, and --json produce output that the filter
/// would incorrectly strip.
fn should_passthrough_run_view(extra_args: &[String]) -> bool {
    extra_args
        .iter()
        .any(|a| a == "--log-failed" || a == "--log")
        || has_json_flag(extra_args)
}

fn view_run(args: &[String], _verbose: u8) -> Result<()> {
    let (run_id, extra_args) = super::extract_optional_identifier_and_extra_args(args);
    let base_args = if let Some(ref run_id) = run_id {
        vec!["run", "view", run_id.as_str()]
    } else {
        vec!["run", "view"]
    };

    // Pass through when user requests logs or JSON — the filter would strip them
    if should_passthrough_run_view(&extra_args) {
        return run_passthrough_with_extra("gh", &base_args, &extra_args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args(&base_args);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh run view")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let run_label = run_id
            .as_deref()
            .map(|id| format!("gh run view {}", id))
            .unwrap_or_else(|| "gh run view".to_string());
        let mycelium_label = run_id
            .as_deref()
            .map(|id| format!("mycelium gh run view {}", id))
            .unwrap_or_else(|| "mycelium gh run view".to_string());
        timer.track_with_parse_info(&run_label, &mycelium_label, &stderr, &stderr, 3, "compact");
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let run_label = run_id
        .as_deref()
        .map(|id| format!("gh run view {}", id))
        .unwrap_or_else(|| "gh run view".to_string());
    let mycelium_label = run_id
        .as_deref()
        .map(|id| format!("mycelium gh run view {}", id))
        .unwrap_or_else(|| "mycelium gh run view".to_string());

    // Route through Hyphae for large run views (workflows with many jobs/steps).
    let run_id_clone = run_id.clone();
    let filtered = crate::hyphae::route_or_filter(&run_label, &raw, |r| {
        filter_run_view_output_as_result(r, run_id_clone.as_deref())
    });
    print!("{}", filtered.output);

    let parse_tier = filter_run_view_output(&raw, run_id.as_deref()).parse_tier;
    timer.track_with_parse_info(
        &run_label,
        &mycelium_label,
        &raw,
        &filtered.output,
        parse_tier,
        "compact",
    );
    Ok(())
}

fn filter_run_view_output_as_result(raw: &str, run_id: Option<&str>) -> FilterResult {
    let analysis = filter_run_view_output(raw, run_id);
    match GhRunViewParser::parse(raw) {
        ParseResult::Full(_) => FilterResult::full(raw, analysis.filtered),
        ParseResult::Degraded(_, _) => FilterResult::degraded(raw, analysis.filtered),
        ParseResult::Passthrough(_) => FilterResult::passthrough(&analysis.filtered),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_view_passthrough_log_failed() {
        assert!(should_passthrough_run_view(&["--log-failed".into()]));
    }

    #[test]
    fn test_run_view_passthrough_log() {
        assert!(should_passthrough_run_view(&["--log".into()]));
    }

    #[test]
    fn test_run_view_passthrough_json() {
        assert!(should_passthrough_run_view(&[
            "--json".into(),
            "jobs".into()
        ]));
    }

    #[test]
    fn test_run_view_passthrough_template_and_web() {
        assert!(should_passthrough_run_view(&[
            "--jq".into(),
            ".jobs".into()
        ]));
        assert!(should_passthrough_run_view(&["--jq=.jobs".into()]));
        assert!(should_passthrough_run_view(&[
            "--template".into(),
            "{{.jobs}}".into()
        ]));
        assert!(should_passthrough_run_view(
            &["--template={{.jobs}}".into()]
        ));
        assert!(should_passthrough_run_view(&["--web".into()]));
    }

    #[test]
    fn test_run_view_no_passthrough_empty() {
        assert!(!should_passthrough_run_view(&[]));
    }

    #[test]
    fn test_run_view_no_passthrough_other_flags() {
        assert!(!should_passthrough_run_view(&["--exit-status".into()]));
    }

    #[test]
    fn test_run_list_passthrough_json_template() {
        assert!(should_passthrough_run_list(&["--json".into()]));
        assert!(should_passthrough_run_list(&["--json=databaseId".into()]));
        assert!(should_passthrough_run_list(&[
            "--jq".into(),
            ".workflowName".into()
        ]));
        assert!(should_passthrough_run_list(&["--jq=.workflowName".into()]));
        assert!(should_passthrough_run_list(&[
            "--template".into(),
            "{{.workflowName}}".into()
        ]));
        assert!(should_passthrough_run_list(&[
            "--template={{.workflowName}}".into()
        ]));
        assert!(!should_passthrough_run_list(&[
            "--limit".into(),
            "5".into()
        ]));
    }

    #[test]
    fn test_filter_run_view_output_reports_failed_jobs_only() {
        let raw = "\
Workflow Run #123
Status: completed
Conclusion: failure

JOBS
✓ build in 2m10s
X integration-tests in 4m20s
✓ lint in 45s
";

        let parsed = filter_run_view_output(raw, Some("123"));
        assert_eq!(parsed.parse_tier, 1);
        assert!(parsed.filtered.contains("Workflow Run #123"));
        assert!(
            parsed
                .filtered
                .contains("FAIL: X integration-tests in 4m20s")
        );
        assert!(!parsed.filtered.contains("✓ build"));
    }

    #[test]
    fn test_filter_run_view_output_passthroughs_unstructured_text() {
        let raw = "gh: authentication required";
        let parsed = filter_run_view_output(raw, None);
        assert_eq!(parsed.parse_tier, 3);
    }
}
