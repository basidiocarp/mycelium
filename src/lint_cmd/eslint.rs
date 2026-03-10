//! ESLint JSON output parser and formatter.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{emit_passthrough_warning, truncate_output, OutputParser, ParseResult};
use crate::utils::truncate;
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
                by_code.sort_by(|a, b| b.1.cmp(&a.1));
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
pub fn filter_eslint_json(output: &str) -> String {
    let report = match EslintParser::parse(output) {
        ParseResult::Full(r) | ParseResult::Degraded(r, _) => r,
        ParseResult::Passthrough(_) => {
            return format!(
                "ESLint output (JSON parse failed)\n{}",
                truncate(output, 500)
            );
        }
    };

    let total_errors = report.total_errors;
    let total_warnings = report.total_warnings;
    let total_files = report.files_affected;

    if total_errors == 0 && total_warnings == 0 {
        return "✓ ESLint: No issues found".to_string();
    }

    let mut result = String::new();
    result.push_str(&format!(
        "ESLint: {} errors, {} warnings in {} files\n",
        total_errors, total_warnings, total_files
    ));
    result.push_str("═══════════════════════════════════════\n");

    if !report.by_code.is_empty() {
        result.push_str("Top rules:\n");
        for (rule, count) in report.by_code.iter().take(10) {
            result.push_str(&format!("  {} ({}x)\n", rule, count));
        }
        result.push('\n');
    }

    // Group diagnostics by file for per-file breakdown
    let mut by_file: HashMap<&str, Vec<&Diagnostic>> = HashMap::new();
    for d in &report.diagnostics {
        by_file.entry(d.file.as_str()).or_default().push(d);
    }
    let mut file_list: Vec<(&str, usize)> = by_file.iter().map(|(f, ds)| (*f, ds.len())).collect();
    file_list.sort_by(|a, b| b.1.cmp(&a.1));

    result.push_str("Top files:\n");
    for (file, count) in file_list.iter().take(10) {
        result.push_str(&format!("  {} ({} issues)\n", file, count));

        let file_diags = &by_file[file];
        let mut file_rules: HashMap<&str, usize> = HashMap::new();
        for d in file_diags.iter() {
            *file_rules.entry(d.code.as_str()).or_insert(0) += 1;
        }
        let mut file_rule_counts: Vec<_> = file_rules.iter().collect();
        file_rule_counts.sort_by(|a, b| b.1.cmp(a.1));
        for (rule, count) in file_rule_counts.iter().take(3) {
            result.push_str(&format!("    {} ({})\n", rule, count));
        }
    }

    if file_list.len() > 10 {
        result.push_str(&format!("\n... +{} more files\n", file_list.len() - 10));
    }

    result.trim().to_string()
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
}
