//! PR view, checks, and status subcommand handlers.

use crate::vcs::gh_cmd::{
    extract_identifier_and_extra_args, filter_markdown_body, run_passthrough_with_extra,
};
use crate::{tracking, utils::truncate};
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

pub fn should_passthrough_pr_view(extra_args: &[String]) -> bool {
    extra_args
        .iter()
        .any(|a| a == "--json" || a == "--jq" || a == "--web")
}

pub fn view_pr(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (pr_number, extra_args) = match extract_identifier_and_extra_args(args) {
        Some(result) => result,
        None => return Err(anyhow::anyhow!("PR number required")),
    };

    // If the user provides --jq or --web, pass through directly.
    // Note: --json is already handled globally by run() via has_json_flag.
    if should_passthrough_pr_view(&extra_args) {
        return run_passthrough_with_extra("gh", &["pr", "view", &pr_number], &extra_args);
    }

    let mut cmd = Command::new("gh");
    cmd.args([
        "pr",
        "view",
        &pr_number,
        "--json",
        "number,title,state,author,body,url,mergeable,reviews,statusCheckRollup",
    ]);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr view")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track(
            &format!("gh pr view {}", pr_number),
            &format!("mycelium gh pr view {}", pr_number),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let json: Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse gh pr view output")?;

    let mut filtered = String::new();

    let number = json["number"].as_i64().unwrap_or(0);
    let title = json["title"].as_str().unwrap_or("???");
    let state = json["state"].as_str().unwrap_or("???");
    let author = json["author"]["login"].as_str().unwrap_or("???");
    let url = json["url"].as_str().unwrap_or("");
    let mergeable = json["mergeable"].as_str().unwrap_or("UNKNOWN");

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

    let line = format!("{} PR #{}: {}\n", state_icon, number, title);
    filtered.push_str(&line);
    print!("{}", line);

    let line = format!("  {}\n", author);
    filtered.push_str(&line);
    print!("{}", line);

    let mergeable_str = match mergeable {
        "MERGEABLE" => "✓",
        "CONFLICTING" => "✗",
        _ => "?",
    };
    let line = format!("  {} | {}\n", state, mergeable_str);
    filtered.push_str(&line);
    print!("{}", line);

    if let Some(reviews) = json["reviews"]["nodes"].as_array() {
        let approved = reviews
            .iter()
            .filter(|r| r["state"].as_str() == Some("APPROVED"))
            .count();
        let changes = reviews
            .iter()
            .filter(|r| r["state"].as_str() == Some("CHANGES_REQUESTED"))
            .count();

        if approved > 0 || changes > 0 {
            let line = format!(
                "  Reviews: {} approved, {} changes requested\n",
                approved, changes
            );
            filtered.push_str(&line);
            print!("{}", line);
        }
    }

    if let Some(checks) = json["statusCheckRollup"].as_array() {
        let total = checks.len();
        let passed = checks
            .iter()
            .filter(|c| {
                c["conclusion"].as_str() == Some("SUCCESS")
                    || c["state"].as_str() == Some("SUCCESS")
            })
            .count();
        let failed = checks
            .iter()
            .filter(|c| {
                c["conclusion"].as_str() == Some("FAILURE")
                    || c["state"].as_str() == Some("FAILURE")
            })
            .count();

        if ultra_compact {
            if failed > 0 {
                let line = format!("  ✗{}/{}  {} fail\n", passed, total, failed);
                filtered.push_str(&line);
                print!("{}", line);
            } else {
                let line = format!("  ✓{}/{}\n", passed, total);
                filtered.push_str(&line);
                print!("{}", line);
            }
        } else {
            let line = format!("  Checks: {}/{} passed\n", passed, total);
            filtered.push_str(&line);
            print!("{}", line);
            if failed > 0 {
                let line = format!("  ⚠️  {} checks failed\n", failed);
                filtered.push_str(&line);
                print!("{}", line);
            }
        }
    }

    let line = format!("  {}\n", url);
    filtered.push_str(&line);
    print!("{}", line);

    if let Some(body) = json["body"].as_str() {
        if !body.is_empty() {
            let body_filtered = filter_markdown_body(body);
            if !body_filtered.is_empty() {
                filtered.push('\n');
                println!();
                for line in body_filtered.lines() {
                    let formatted = format!("  {}\n", line);
                    filtered.push_str(&formatted);
                    print!("{}", formatted);
                }
            }
        }
    }

    timer.track(
        &format!("gh pr view {}", pr_number),
        &format!("mycelium gh pr view {}", pr_number),
        &raw,
        &filtered,
    );
    Ok(())
}

