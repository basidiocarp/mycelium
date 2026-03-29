//! Pylint JSON output parser, formatter, and generic lint fallback.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{
    FormatMode, OutputParser, ParseResult, TokenFormatter, emit_passthrough_warning,
    truncate_output,
};
use crate::utils::truncate;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct PylintDiagnostic {
    #[serde(rename = "type")]
    msg_type: String, // "warning", "error", "convention", "refactor"
    #[allow(dead_code)]
    module: String,
    #[allow(dead_code)]
    obj: String,
    line: usize,
    column: usize,
    path: String,
    symbol: String, // rule code like "unused-variable"
    message: String,
    #[serde(rename = "message-id")]
    message_id: String, // e.g., "W0612"
}

/// Parser for Pylint JSON output format.
pub(super) struct PylintParser;

impl OutputParser for PylintParser {
    type Output = DiagnosticReport;

    fn parse(input: &str) -> ParseResult<DiagnosticReport> {
        match serde_json::from_str::<Vec<PylintDiagnostic>>(input) {
            Ok(items) => {
                let mut diagnostics = Vec::new();
                for item in &items {
                    let severity = match item.msg_type.as_str() {
                        "error" | "fatal" => DiagnosticSeverity::Error,
                        _ => DiagnosticSeverity::Warning,
                    };
                    let code = format!("{} ({})", item.symbol, item.message_id);
                    let file = super::compact_path(&item.path);
                    diagnostics.push(Diagnostic {
                        file,
                        line: item.line,
                        column: item.column,
                        severity,
                        code,
                        message: item.message.clone(),
                        context: Vec::new(),
                    });
                }
                let total_errors = diagnostics
                    .iter()
                    .filter(|d| matches!(d.severity, DiagnosticSeverity::Error))
                    .count();
                let total_warnings = diagnostics
                    .iter()
                    .filter(|d| matches!(d.severity, DiagnosticSeverity::Warning))
                    .count();
                let mut by_code_map: HashMap<String, usize> = HashMap::new();
                for d in &diagnostics {
                    *by_code_map.entry(d.code.clone()).or_insert(0) += 1;
                }
                let mut by_code: Vec<(String, usize)> = by_code_map.into_iter().collect();
                by_code.sort_by(|a, b| match b.1.cmp(&a.1) {
                    std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                    other => other,
                });
                let files_affected: std::collections::HashSet<&str> =
                    diagnostics.iter().map(|d| d.file.as_str()).collect();
                let files_affected = files_affected.len();
                ParseResult::Full(DiagnosticReport {
                    tool: "Pylint".to_string(),
                    total_errors,
                    total_warnings,
                    files_affected,
                    diagnostics,
                    by_code,
                    global_messages: Vec::new(),
                })
            }
            Err(e) => {
                emit_passthrough_warning("pylint", &format!("JSON parse failed: {}", e));
                ParseResult::Passthrough(truncate_output(input, 2000))
            }
        }
    }
}

/// Filter pylint JSON2 output - group by symbol and file
#[allow(dead_code)]
pub fn filter_pylint_json(output: &str) -> String {
    let report = match PylintParser::parse(output) {
        ParseResult::Full(r) | ParseResult::Degraded(r, _) => r,
        ParseResult::Passthrough(raw_out) => return raw_out,
    };

    report.format(FormatMode::Compact)
}

/// Filter generic linter output (fallback for non-ESLint/Pylint linters)
#[allow(dead_code)]
pub fn filter_generic_lint(output: &str) -> String {
    let mut warnings = 0;
    let mut errors = 0;
    let mut issues: Vec<String> = Vec::new();

    for line in output.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("warning") {
            warnings += 1;
            issues.push(line.to_string());
        }
        if line_lower.contains("error") && !line_lower.contains("0 error") {
            errors += 1;
            issues.push(line.to_string());
        }
    }

    if errors == 0 && warnings == 0 {
        return "✓ Lint: No issues found".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!("Lint: {} errors, {} warnings\n", errors, warnings));
    result.push_str("═══════════════════════════════════════\n");

    for issue in issues.iter().take(20) {
        result.push_str(&format!("{}\n", truncate(issue, 100)));
    }

    if issues.len() > 20 {
        result.push_str(&format!("\n... +{} more issues\n", issues.len() - 20));
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_pylint_json_no_issues() {
        let output = "[]";
        let result = filter_pylint_json(output);
        assert!(result.contains("✓ Pylint"));
        assert!(result.contains("No issues found"));
    }

    #[test]
    fn test_filter_pylint_json_with_issues() {
        let json = r#"[
            {
                "type": "warning",
                "module": "main",
                "obj": "",
                "line": 10,
                "column": 0,
                "path": "src/main.py",
                "symbol": "unused-variable",
                "message": "Unused variable 'x'",
                "message-id": "W0612"
            },
            {
                "type": "warning",
                "module": "main",
                "obj": "foo",
                "line": 15,
                "column": 4,
                "path": "src/main.py",
                "symbol": "unused-variable",
                "message": "Unused variable 'y'",
                "message-id": "W0612"
            },
            {
                "type": "error",
                "module": "utils",
                "obj": "bar",
                "line": 20,
                "column": 0,
                "path": "src/utils.py",
                "symbol": "undefined-variable",
                "message": "Undefined variable 'z'",
                "message-id": "E0602"
            }
        ]"#;

        let result = filter_pylint_json(json);
        assert!(result.contains("Pylint: 1 errors in 2 files, 2 warnings"));
        assert!(result.contains("2 files"));
        assert!(result.contains("unused-variable (W0612)"));
        assert!(result.contains("undefined-variable (E0602)"));
        assert!(result.contains("main.py"));
        assert!(result.contains("utils.py"));
    }

    #[test]
    fn test_pylint_snapshot() {
        let input = include_str!("../../tests/fixtures/pylint_json_raw.txt");
        let output = filter_pylint_json(input);
        insta::assert_snapshot!(output);
    }

    fn count_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    #[test]
    fn test_pylint_token_savings() {
        let input = include_str!("../../tests/fixtures/pylint_json_raw.txt");
        let output = filter_pylint_json(input);
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 45,
            "Expected >= 45% token savings, got {}%",
            savings
        );
    }
}
