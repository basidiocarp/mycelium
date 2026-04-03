//! GitHub CLI repo and api sub-command handlers.

use crate::filter::FilterResult;
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::has_json_flag;
use super::parsers::GhRepoViewParser;
use super::passthrough::{run_passthrough, run_passthrough_with_extra};

fn should_passthrough_repo_view(args: &[String]) -> bool {
    has_json_flag(args)
}

pub(super) fn dispatch_repo(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    // Parse subcommand (default to "view")
    let (subcommand, rest_args) = if args.is_empty() {
        ("view", args)
    } else {
        (args[0].as_str(), &args[1..])
    };

    if subcommand != "view" {
        return run_passthrough("gh", "repo", args);
    }

    if should_passthrough_repo_view(rest_args) {
        return run_passthrough_with_extra("gh", &["repo", "view"], rest_args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.arg("repo").arg("view");

    for arg in rest_args {
        cmd.arg(arg);
    }

    cmd.args([
        "--json",
        "name,owner,description,url,stargazerCount,forkCount,isPrivate",
    ]);

    let output = cmd.output().context("Failed to run gh repo view")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track_with_parse_info(
            "gh repo view",
            "mycelium gh repo view",
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

    let parse_result = GhRepoViewParser::parse(&raw);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };

    let filter_result = match parse_result {
        ParseResult::Full(repo) => {
            let out = repo.format(mode);
            FilterResult::full(&raw, out)
        }
        ParseResult::Degraded(repo, _) => {
            let out = repo.format(mode);
            FilterResult::degraded(&raw, out)
        }
        ParseResult::Passthrough(raw_out) => FilterResult::passthrough(&raw_out),
    };

    let validated = crate::hyphae::validate_filter_output(&raw, filter_result);
    print!("{}", validated.output);

    timer.track_with_parse_info(
        "gh repo view",
        "mycelium gh repo view",
        &raw,
        &validated.output,
        parse_tier,
        format_mode_str,
    );
    Ok(())
}

pub(super) fn dispatch_api(args: &[String], _verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.arg("api");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh api")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track(
            &format!("gh api {}", args.join(" ")),
            &format!("mycelium gh api {}", args.join(" ")),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let cmd_label = format!("gh api {}", args.join(" "));

    // Route through Hyphae for large API responses — they can be arbitrarily big.
    // The filter closure returns raw unchanged (gh api is explicit/advanced, so
    // we don't compress the content, but Hyphae can chunk very large responses).
    let filtered = crate::hyphae::route_or_filter(&cmd_label, &raw, |r| {
        FilterResult::full(r, r.to_string())
    });
    print!("{}", filtered.output);

    timer.track(
        &cmd_label,
        &format!("mycelium {}", cmd_label),
        &raw,
        &filtered.output,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_passthrough_repo_view;

    #[test]
    fn test_should_passthrough_repo_view_json_template_web() {
        assert!(should_passthrough_repo_view(&["--json".into()]));
        assert!(should_passthrough_repo_view(&["--json=name".into()]));
        assert!(should_passthrough_repo_view(&[
            "--jq".into(),
            ".name".into()
        ]));
        assert!(should_passthrough_repo_view(&["--jq=.name".into()]));
        assert!(should_passthrough_repo_view(&[
            "--template".into(),
            "{{.name}}".into()
        ]));
        assert!(should_passthrough_repo_view(&[
            "--template={{.name}}".into()
        ]));
        assert!(should_passthrough_repo_view(&["--web".into()]));
    }

    #[test]
    fn test_should_passthrough_repo_view_default() {
        assert!(!should_passthrough_repo_view(&[]));
    }
}
