//! Token-optimized filter for linters (ESLint, Biome, Ruff, Mypy) with grouped rule violations.
pub mod eslint;
pub mod pylint;

use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter, truncate_output};
use crate::python::mypy as mypy_cmd;
use crate::python::ruff as ruff_cmd;
use crate::tracking;
use crate::utils::package_manager_exec;
use anyhow::{Context, Result};
use std::process::Command;

pub use eslint::filter_eslint_json;
pub use pylint::{filter_generic_lint, filter_pylint_json};

struct LintAnalysis {
    filtered: String,
    parse_tier: u8,
}

struct GenericLintParser;

fn has_explicit_output_format(args: &[String]) -> bool {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--output-format" {
            if iter.peek().is_some() {
                return true;
            }
            continue;
        }
        if arg.starts_with("--output-format=") {
            return true;
        }
    }
    false
}

fn should_skip_output_format_arg(linter: &str, args: &[String], idx: usize) -> bool {
    if !matches!(linter, "ruff" | "pylint") {
        return false;
    }

    let arg = &args[idx];
    if arg.starts_with("--output-format=") {
        return true;
    }

    if arg == "--output-format" {
        return true;
    }

    idx > 0 && args[idx - 1] == "--output-format"
}

/// Compact file path (remove common prefixes)
pub(super) fn compact_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    if let Some(pos) = path.rfind("/src/") {
        format!("src/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind("/lib/") {
        format!("lib/{}", &path[pos + 5..])
    } else if let Some(pos) = path.rfind('/') {
        path[pos + 1..].to_string()
    } else {
        path
    }
}

/// Check if a linter is Python-based (uses pip/pipx, not npm/pnpm)
fn is_python_linter(linter: &str) -> bool {
    matches!(linter, "ruff" | "pylint" | "mypy" | "flake8")
}

/// Strip package manager prefixes (npx, bunx, pnpm, pnpm exec, yarn) from args.
/// Returns the number of args to skip.
fn strip_pm_prefix(args: &[String]) -> usize {
    let pm_names = ["npx", "bunx", "pnpm", "yarn"];
    let mut skip = 0;
    for arg in args {
        if pm_names.contains(&arg.as_str()) || arg == "exec" {
            skip += 1;
        } else {
            break;
        }
    }
    skip
}

/// Detect the linter name from args (after stripping PM prefixes).
/// Returns the linter name and whether it was explicitly specified.
fn detect_linter(args: &[String]) -> (&str, bool) {
    let is_path_or_flag = args.is_empty()
        || args[0].starts_with('-')
        || args[0].contains('/')
        || args[0].contains('.');

    if is_path_or_flag {
        ("eslint", false)
    } else {
        (&args[0], true)
    }
}

/// Run a linter (ESLint, Ruff, Pylint, Mypy, or others) with grouped, token-compressed output.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let skip = strip_pm_prefix(args);
    let effective_args = &args[skip..];

    let (linter, explicit) = detect_linter(effective_args);

    // Python linters use Command::new() directly (they're on PATH via pip/pipx)
    // JS linters use package_manager_exec (npx/pnpm exec)
    let mut cmd = if is_python_linter(linter) {
        Command::new(linter)
    } else {
        package_manager_exec(linter)
    };

    // Add format flags based on linter
    match linter {
        "eslint" => {
            cmd.arg("-f").arg("json");
        }
        "ruff" => {
            // Force JSON output for ruff check
            if !has_explicit_output_format(effective_args) {
                cmd.arg("check").arg("--output-format=json");
            }
        }
        "pylint" => {
            // Force JSON2 output for pylint
            if !has_explicit_output_format(effective_args) {
                cmd.arg("--output-format=json2");
            }
        }
        "mypy" => {
            // mypy uses default text output (no special flags)
        }
        _ => {
            // Other linters: no special formatting
        }
    }

    // Add user arguments (skip first if it was the linter name, and skip "check" for ruff if we added it)
    let start_idx = if !explicit {
        0
    } else if linter == "ruff" && !effective_args.is_empty() && effective_args[0] == "ruff" {
        // Skip "ruff" and "check" if we already added "check"
        if effective_args.len() > 1 && effective_args[1] == "check" {
            2
        } else {
            1
        }
    } else {
        1
    };

    for (offset, arg) in effective_args[start_idx..].iter().enumerate() {
        if should_skip_output_format_arg(linter, effective_args, start_idx + offset) {
            continue;
        }
        cmd.arg(arg);
    }

    // Default to current directory if no path specified (for ruff/pylint/mypy/eslint)
    if matches!(linter, "ruff" | "pylint" | "mypy" | "eslint") {
        let has_path = effective_args
            .iter()
            .skip(start_idx)
            .enumerate()
            .any(|(offset, arg)| {
                !should_skip_output_format_arg(linter, effective_args, start_idx + offset)
                    && !arg.starts_with('-')
                    && !arg.contains('=')
            });
        if !has_path {
            cmd.arg(".");
        }
    }

    if verbose > 0 {
        eprintln!("Running: {} with structured output", linter);
    }

    let output = cmd.output().context(format!(
        "Failed to run {}. Is it installed? Try: pip install {} (or npm/pnpm for JS linters)",
        linter, linter
    ))?;

    // Check if process was killed by signal (SIGABRT, SIGKILL, etc.)
    if !output.status.success() && output.status.code().is_none() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[!] Linter process terminated abnormally (possibly out of memory)");
        if !stderr.is_empty() {
            eprintln!(
                "stderr: {}",
                stderr.lines().take(5).collect::<Vec<_>>().join("\n")
            );
        }
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let analysis = analyze_lint_output(linter, &stdout, &raw);
    let filtered = analysis.filtered;

    let exit_code = output
        .status
        .code()
        .unwrap_or(if output.status.success() { 0 } else { 1 });
    if let Some(hint) = crate::tee::tee_and_hint(&raw, "lint", exit_code) {
        println!("{}\n{}", filtered, hint);
    } else {
        println!("{}", filtered);
    }

    timer.track_with_parse_info(
        &format!("{} {}", linter, args.join(" ")),
        &format!("mycelium lint {} {}", linter, args.join(" ")),
        &raw,
        &filtered,
        analysis.parse_tier,
        "compact",
    );

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

