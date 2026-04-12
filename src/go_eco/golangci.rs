//! Token-optimized golangci-lint filter with JSON parsing and rule grouping.
use crate::tracking;
use crate::utils::truncate;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Position {
    #[serde(rename = "Filename")]
    filename: String,
}

#[derive(Debug, Deserialize)]
struct Issue {
    #[serde(rename = "FromLinter")]
    from_linter: String,
    #[serde(rename = "Pos")]
    pos: Position,
}

#[derive(Debug, Deserialize)]
struct GolangciOutput {
    #[serde(rename = "Issues")]
    issues: Vec<Issue>,
}

/// Execute `golangci-lint run` with JSON output and grouped issue summary.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let mut cmd = Command::new("golangci-lint");

    // Force JSON output
    let has_format = args
        .iter()
        .any(|a| a == "--out-format" || a.starts_with("--out-format="));

    if !has_format {
        cmd.arg("run").arg("--out-format=json");
    } else {
        cmd.arg("run");
    }

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: golangci-lint run --out-format=json");
    }

    let output = cmd.output().context(
        "Failed to run golangci-lint. Is it installed? Try: go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest",
    )?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let filtered = filter_golangci_json(&stdout);

    println!("{}", filtered);

    // Include stderr if present (config errors, etc.)
    if !stderr.trim().is_empty() && verbose > 0 {
        eprintln!("{}", stderr.trim());
    }

    timer.track(
        &format!("golangci-lint {}", args.join(" ")),
        &format!("mycelium golangci-lint {}", args.join(" ")),
        &raw,
        &filtered,
    );

    // golangci-lint returns exit code 1 when issues found (expected behavior)
    // Don't exit with error code in that case
    Ok(())
}

/// Filter golangci-lint JSON output - group by linter and file
fn filter_golangci_json(output: &str) -> String {
    let result: Result<GolangciOutput, _> = serde_json::from_str(output);

    let golangci_output = match result {
        Ok(o) => o,
        Err(e) => {
            // Fallback if JSON parsing fails
            return format!(
                "golangci-lint (JSON parse failed: {})\n{}",
                e,
                truncate(output, 500)
            );
        }
    };

    let issues = golangci_output.issues;

    if issues.is_empty() {
        return "✓ golangci-lint: No issues found".to_string();
    }

    let total_issues = issues.len();

    // Count unique files
    let unique_files: std::collections::HashSet<_> =
        issues.iter().map(|i| &i.pos.filename).collect();
    let total_files = unique_files.len();

    // Group by linter
    let mut by_linter: HashMap<String, usize> = HashMap::new();
    for issue in &issues {
        *by_linter.entry(issue.from_linter.clone()).or_insert(0) += 1;
    }

    // Group by file
    let mut by_file: HashMap<&str, usize> = HashMap::new();
    for issue in &issues {
        *by_file.entry(&issue.pos.filename).or_insert(0) += 1;
    }

    let mut file_counts: Vec<_> = by_file.iter().collect();
    file_counts.sort_by(|a, b| b.1.cmp(a.1));

    // Build output
    let mut result = String::new();
    result.push_str(&format!(
        "golangci-lint: {} issues in {} files\n",
        total_issues, total_files
    ));
    result.push_str("═══════════════════════════════════════\n");

    // Show top linters
    let mut linter_counts: Vec<_> = by_linter.iter().collect();
    linter_counts.sort_by(|a, b| b.1.cmp(a.1));

    if !linter_counts.is_empty() {
        result.push_str("Top linters:\n");
        for (linter, count) in linter_counts.iter().take(10) {
            result.push_str(&format!("  {} ({}x)\n", linter, count));
        }
        result.push('\n');
    }

    // Show top files
    result.push_str("Top files:\n");
    for (file, count) in file_counts.iter().take(10) {
        let short_path = compact_path(file);
        result.push_str(&format!("  {} ({} issues)\n", short_path, count));

        // Show top 3 linters in this file
        let mut file_linters: HashMap<String, usize> = HashMap::new();
        for issue in issues.iter().filter(|i| &i.pos.filename == *file) {
            *file_linters.entry(issue.from_linter.clone()).or_insert(0) += 1;
        }

        let mut file_linter_counts: Vec<_> = file_linters.iter().collect();
        file_linter_counts.sort_by(|a, b| b.1.cmp(a.1));

        for (linter, count) in file_linter_counts.iter().take(3) {
            result.push_str(&format!("    {} ({})\n", linter, count));
        }
    }

    if file_counts.len() > 10 {
        result.push_str(&format!("\n... +{} more files\n", file_counts.len() - 10));
    }

    result.trim().to_string()
}