pub fn pr_checks(args: &[String], _verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (pr_number, extra_args) = match extract_identifier_and_extra_args(args) {
        Some(result) => result,
        None => return Err(anyhow::anyhow!("PR number required")),
    };

    let mut cmd = Command::new("gh");
    cmd.args(["pr", "checks", &pr_number]);
    for arg in &extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().context("Failed to run gh pr checks")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track(
            &format!("gh pr checks {}", pr_number),
            &format!("mycelium gh pr checks {}", pr_number),
            &stderr,
            &stderr,
        );
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut passed = 0;
    let mut failed = 0;
    let mut pending = 0;
    let mut failed_checks = Vec::new();

    for line in stdout.lines() {
        if line.contains('✓') || line.contains("pass") {
            passed += 1;
        } else if line.contains('✗') || line.contains("fail") {
            failed += 1;
            failed_checks.push(line.trim().to_string());
        } else if line.contains('*') || line.contains("pending") {
            pending += 1;
        }
    }

    let mut filtered = String::new();

    let line = "🔍 CI Checks Summary:\n";
    filtered.push_str(line);
    print!("{}", line);

    let line = format!("  ✅ Passed: {}\n", passed);
    filtered.push_str(&line);
    print!("{}", line);

    let line = format!("  ❌ Failed: {}\n", failed);
    filtered.push_str(&line);
    print!("{}", line);

    if pending > 0 {
        let line = format!("  ⏳ Pending: {}\n", pending);
        filtered.push_str(&line);
        print!("{}", line);
    }

    if !failed_checks.is_empty() {
        let line = "\n  Failed checks:\n";
        filtered.push_str(line);
        print!("{}", line);
        for check in failed_checks {
            let line = format!("    {}\n", check);
            filtered.push_str(&line);
            print!("{}", line);
        }
    }

    timer.track(
        &format!("gh pr checks {}", pr_number),
        &format!("mycelium gh pr checks {}", pr_number),
        &raw,
        &filtered,
    );
    Ok(())
}