fn analyze_lint_output(linter: &str, stdout: &str, raw: &str) -> LintAnalysis {
    match linter {
        "eslint" => match eslint::EslintParser::parse(stdout) {
            ParseResult::Full(_) => LintAnalysis {
                filtered: filter_eslint_json(stdout),
                parse_tier: 1,
            },
            ParseResult::Degraded(_, _) => LintAnalysis {
                filtered: filter_eslint_json(stdout),
                parse_tier: 2,
            },
            ParseResult::Passthrough(_) => LintAnalysis {
                filtered: filter_eslint_json(stdout),
                parse_tier: 3,
            },
        },
        "ruff" => match ruff_cmd::RuffCheckParser::parse(stdout) {
            ParseResult::Full(report) => LintAnalysis {
                filtered: report.format(FormatMode::Compact),
                parse_tier: 1,
            },
            ParseResult::Degraded(report, _) => LintAnalysis {
                filtered: report.format(FormatMode::Compact),
                parse_tier: 2,
            },
            ParseResult::Passthrough(_) => LintAnalysis {
                filtered: ruff_cmd::filter_ruff_check_json(stdout),
                parse_tier: 3,
            },
        },
        "pylint" => match pylint::PylintParser::parse(stdout) {
            ParseResult::Full(_) => LintAnalysis {
                filtered: filter_pylint_json(stdout),
                parse_tier: 1,
            },
            ParseResult::Degraded(_, _) => LintAnalysis {
                filtered: filter_pylint_json(stdout),
                parse_tier: 2,
            },
            ParseResult::Passthrough(_) => LintAnalysis {
                filtered: filter_pylint_json(stdout),
                parse_tier: 3,
            },
        },
        "mypy" => match mypy_cmd::MypyParser::parse(raw) {
            ParseResult::Full(report) => LintAnalysis {
                filtered: report.format(FormatMode::Compact),
                parse_tier: 1,
            },
            ParseResult::Degraded(report, _) => LintAnalysis {
                filtered: report.format(FormatMode::Compact),
                parse_tier: 2,
            },
            ParseResult::Passthrough(raw_out) => LintAnalysis {
                filtered: raw_out,
                parse_tier: 3,
            },
        },
        _ => match GenericLintParser::parse(raw) {
            ParseResult::Full(_) => LintAnalysis {
                filtered: filter_generic_lint(raw),
                parse_tier: 1,
            },
            ParseResult::Degraded(_, _) => LintAnalysis {
                filtered: filter_generic_lint(raw),
                parse_tier: 2,
            },
            ParseResult::Passthrough(raw_out) => LintAnalysis {
                filtered: raw_out,
                parse_tier: 3,
            },
        },
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_path() {
        assert_eq!(
            compact_path("/Users/foo/project/src/utils.ts"),
            "src/utils.ts"
        );
        assert_eq!(
            compact_path("C:\\Users\\project\\src\\api.ts"),
            "src/api.ts"
        );
        assert_eq!(compact_path("simple.ts"), "simple.ts");
    }

    #[test]
    fn test_strip_pm_prefix_npx() {
        let args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
        assert_eq!(strip_pm_prefix(&args), 1);
    }

    #[test]
    fn test_strip_pm_prefix_bunx() {
        let args: Vec<String> = vec!["bunx".into(), "eslint".into(), ".".into()];
        assert_eq!(strip_pm_prefix(&args), 1);
    }

    #[test]
    fn test_strip_pm_prefix_pnpm_exec() {
        let args: Vec<String> = vec!["pnpm".into(), "exec".into(), "eslint".into()];
        assert_eq!(strip_pm_prefix(&args), 2);
    }

    #[test]
    fn test_strip_pm_prefix_none() {
        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
        assert_eq!(strip_pm_prefix(&args), 0);
    }

    #[test]
    fn test_strip_pm_prefix_empty() {
        let args: Vec<String> = vec![];
        assert_eq!(strip_pm_prefix(&args), 0);
    }

    #[test]
    fn test_detect_linter_eslint() {
        let args: Vec<String> = vec!["eslint".into(), "src/".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(explicit);
    }

    #[test]
    fn test_detect_linter_default_on_path() {
        let args: Vec<String> = vec!["src/".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(!explicit);
    }

    #[test]
    fn test_detect_linter_default_on_flag() {
        let args: Vec<String> = vec!["--max-warnings=0".into()];
        let (linter, explicit) = detect_linter(&args);
        assert_eq!(linter, "eslint");
        assert!(!explicit);
    }

    #[test]
    fn test_detect_linter_after_npx_strip() {
        // Simulates: mycelium lint npx eslint src/ → after strip_pm_prefix, args = ["eslint", "src/"]
        let full_args: Vec<String> = vec!["npx".into(), "eslint".into(), "src/".into()];
        let skip = strip_pm_prefix(&full_args);
        let effective = &full_args[skip..];
        let (linter, _) = detect_linter(effective);
        assert_eq!(linter, "eslint");
    }

    #[test]
    fn test_detect_linter_after_pnpm_exec_strip() {
        let full_args: Vec<String> =
            vec!["pnpm".into(), "exec".into(), "biome".into(), "check".into()];
        let skip = strip_pm_prefix(&full_args);
        let effective = &full_args[skip..];
        let (linter, _) = detect_linter(effective);
        assert_eq!(linter, "biome");
    }

    #[test]
    fn test_is_python_linter() {
        assert!(is_python_linter("ruff"));
        assert!(is_python_linter("pylint"));
        assert!(is_python_linter("mypy"));
        assert!(is_python_linter("flake8"));
        assert!(!is_python_linter("eslint"));
        assert!(!is_python_linter("biome"));
        assert!(!is_python_linter("unknown"));
    }

    #[test]
    fn test_analyze_lint_output_eslint_passthroughs_invalid_json() {
        let analysis = analyze_lint_output("eslint", "{not-json}", "{not-json}");
        assert_eq!(analysis.parse_tier, 3);
    }

    #[test]
    fn test_analyze_lint_output_generic_lint_is_degraded() {
        let analysis = analyze_lint_output(
            "biome",
            "src/app.ts:1:1 error: unexpected thing\nsrc/app.ts:2:1 warning: style",
            "src/app.ts:1:1 error: unexpected thing\nsrc/app.ts:2:1 warning: style",
        );
        assert_eq!(analysis.parse_tier, 2);
        assert!(analysis.filtered.contains("Lint: 1 errors, 1 warnings"));
    }

    #[test]
    fn test_has_explicit_output_format_supports_split_and_equals_forms() {
        assert!(has_explicit_output_format(&[
            "--output-format".into(),
            "json".into()
        ]));
        assert!(has_explicit_output_format(&["--output-format=json".into()]));
        assert!(!has_explicit_output_format(&["--fix".into(), ".".into()]));
    }

    #[test]
    fn test_skip_output_format_arg_consumes_split_value_token() {
        let args = vec![
            "ruff".into(),
            "check".into(),
            "--output-format".into(),
            "json".into(),
            ".".into(),
        ];
        assert!(should_skip_output_format_arg("ruff", &args, 2));
        assert!(should_skip_output_format_arg("ruff", &args, 3));
        assert!(!should_skip_output_format_arg("ruff", &args, 4));
    }

    #[test]
    fn test_split_output_format_value_is_not_treated_as_path() {
        let effective_args = vec!["pylint".into(), "--output-format".into(), "json2".into()];
        let start_idx = 1;
        let has_path = effective_args
            .iter()
            .skip(start_idx)
            .enumerate()
            .any(|(offset, arg)| {
                !should_skip_output_format_arg("pylint", &effective_args, start_idx + offset)
                    && !arg.starts_with('-')
                    && !arg.contains('=')
            });
        assert!(!has_path);
    }
}
