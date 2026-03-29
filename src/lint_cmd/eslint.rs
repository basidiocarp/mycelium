//! ESLint JSON output parser and formatter.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{
    FormatMode, OutputParser, ParseResult, TokenFormatter, emit_passthrough_warning,
    truncate_output,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct EslintMessage {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: usize,
    column: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct EslintResult {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<EslintMessage>,
    #[serde(rename = "errorCount")]
    error_count: usize,
    #[serde(rename = "warningCount")]
    warning_count: usize,
}

/// Parser for ESLint JSON output format.
pub(super) struct EslintParser;

impl OutputParser for EslintParser {
    type Output = DiagnosticReport;

    fn parse(input: &str) -> ParseResult<DiagnosticReport> {
        match serde_json::from_str::<Vec<EslintResult>>(input) {
            Ok(results) => {
                let mut diagnostics = Vec::new();
                for result in &results {
                    for msg in &result.messages {
                        let severity = if msg.severity >= 2 {
                            DiagnosticSeverity::Error
                        } else {
                            DiagnosticSeverity::Warning
                        };
                        let code = msg.rule_id.clone().unwrap_or_else(|| "unknown".to_string());
                        let file = super::compact_path(&result.file_path);
                        diagnostics.push(Diagnostic {
                            file,
                            line: msg.line,
                            column: msg.column,
                            severity,
                            code,
                            message: msg.message.clone(),
                            context: Vec::new(),
                        });
                    }
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
                    tool: "ESLint".to_string(),
                    total_errors,
                    total_warnings,
                    files_affected,
                    diagnostics,
                    by_code,
                    global_messages: Vec::new(),
                })
            }
            Err(e) => {
                emit_passthrough_warning("eslint", &format!("JSON parse failed: {}", e));
                ParseResult::Passthrough(truncate_output(input, 2000))
            }
        }
    }
}

/// Filter ESLint JSON output - group by rule and file
#[allow(dead_code)]
pub fn filter_eslint_json(output: &str) -> String {
    let report = match EslintParser::parse(output) {
        ParseResult::Full(r) | ParseResult::Degraded(r, _) => r,
        ParseResult::Passthrough(raw_out) => return raw_out,
    };

    report.format(FormatMode::Compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_eslint_json() {
        let json = r#"[
            {
                "filePath": "/Users/test/project/src/utils.ts",
                "messages": [
                    {
                        "ruleId": "prefer-const",
                        "severity": 1,
                        "message": "Use const instead of let",
                        "line": 10,
                        "column": 5
                    },
                    {
                        "ruleId": "prefer-const",
                        "severity": 1,
                        "message": "Use const instead of let",
                        "line": 15,
                        "column": 5
                    }
                ],
                "errorCount": 0,
                "warningCount": 2
            },
            {
                "filePath": "/Users/test/project/src/api.ts",
                "messages": [
                    {
                        "ruleId": "@typescript-eslint/no-unused-vars",
                        "severity": 2,
                        "message": "Variable x is unused",
                        "line": 20,
                        "column": 10
                    }
                ],
                "errorCount": 1,
                "warningCount": 0
            }
        ]"#;

        let result = filter_eslint_json(json);
        assert!(result.contains("ESLint:"));
        assert!(result.contains("prefer-const"));
        assert!(result.contains("no-unused-vars"));
        assert!(result.contains("src/utils.ts"));
    }

    #[test]
    fn test_eslint_snapshot() {
        let input = include_str!("../../tests/fixtures/eslint_json_raw.txt");
        let output = filter_eslint_json(input);
        insta::assert_snapshot!(output);
    }

    fn count_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    #[test]
    fn test_eslint_token_savings() {
        let input = include_str!("../../tests/fixtures/eslint_json_raw.txt");
        let output = filter_eslint_json(input);
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 45,
            "Expected >= 45% token savings, got {}%",
            savings
        );
    }
}
