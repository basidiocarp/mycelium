//! GitHub CLI repo and api sub-command handlers.

use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

use super::parsers::GhRepoViewParser;
use super::passthrough::run_passthrough;

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

    let filtered = match parse_result {
        ParseResult::Full(repo) | ParseResult::Degraded(repo, _) => {
            let out = repo.format(mode);
            println!("{}", out);
            out
        }
        ParseResult::Passthrough(raw_out) => {
            print!("{}", raw_out);
            raw_out
        }
    };

    timer.track_with_parse_info(
        "gh repo view",
        "mycelium gh repo view",
        &raw,
        &filtered,
        parse_tier,
        format_mode_str,
    );
    Ok(())
}

pub(super) fn dispatch_api(args: &[String], _verbose: u8) -> Result<()> {
    // gh api is an explicit/advanced command — the user knows what they asked for.
    // Converting JSON to a schema destroys all values and forces Claude to re-fetch.
    // Passthrough preserves the full response and tracks metrics at 0% savings.
    run_passthrough("gh", "api", args)
}
