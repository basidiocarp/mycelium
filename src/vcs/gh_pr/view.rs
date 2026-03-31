//! PR view subcommand handlers.

use crate::tracking;
use crate::vcs::gh_cmd::{
    extract_optional_identifier_and_extra_args, filter_markdown_body, run_passthrough_with_extra,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

pub fn should_passthrough_pr_view(extra_args: &[String]) -> bool {
    extra_args.iter().any(|a| {
        a == "--comments" || a == "--json" || a == "--jq" || a == "--template" || a == "--web"
    })
}

pub fn view_pr(args: &[String], _verbose: u8, ultra_compact: bool) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let (pr_number, extra_args) = extract_optional_identifier_and_extra_args(args);
    let base_args = if let Some(ref pr_number) = pr_number {
        vec!["pr", "view", pr_number.as_str()]
    } else {
        vec!["pr", "view"]
    };

    // If the user provides output-shaping flags, pass through directly.
    // Note: --json is already handled globally by run() via has_json_flag.
    if should_passthrough_pr_view(&extra_args) {
        return run_passthrough_with_extra("gh", &base_args, &extra_args);
    }

    let mut cmd = Command::new("gh");
    cmd.args(&base_args);
    cmd.args([
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
        let pr_label = pr_number
            .as_deref()
            .map(|n| format!("gh pr view {}", n))
            .unwrap_or_else(|| "gh pr view".to_string());
        let mycelium_label = pr_number
            .as_deref()
            .map(|n| format!("mycelium gh pr view {}", n))
            .unwrap_or_else(|| "mycelium gh pr view".to_string());
        timer.track(&pr_label, &mycelium_label, &stderr, &stderr);
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
            "OPEN" => "open",
            "MERGED" => "merged",
            "CLOSED" => "closed",
            _ => "-",
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
                let line = format!("  {} checks failed\n", failed);
                filtered.push_str(&line);
                print!("{}", line);
            }
        }
    }

    let line = format!("  {}\n", url);
    filtered.push_str(&line);
    print!("{}", line);

    if let Some(body) = json["body"].as_str()
        && !body.is_empty()
    {
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

    let pr_label = pr_number
        .as_deref()
        .map(|n| format!("gh pr view {}", n))
        .unwrap_or_else(|| "gh pr view".to_string());
    let mycelium_label = pr_number
        .as_deref()
        .map(|n| format!("mycelium gh pr view {}", n))
        .unwrap_or_else(|| "mycelium gh pr view".to_string());
    timer.track(&pr_label, &mycelium_label, &raw, &filtered);
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
    fn test_should_passthrough_pr_view_comments_and_template() {
        assert!(should_passthrough_pr_view(&["--comments".into()]));
        assert!(should_passthrough_pr_view(&[
            "--template".into(),
            "{{.body}}".into()
        ]));
    }

    #[test]
    fn test_should_passthrough_pr_view_default() {
        assert!(!should_passthrough_pr_view(&[]));
    }

    #[test]
    fn test_should_passthrough_pr_view_other_flags() {
        assert!(!should_passthrough_pr_view(&[]));
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
            crate::tracking::estimate_tokens(text)
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
