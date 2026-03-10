//! PR list subcommand handler.

use crate::{tracking, utils::truncate};
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

pub fn list_prs(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
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
            filtered.push_str("📋 Pull Requests\n");
            println!("📋 Pull Requests");
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
                    "OPEN" => "🟢",
                    "MERGED" => "🟣",
                    "CLOSED" => "🔴",
                    _ => "⚪",
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

    timer.track("gh pr list", "mycelium gh pr list", &raw, &filtered);
    Ok(())
}
