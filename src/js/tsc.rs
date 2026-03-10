//! TypeScript compiler filter that groups errors by file and error code.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter};
use crate::utils::which_command;
use anyhow::{Context, Result};
use regex::Regex;

/// Run the TypeScript compiler and filter output to group errors by file and code.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    use crate::{tee, tracking, utils};

    let (cmd, base_args): (String, Vec<String>) = if which_command("tsc").is_some() {
        ("tsc".to_string(), vec![])
    } else {
        ("npx".to_string(), vec!["tsc".to_string()])
    };

    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("Running: {} {}", cmd, args.join(" "));
    }

    let output = std::process::Command::new(&cmd)
        .args(base_args.iter().chain(args.iter()))
        .output()
        .with_context(|| format!("Failed to run {}", cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}{}", stdout, stderr);

    let parse_result = TscParser::parse(&raw);
    let parse_tier: u8 = match &parse_result {
        ParseResult::Full(_) => 1,
        ParseResult::Degraded(_, _) => 2,
        ParseResult::Passthrough(_) => 3,
    };
    let mode = FormatMode::Compact;
    let filtered = match parse_result {
        ParseResult::Full(report) => report.format(mode),
        ParseResult::Degraded(report, _) => report.format(mode),
        ParseResult::Passthrough(raw_out) => raw_out,
    };

    let exit_code = utils::exit_code(&output.status);
    if let Some(hint) = tee::tee_and_hint(&raw, "tsc", exit_code) {
        println!("{}\n{}", filtered, hint);
    } else {
        println!("{}", filtered);
    }

    let raw_label = format!("{} {}", cmd, args.join(" "));
    let rtk_label = format!("mycelium tsc {}", args.join(" "));
    timer.track_with_parse_info(
        &raw_label, &rtk_label, &raw, &filtered, parse_tier, "compact",
    );

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Parser for TypeScript compiler output.
pub struct TscParser;

impl OutputParser for TscParser {
    type Output = DiagnosticReport;

    fn parse(input: &str) -> ParseResult<DiagnosticReport> {
        fn tsc_error() -> &'static Regex {
            static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
            RE.get_or_init(|| Regex::new(
                r"^(.+?)\((\d+),(\d+)\):\s+(error|warning)\s+(TS\d+):\s+(.+)$"
            ).unwrap())
        }

        let mut diagnostics: Vec<Diagnostic> = Vec::new();
        let lines: Vec<&str> = input.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            if let Some(caps) = tsc_error().captures(line) {
                let severity = if &caps[4] == "error" {
                    DiagnosticSeverity::Error
                } else {
                    DiagnosticSeverity::Warning
                };
                let mut diag = Diagnostic {
                    file: caps[1].to_string(),
                    line: caps[2].parse().unwrap_or(0),
                    column: caps[3].parse().unwrap_or(0),
                    severity,
                    code: caps[5].to_string(),
                    message: caps[6].to_string(),
                    context: Vec::new(),
                };
                i += 1;
                while i < lines.len() {
                    let next = lines[i];
                    if !next.is_empty()
                        && (next.starts_with("  ") || next.starts_with('\t'))
                        && !tsc_error().is_match(next)
                    {
                        diag.context.push(next.trim().to_string());
                        i += 1;
                    } else {
                        break;
                    }
                }
                diagnostics.push(diag);
            } else {
                i += 1;
            }
        }

        if diagnostics.is_empty() {
            return ParseResult::Full(DiagnosticReport {
                tool: "TypeScript".to_string(),
                total_errors: 0,
                total_warnings: 0,
                files_affected: 0,
                diagnostics: Vec::new(),
                by_code: Vec::new(),
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

        let mut by_code_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for d in &diagnostics {
            *by_code_map.entry(d.code.clone()).or_insert(0) += 1;
        }
        let mut by_code: Vec<(String, usize)> = by_code_map.into_iter().collect();
        by_code.sort_by(|a, b| b.1.cmp(&a.1));

        let files_affected = {
            let mut files: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for d in &diagnostics {
                files.insert(d.file.as_str());
            }
            files.len()
        };

        ParseResult::Full(DiagnosticReport {
            tool: "TypeScript".to_string(),
            total_errors,
            total_warnings,
            files_affected,
            diagnostics,
            by_code,
        })
    }
}

/// Filter TypeScript compiler output — thin wrapper around TscParser.
#[allow(dead_code)]
pub fn filter_tsc_output(output: &str) -> String {
    let result = TscParser::parse(output);
    let mode = FormatMode::Compact;
    match result {
        ParseResult::Full(report) => report.format(mode),
        ParseResult::Degraded(report, _) => report.format(mode),
        ParseResult::Passthrough(raw) => raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_tsc_output() {
        let output = r#"
src/server/api/auth.ts(12,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/server/api/auth.ts(15,10): error TS2345: Argument of type 'number' is not assignable to parameter of type 'string'.
src/components/Button.tsx(8,3): error TS2339: Property 'onClick' does not exist on type 'ButtonProps'.
src/components/Button.tsx(10,5): error TS2322: Type 'string' is not assignable to type 'number'.

Found 4 errors in 2 files.
"#;
        let result = filter_tsc_output(output);
        assert!(result.contains("TypeScript: 4 errors in 2 files"));
        assert!(result.contains("auth.ts (2 errors)"));
        assert!(result.contains("Button.tsx (2 errors)"));
        assert!(result.contains("TS2322"));
        assert!(!result.contains("Found 4 errors")); // Summary line should be replaced
    }

    #[test]
    fn test_every_error_message_shown() {
        let output = "\
src/api.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.
src/api.ts(20,5): error TS2322: Type 'boolean' is not assignable to type 'string'.
src/api.ts(30,5): error TS2322: Type 'null' is not assignable to type 'object'.
";
        let result = filter_tsc_output(output);
        // Each error message must be individually visible, not collapsed
        assert!(result.contains("Type 'string' is not assignable to type 'number'"));
        assert!(result.contains("Type 'boolean' is not assignable to type 'string'"));
        assert!(result.contains("Type 'null' is not assignable to type 'object'"));
        assert!(result.contains("L10:"));
        assert!(result.contains("L20:"));
        assert!(result.contains("L30:"));
    }

    #[test]
    fn test_continuation_lines_preserved() {
        let output = "\
src/app.tsx(10,3): error TS2322: Type '{ children: Element; }' is not assignable to type 'Props'.
  Property 'children' does not exist on type 'Props'.
src/app.tsx(20,5): error TS2345: Argument of type 'number' is not assignable to parameter of type 'string'.
";
        let result = filter_tsc_output(output);
        assert!(result.contains("Property 'children' does not exist on type 'Props'"));
        assert!(result.contains("L10:"));
        assert!(result.contains("L20:"));
    }

    #[test]
    fn test_no_file_limit() {
        // 15 files with errors — all must appear
        let mut output = String::new();
        for i in 1..=15 {
            output.push_str(&format!(
                "src/file{}.ts({},1): error TS2322: Error in file {}.\n",
                i, i, i
            ));
        }
        let result = filter_tsc_output(&output);
        assert!(result.contains("15 errors in 15 files"));
        for i in 1..=15 {
            assert!(
                result.contains(&format!("file{}.ts", i)),
                "file{}.ts missing from output",
                i
            );
        }
    }

    #[test]
    fn test_filter_no_errors() {
        let output = "Found 0 errors. Watching for file changes.";
        let result = filter_tsc_output(output);
        assert!(result.contains("No errors found"));
    }
}
