//! Generic fallback lint parser for unknown linters.

use crate::parser::{OutputParser, ParseResult, truncate_output};

pub(super) struct GenericLintParser;

impl OutputParser for GenericLintParser {
    type Output = crate::parser::types::DiagnosticReport;

    fn parse(input: &str) -> ParseResult<Self::Output> {
        use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};

        let mut diagnostics = Vec::new();
        let mut warnings = 0usize;
        let mut errors = 0usize;

        for (idx, line) in input.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let lowered = trimmed.to_lowercase();
            let severity = if lowered.contains("error") && !lowered.contains("0 error") {
                errors += 1;
                Some(DiagnosticSeverity::Error)
            } else if lowered.contains("warning") {
                warnings += 1;
                Some(DiagnosticSeverity::Warning)
            } else {
                None
            };

            if let Some(severity) = severity {
                diagnostics.push(Diagnostic {
                    file: "output".to_string(),
                    line: idx + 1,
                    column: 0,
                    severity,
                    code: "generic".to_string(),
                    message: trimmed.to_string(),
                    context: Vec::new(),
                });
            }
        }

        if diagnostics.is_empty() {
            if input.trim().is_empty()
                || input.to_lowercase().contains("no issues")
                || input.to_lowercase().contains("0 errors")
            {
                return ParseResult::Full(DiagnosticReport {
                    tool: "Lint".to_string(),
                    total_errors: 0,
                    total_warnings: 0,
                    files_affected: 0,
                    diagnostics: Vec::new(),
                    by_code: Vec::new(),
                    global_messages: Vec::new(),
                });
            }

            return ParseResult::Passthrough(truncate_output(input, 2000));
        }

        ParseResult::Degraded(
            DiagnosticReport {
                tool: "Lint".to_string(),
                total_errors: errors,
                total_warnings: warnings,
                files_affected: 1,
                diagnostics,
                by_code: vec![("generic".to_string(), errors + warnings)],
                global_messages: Vec::new(),
            },
            vec!["generic heuristic lint parser".to_string()],
        )
    }
}
