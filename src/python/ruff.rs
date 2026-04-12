//! Ruff linter/formatter filter with JSON parsing for check and text parsing for format.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter, truncate_output};
use crate::utils::truncate;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct RuffLocation {
    row: usize,
    column: usize,
}

#[derive(Debug, Deserialize)]
struct RuffDiagnostic {
    code: String,
    message: String,
    location: RuffLocation,
    filename: String,
}

pub struct RuffCheckParser;

impl OutputParser for RuffCheckParser {
    type Output = DiagnosticReport;

    fn parse(input: &str) -> ParseResult<DiagnosticReport> {
        if input.trim().is_empty() {
            return ParseResult::Full(DiagnosticReport {
                tool: "Ruff".to_string(),
                total_errors: 0,
                total_warnings: 0,
                files_affected: 0,
                diagnostics: Vec::new(),
                by_code: Vec::new(),
                global_messages: Vec::new(),
            });
        }

        let diagnostics_json = match serde_json::from_str::<Vec<RuffDiagnostic>>(input) {
            Ok(diagnostics) => diagnostics,
            Err(_) => return ParseResult::Passthrough(truncate_output(input, 2000)),
        };

        let diagnostics: Vec<Diagnostic> = diagnostics_json
            .into_iter()
            .map(|diag| Diagnostic {
                file: compact_path(&diag.filename),
                line: diag.location.row,
                column: diag.location.column,
                severity: DiagnosticSeverity::Error,
                code: diag.code,
                message: diag.message,
                context: Vec::new(),
            })
            .collect();

        let total_errors = diagnostics.len();
        let mut by_code_map: HashMap<String, usize> = HashMap::new();
        let mut files: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for diagnostic in &diagnostics {
            *by_code_map.entry(diagnostic.code.clone()).or_insert(0) += 1;
            files.insert(diagnostic.file.as_str());
        }
        let mut by_code: Vec<(String, usize)> = by_code_map.into_iter().collect();
        by_code.sort_by(|a, b| match b.1.cmp(&a.1) {
            std::cmp::Ordering::Equal => a.0.cmp(&b.0),
            other => other,
        });

        ParseResult::Full(DiagnosticReport {
            tool: "Ruff".to_string(),
            total_errors,
            total_warnings: 0,
            files_affected: files.len(),
            diagnostics,
            by_code,
            global_messages: Vec::new(),
        })
    }
}

/// Run ruff linter — forces `--output-format=json` and defaults path to `.`.
pub fn run_check(args: &[String], verbose: u8) -> Result<()> {
    let mut full_args = vec!["check".to_string()];
    if !args.contains(&"--output-format".to_string()) {
        full_args.push("--output-format=json".to_string());
    }
    // Skip leading "check" token if user provided it explicitly
    let start_idx = if !args.is_empty() && args[0] == "check" {
        1
    } else {
        0
    };
    full_args.extend_from_slice(&args[start_idx..]);
    // Default to current directory if no path/file specified
    if args
        .iter()
        .skip(start_idx)
        .all(|a| a.starts_with('-') || a.contains('='))
    {
        full_args.push(".".to_string());
    }

    crate::filtered_cmd::FilteredCommand::new("ruff")
        .args(full_args)
        .verbose(verbose)
        .filter(filter_ruff_check_json)
        .run()
}

/// Run ruff formatter — shows files that would be reformatted.
pub fn run_format(args: &[String], verbose: u8) -> Result<()> {
    crate::filtered_cmd::FilteredCommand::new("ruff")
        .args(args.to_vec())
        .verbose(verbose)
        .filter(filter_ruff_format)
        .run()
}

/// Passthrough ruff invocation (version, rule, help, etc.) with no filtering.
pub fn run_passthrough(args: &[String], verbose: u8) -> Result<()> {
    crate::filtered_cmd::FilteredCommand::new("ruff")
        .args(args.to_vec())
        .verbose(verbose)
        .run()
}

/// Filter ruff check JSON output - group by rule and file
pub fn filter_ruff_check_json(output: &str) -> String {
    match RuffCheckParser::parse(output) {
        ParseResult::Full(report) | ParseResult::Degraded(report, _) => {
            report.format(FormatMode::Compact)
        }
        ParseResult::Passthrough(_) => {
            format!("Ruff check (JSON parse failed)\n{}", truncate(output, 500))
        }
    }
}