/// Compact file path (remove common prefixes)
fn compact_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    if let Some(pos) = path.rfind("/pkg/") {
        format!("pkg/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/cmd/") {
        format!("cmd/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/internal/") {
        format!("internal/{}", &path[pos + 10..])
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
    fn test_filter_golangci_no_issues() {
        let output = r#"{"Issues":[]}"#;
        let result = filter_golangci_json(output);
        assert!(result.contains("✓ golangci-lint"));
        assert!(result.contains("No issues found"));
    }

    #[test]
    fn test_filter_golangci_with_issues() {
        let output = r#"{
  "Issues": [
    {
      "FromLinter": "errcheck",
      "Text": "Error return value not checked",
      "Pos": {"Filename": "main.go", "Line": 42, "Column": 5}
    },
    {
      "FromLinter": "errcheck",
      "Text": "Error return value not checked",
      "Pos": {"Filename": "main.go", "Line": 50, "Column": 10}
    },
    {
      "FromLinter": "gosimple",
      "Text": "Should use strings.Contains",
      "Pos": {"Filename": "utils.go", "Line": 15, "Column": 2}
    }
  ]
}"#;

        let result = filter_golangci_json(output);
        assert!(result.contains("3 issues"));
        assert!(result.contains("2 files"));
        assert!(result.contains("errcheck"));
        assert!(result.contains("gosimple"));
        assert!(result.contains("main.go"));
        assert!(result.contains("utils.go"));
    }

    #[test]
    fn test_compact_path() {
        assert_eq!(
            compact_path("/Users/foo/project/pkg/handler/server.go"),
            "pkg/handler/server.go"
        );
        assert_eq!(
            compact_path("/home/user/app/cmd/main/main.go"),
            "cmd/main/main.go"
        );
        assert_eq!(
            compact_path("/project/internal/config/loader.go"),
            "internal/config/loader.go"
        );
        assert_eq!(compact_path("relative/file.go"), "file.go");
    }

    #[test]
    fn test_filter_golangci_token_savings() {
        fn count_tokens(text: &str) -> usize {
            crate::tracking::estimate_tokens(text)
        }

        // Real golangci-lint JSON output includes SourceLines, Replacement, and other verbose
        // fields per issue. Adding those fields to each issue creates the bulk that the filter
        // strips, pushing savings comfortably above 60%.
        let input = r#"{
  "Issues": [
    {"FromLinter": "errcheck", "Text": "Error return value of `os.Remove` is not checked", "SourceLines": ["    os.Remove(tmpFile)"], "Replacement": null, "Pos": {"Filename": "cmd/main/main.go", "Line": 42, "Column": 5}},
    {"FromLinter": "errcheck", "Text": "Error return value of `db.Close` is not checked", "SourceLines": ["    db.Close()"], "Replacement": null, "Pos": {"Filename": "cmd/main/main.go", "Line": 50, "Column": 10}},
    {"FromLinter": "errcheck", "Text": "Error return value of `file.Close` is not checked", "SourceLines": ["    file.Close()"], "Replacement": null, "Pos": {"Filename": "pkg/handler/server.go", "Line": 25, "Column": 3}},
    {"FromLinter": "gosimple", "Text": "Should use strings.Contains(s, ...) instead of strings.Index(s, ...) >= 0", "SourceLines": ["    if strings.Index(s, needle) >= 0 {"], "Replacement": {"NeedOnlyDelete": false, "NewLines": ["    if strings.Contains(s, needle) {"]}, "Pos": {"Filename": "pkg/utils/strings.go", "Line": 15, "Column": 2}},
    {"FromLinter": "gosimple", "Text": "Redundant type conversion, use string(x) instead of fmt.Sprintf(\"%s\", x)", "SourceLines": ["    result := fmt.Sprintf(\"%s\", value)"], "Replacement": null, "Pos": {"Filename": "pkg/utils/types.go", "Line": 30, "Column": 8}},
    {"FromLinter": "govet", "Text": "Printf format %d has arg x of wrong type string", "SourceLines": ["    fmt.Printf(\"%d\", stringValue)"], "Replacement": null, "Pos": {"Filename": "internal/logger/log.go", "Line": 60, "Column": 4}},
    {"FromLinter": "staticcheck", "Text": "SA4006: this value of `err` is never used", "SourceLines": ["    err = doSomething()"], "Replacement": null, "Pos": {"Filename": "cmd/server/server.go", "Line": 100, "Column": 2}},
    {"FromLinter": "staticcheck", "Text": "SA1006: printf with dynamic first argument and no further arguments", "SourceLines": ["    fmt.Printf(msg)"], "Replacement": null, "Pos": {"Filename": "pkg/handler/handler.go", "Line": 75, "Column": 1}},
    {"FromLinter": "unused", "Text": "field `InternalID` is unused", "SourceLines": ["    InternalID string `json:\"-\"`"], "Replacement": null, "Pos": {"Filename": "pkg/types/types.go", "Line": 12, "Column": 2}},
    {"FromLinter": "ineffassign", "Text": "ineffectual assignment to `result`", "SourceLines": ["    result = computeValue()"], "Replacement": null, "Pos": {"Filename": "internal/config/loader.go", "Line": 55, "Column": 5}},
    {"FromLinter": "errcheck", "Text": "Error return value of `w.Write` is not checked", "SourceLines": ["    w.Write([]byte(response))"], "Replacement": null, "Pos": {"Filename": "pkg/handler/handler.go", "Line": 90, "Column": 3}},
    {"FromLinter": "gosimple", "Text": "Should use a simple channel send/receive instead of select with a single case", "SourceLines": ["    select { case ch <- val: }"], "Replacement": null, "Pos": {"Filename": "internal/worker/pool.go", "Line": 33, "Column": 6}}
  ]
}"#;

        let output = filter_golangci_json(input);
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 60,
            "golangci-lint filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