pub fn pr_status(_verbose: u8, _ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("gh");
    cmd.args([
        "pr",
        "status",
        "--json",
        "currentBranch,createdBy,reviewDecision,statusCheckRollup",
    ]);

    let output = cmd.output().context("Failed to run gh pr status")?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        timer.track("gh pr status", "mycelium gh pr status", &stderr, &stderr);
        eprintln!("{}", stderr.trim());
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let json: Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse gh pr status output")?;

    let mut filtered = String::new();

    if let Some(created_by) = json["createdBy"].as_array() {
        let line = format!("📝 Your PRs ({}):\n", created_by.len());
        filtered.push_str(&line);
        print!("{}", line);
        for pr in created_by.iter().take(5) {
            let number = pr["number"].as_i64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("???");
            let reviews = pr["reviewDecision"].as_str().unwrap_or("PENDING");
            let line = format!("  #{} {} [{}]\n", number, truncate(title, 50), reviews);
            filtered.push_str(&line);
            print!("{}", line);
        }
    }

    timer.track("gh pr status", "mycelium gh pr status", &raw, &filtered);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vcs::gh_cmd::filter_markdown_body;

    #[test]
    fn test_should_passthrough_pr_view_json() {
        assert!(should_passthrough_pr_view(&[
            "--json".into(),
            "body,comments".into()
        ]));
    }

    #[test]
    fn test_should_passthrough_pr_view_jq() {
        assert!(should_passthrough_pr_view(&["--jq".into(), ".body".into()]));
    }

    #[test]
    fn test_should_passthrough_pr_view_web() {
        assert!(should_passthrough_pr_view(&["--web".into()]));
    }

    #[test]
    fn test_should_passthrough_pr_view_default() {
        assert!(!should_passthrough_pr_view(&[]));
    }

    #[test]
    fn test_should_passthrough_pr_view_other_flags() {
        assert!(!should_passthrough_pr_view(&["--comments".into()]));
    }

    #[test]
    fn test_filter_markdown_body_html_comment_single_line() {
        let input = "Hello\n<!-- this is a comment -->\nWorld";
        let result = filter_markdown_body(input);
        assert!(!result.contains("<!--"));
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_filter_markdown_body_html_comment_multiline() {
        let input = "Before\n<!--\nmultiline\ncomment\n-->\nAfter";
        let result = filter_markdown_body(input);
        assert!(!result.contains("<!--"));
        assert!(!result.contains("multiline"));
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
    }

    #[test]
    fn test_filter_markdown_body_badge_lines() {
        let input = "# Title\n[![CI](https://img.shields.io/badge.svg)](https://github.com/actions)\nSome text";
        let result = filter_markdown_body(input);
        assert!(!result.contains("shields.io"));
        assert!(result.contains("# Title"));
        assert!(result.contains("Some text"));
    }

    #[test]
    fn test_filter_markdown_body_image_only_lines() {
        let input = "# Title\n![screenshot](https://example.com/img.png)\nSome text";
        let result = filter_markdown_body(input);
        assert!(!result.contains("![screenshot]"));
        assert!(result.contains("# Title"));
        assert!(result.contains("Some text"));
    }

    #[test]
    fn test_filter_markdown_body_horizontal_rules() {
        let input = "Section 1\n---\nSection 2\n***\nSection 3\n___\nEnd";
        let result = filter_markdown_body(input);
        assert!(!result.contains("---"));
        assert!(!result.contains("***"));
        assert!(!result.contains("___"));
        assert!(result.contains("Section 1"));
        assert!(result.contains("Section 2"));
        assert!(result.contains("Section 3"));
    }

    #[test]
    fn test_filter_markdown_body_blank_lines_collapse() {
        let input = "Line 1\n\n\n\n\nLine 2";
        let result = filter_markdown_body(input);
        assert!(!result.contains("\n\n\n"));
        assert!(result.contains("Line 1"));
        assert!(result.contains("Line 2"));
    }

    #[test]
    fn test_filter_markdown_body_code_block_preserved() {
        let input = "Text before\n```python\n<!-- not a comment -->\n![not an image](url)\n---\n```\nText after";
        let result = filter_markdown_body(input);
        assert!(result.contains("<!-- not a comment -->"));
        assert!(result.contains("![not an image](url)"));
        assert!(result.contains("---"));
        assert!(result.contains("Text before"));
        assert!(result.contains("Text after"));
    }

    #[test]
    fn test_filter_markdown_body_empty() {
        assert_eq!(filter_markdown_body(""), "");
    }

    #[test]
    fn test_filter_markdown_body_meaningful_content_preserved() {
        let input = "## Summary\n- Item 1\n- Item 2\n\n[Link](https://example.com)\n\n| Col1 | Col2 |\n| --- | --- |\n| a | b |";
        let result = filter_markdown_body(input);
        assert!(result.contains("## Summary"));
        assert!(result.contains("- Item 1"));
        assert!(result.contains("- Item 2"));
        assert!(result.contains("[Link](https://example.com)"));
        assert!(result.contains("| Col1 | Col2 |"));
    }

    #[test]
    fn test_filter_markdown_body_token_savings() {
        let input = r#"<!-- This PR template is auto-generated -->
<!-- Please fill in the following sections -->

## Summary

Added smart markdown filtering for gh issue/pr view commands.

---

## Changes

- Filter HTML comments
- Filter badge lines
- Filter image-only lines
- Collapse blank lines

***

## Test Plan

- [x] Unit tests added
- [x] Snapshot tests pass
- [ ] Manual testing

___

<!-- Do not edit below this line -->
<!-- Auto-generated footer -->"#;

        let result = filter_markdown_body(input);

        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&result);
        let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);

        assert!(
            savings >= 30.0,
            "Expected ≥30% savings, got {:.1}% (input: {} tokens, output: {} tokens)",
            savings,
            input_tokens,
            output_tokens
        );

        assert!(result.contains("## Summary"));
        assert!(result.contains("## Changes"));
        assert!(result.contains("## Test Plan"));
        assert!(result.contains("Filter HTML comments"));
    }
}
