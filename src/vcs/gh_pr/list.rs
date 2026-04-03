//! PR list subcommand handler.

use crate::filter::FilterResult;
use crate::{tracking, utils::truncate};
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

use crate::vcs::gh_cmd::run_passthrough_with_extra;

pub fn should_passthrough_pr_list(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--json" || a == "--jq" || a == "--template" || a == "--web")
}

pub fn list_prs(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    if should_passthrough_pr_list(args) {
        return run_passthrough_with_extra("gh", &["pr", "list"], args);
    }

    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args([
        "pr",
        "list",
        "--json",
        "number,title,state,author,updatedAt",
    ]);

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr list")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track("gh pr list", "mycelium gh pr list", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let json: Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse gh pr list output")?;

    let mut filtered = String::new();

    if let Some(prs) = json.as_array() {
        if ultra_compact {
            filtered.push_str("PRs\n");
            println!("PRs");
        } else {
            filtered.push_str("Pull Requests\n");
            println!("Pull Requests");
        }

        for pr in prs.iter().take(20) {
            let number = pr["number"].as_i64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("???");
            let state = pr["state"].as_str().unwrap_or("???");
            let author = pr["author"]["login"].as_str().unwrap_or("???");

            let state_icon = if ultra_compact {
                match state {
                    "OPEN" => "O",
                    "MERGED" => "M",
                    "CLOSED" => "C",
                    _ => "?",
                }
            } else {
                match state {
                    "OPEN" => "open",
                    "MERGED" => "merged",
                    "CLOSED" => "closed",
                    _ => "-",
                }
            };

            let line = format!(
                "  {} #{} {} ({})\n",
                state_icon,
                number,
                truncate(title, 60),
                author
            );
            filtered.push_str(&line);
            print!("{}", line);
        }

        if prs.len() > 20 {
            let more_line = format!("  ... {} more (use gh pr list for all)\n", prs.len() - 20);
            filtered.push_str(&more_line);
            print!("{}", more_line);
        }
    }

    // JSON parse succeeded → Full quality.
    let _filter_result = FilterResult::full(&raw, filtered.clone());

    timer.track("gh pr list", "mycelium gh pr list", &raw, &filtered);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_passthrough_pr_list;

    #[test]
    fn test_should_passthrough_pr_list_json_template_web() {
        assert!(should_passthrough_pr_list(&["--json".into()]));
        assert!(should_passthrough_pr_list(&[
            "--jq".into(),
            ".title".into()
        ]));
        assert!(should_passthrough_pr_list(&[
            "--template".into(),
            "{{.title}}".into()
        ]));
        assert!(should_passthrough_pr_list(&["--web".into()]));
    }

    #[test]
    fn test_should_passthrough_pr_list_default() {
        assert!(!should_passthrough_pr_list(&[]));
    }
}