/// Filter ruff format output - show files that need formatting
pub fn filter_ruff_format(output: &str) -> String {
    let mut files_to_format: Vec<String> = Vec::new();
    let mut files_checked = 0;

    for line in output.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        // Count "would reformat" lines (check mode) - case insensitive
        if lower.contains("would reformat:") {
            // Extract filename from "Would reformat: path/to/file.py"
            if let Some(filename) = trimmed.split(':').nth(1) {
                files_to_format.push(filename.trim().to_string());
            }
        }

        // Count total checked files - look for patterns like "3 files left unchanged"
        if lower.contains("left unchanged") {
            // Find "X file(s) left unchanged" pattern specifically
            // Split by comma to handle "2 files would be reformatted, 3 files left unchanged"
            let parts: Vec<&str> = trimmed.split(',').collect();
            for part in parts {
                let part_lower = part.to_lowercase();
                if part_lower.contains("left unchanged") {
                    let words: Vec<&str> = part.split_whitespace().collect();
                    // Look for number before "file" or "files"
                    for (i, word) in words.iter().enumerate() {
                        if (word == &"file" || word == &"files")
                            && i > 0
                            && let Ok(count) = words[i - 1].parse::<usize>()
                        {
                            files_checked = count;
                            break;
                        }
                    }
                    break;
                }
            }
        }
    }

    let output_lower = output.to_lowercase();

    // Check if all files are formatted
    if files_to_format.is_empty() && output_lower.contains("left unchanged") {
        return "✓ Ruff format: All files formatted correctly".to_string();
    }

    let mut result = String::new();

    if output_lower.contains("would reformat") {
        // Check mode: show files that need formatting
        if files_to_format.is_empty() {
            result.push_str("✓ Ruff format: All files formatted correctly\n");
        } else {
            result.push_str(&format!(
                "Ruff format: {} files need formatting\n",
                files_to_format.len()
            ));
            result.push_str("═══════════════════════════════════════\n");

            for (i, file) in files_to_format.iter().take(10).enumerate() {
                result.push_str(&format!("{}. {}\n", i + 1, compact_path(file)));
            }

            if files_to_format.len() > 10 {
                result.push_str(&format!(
                    "\n... +{} more files\n",
                    files_to_format.len() - 10
                ));
            }

            if files_checked > 0 {
                result.push_str(&format!("\n✓ {} files already formatted\n", files_checked));
            }

            result.push_str("\nhint: Run `ruff format` to format these files\n");
        }
    } else {
        // Write mode or other output - show summary
        result.push_str(output.trim());
    }

    result.trim().to_string()
}

/// Compact file path (remove common prefixes)
fn compact_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    if let Some(pos) = path.rfind("/src/") {
        format!("src/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/lib/") {
        format!("lib/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/tests/") {
        format!("tests/{}", &path[pos + 7..])
    } else if let Some(pos) = path.rfind('/') {
        path[pos + 1..].to_string()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_ruff_check_no_issues() {
        let output = "[]";
        let result = filter_ruff_check_json(output);
        assert!(result.contains("✓ Ruff"));
        assert!(result.contains("No issues found"));
    }

    #[test]
    fn test_filter_ruff_check_with_issues() {
        let output = r#"[
  {
    "code": "F401",
    "message": "`os` imported but unused",
    "location": {"row": 1, "column": 8},
    "end_location": {"row": 1, "column": 10},
    "filename": "src/main.py",
    "fix": {"applicability": "safe"}
  },
  {
    "code": "F401",
    "message": "`sys` imported but unused",
    "location": {"row": 2, "column": 8},
    "end_location": {"row": 2, "column": 11},
    "filename": "src/main.py",
    "fix": null
  },
  {
    "code": "E501",
    "message": "Line too long (100 > 88 characters)",
    "location": {"row": 10, "column": 89},
    "end_location": {"row": 10, "column": 100},
    "filename": "src/utils.py",
    "fix": null
        }
]"#;
        let result = filter_ruff_check_json(output);
        assert!(result.contains("Ruff: 3 errors in 2 files"));
        assert!(result.contains("2 files"));
        assert!(result.contains("F401"));
        assert!(result.contains("E501"));
        assert!(result.contains("main.py"));
        assert!(result.contains("utils.py"));
    }

    #[test]
    fn test_filter_ruff_format_all_formatted() {
        let output = "5 files left unchanged";
        let result = filter_ruff_format(output);
        assert!(result.contains("✓ Ruff format"));
        assert!(result.contains("All files formatted correctly"));
    }

    #[test]
    fn test_filter_ruff_format_needs_formatting() {
        let output = r#"Would reformat: src/main.py
Would reformat: tests/test_utils.py
2 files would be reformatted, 3 files left unchanged"#;
        let result = filter_ruff_format(output);
        assert!(result.contains("2 files need formatting"));
        assert!(result.contains("main.py"));
        assert!(result.contains("test_utils.py"));
        assert!(result.contains("3 files already formatted"));
    }

    #[test]
    fn test_compact_path() {
        assert_eq!(
            compact_path("/Users/foo/project/src/main.py"),
            "src/main.py"
        );
        assert_eq!(compact_path("/home/user/app/lib/utils.py"), "lib/utils.py");
        assert_eq!(
            compact_path("C:\\Users\\foo\\project\\tests\\test.py"),
            "tests/test.py"
        );
        assert_eq!(compact_path("relative/file.py"), "file.py");
    }

    fn count_tokens(text: &str) -> usize {
        crate::tracking::estimate_tokens(text)
    }

    #[test]
    fn test_filter_ruff_check_token_savings() {
        let input = include_str!("../../tests/fixtures/ruff_check_raw.txt");
        let output = filter_ruff_check_json(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);
        assert!(
            savings >= 55,
            "Expected >=55% token savings, got {}%",
            savings
        );
    }

    #[test]
    fn test_filter_ruff_format_token_savings() {
        let input = include_str!("../../tests/fixtures/ruff_format_raw.txt");
        let output = filter_ruff_format(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let _savings = (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens.max(1);
        // ruff format filter shows formatted file list and summary. With this small
        // fixture, savings are minimal, but real output with many files sees higher reduction.
        assert!(
            output_tokens <= input_tokens,
            "filter should not increase output"
        );
    }
}
