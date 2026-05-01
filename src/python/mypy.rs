//! Mypy type checker filter that groups errors by file and error code.
use crate::parser::types::{Diagnostic, DiagnosticReport, DiagnosticSeverity};
use crate::parser::{FormatMode, OutputParser, ParseResult, TokenFormatter, truncate_output};
use crate::utils::{truncate, which_command};
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;

/// Run mypy and filter output to group errors by file and error code.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let (tool, full_args) = if which_command("mypy").is_some() {
        ("mypy".to_string(), args.to_vec())
    } else {
        let mut a = vec!["-m".to_string(), "mypy".to_string()];
        a.extend_from_slice(args);
        ("python3".to_string(), a)
    };

    crate::filtered_cmd::FilteredCommand::new(&tool)
        .args(full_args)
        .verbose(verbose)
        .strip_ansi(true)
        .filter(filter_mypy_output)
        .run()
}

struct MypyError {
    file: String,
    line: usize,
    code: String,
    message: String,
    context_lines: Vec<String>,
}

pub struct MypyParser;

/// Filter mypy output to group errors by file with error code summaries.
fn mypy_diag() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(.+?):(\d+)(?::\d+)?: (error|warning|note): (.+?)(?:\s+\[(.+)])?$")
            .expect("valid regex")
    })
}

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

impl OutputParser for MypyParser {
    type Output = DiagnosticReport;

    fn parse(output: &str) -> ParseResult<DiagnosticReport> {
        let lines: Vec<&str> = output.lines().collect();
        let mut errors: Vec<MypyError> = Vec::new();
        let mut fileless_lines: Vec<String> = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            if line.starts_with("Found ") && line.contains(" error") {
                i += 1;
                continue;
            }
            if line.starts_with("Success:") {
                i += 1;
                continue;
            }

            if let Some(caps) = mypy_diag().captures(line) {
                let severity = &caps[3];
                let file = compact_path(&caps[1]);
                let line_num: usize = caps[2].parse().unwrap_or(0);
                let message = caps[4].to_string();
                let code = caps
                    .get(5)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();

                if severity == "note" {
                    if let Some(last) = errors.last_mut()
                        && last.file == file
                    {
                        last.context_lines.push(message);
                        i += 1;
                        continue;
                    }
                    fileless_lines.push(line.to_string());
                    i += 1;
                    continue;
                }

                let mut err = MypyError {
                    file,
                    line: line_num,
                    code,
                    message,
                    context_lines: Vec::new(),
                };

                i += 1;
                while i < lines.len() {
                    if let Some(next_caps) = mypy_diag().captures(lines[i])
                        && &next_caps[3] == "note"
                        && compact_path(&next_caps[1]) == err.file
                    {
                        let note_msg = next_caps[4].to_string();
                        err.context_lines.push(note_msg);
                        i += 1;
                        continue;
                    }
                    break;
                }

                errors.push(err);
            } else if line.contains("error:") && !line.trim().is_empty() {
                fileless_lines.push(line.to_string());
                i += 1;
            } else {
                i += 1;
            }
        }

        if errors.is_empty() && fileless_lines.is_empty() {
            if output.contains("Success: no issues found") || output.contains("no issues found") {
                return ParseResult::Full(DiagnosticReport {
                    tool: "mypy".to_string(),
                    total_errors: 0,
                    total_warnings: 0,
                    files_affected: 0,
                    diagnostics: Vec::new(),
                    by_code: Vec::new(),
                    global_messages: Vec::new(),
                });
            }

            return ParseResult::Passthrough(truncate_output(output, 2000));
        }

        let diagnostics: Vec<Diagnostic> = errors
            .iter()
            .map(|err| Diagnostic {
                file: err.file.clone(),
                line: err.line,
                column: 0,
                severity: DiagnosticSeverity::Error,
                code: if err.code.is_empty() {
                    "mypy".to_string()
                } else {
                    format!("[{}]", err.code)
                },
                message: truncate(&err.message, 120),
                context: err
                    .context_lines
                    .iter()
                    .map(|line| truncate(line, 120))
                    .collect(),
            })
            .collect();

        let mut by_code: HashMap<String, usize> = HashMap::new();
        let mut files: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for err in &errors {
            if !err.code.is_empty() {
                *by_code.entry(err.code.clone()).or_insert(0) += 1;
            }
            files.insert(err.file.as_str());
        }
        let mut by_code: Vec<(String, usize)> = by_code.into_iter().collect();
        by_code.sort_by_key(|a| std::cmp::Reverse(a.1));

        ParseResult::Full(DiagnosticReport {
            tool: "mypy".to_string(),
            total_errors: diagnostics.len() + fileless_lines.len(),
            total_warnings: 0,
            files_affected: files.len(),
            diagnostics,
            by_code,
            global_messages: fileless_lines,
        })
    }
}

pub fn filter_mypy_output(output: &str) -> String {
    match MypyParser::parse(output) {
        ParseResult::Full(report) | ParseResult::Degraded(report, _) => {
            report.format(FormatMode::Compact)
        }
        ParseResult::Passthrough(raw) => raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_mypy_errors_grouped_by_file() {
        let output = "\
src/server/auth.py:12: error: Incompatible return value type (got \"str\", expected \"int\")  [return-value]
src/server/auth.py:15: error: Argument 1 has incompatible type \"int\"; expected \"str\"  [arg-type]
src/models/user.py:8: error: Name \"foo\" is not defined  [name-defined]
src/models/user.py:10: error: Incompatible types in assignment  [assignment]
src/models/user.py:20: error: Missing return statement  [return]
Found 5 errors in 2 files (checked 10 source files)
";
        let result = filter_mypy_output(output);
        assert!(result.contains("mypy: 5 errors in 2 files"));
        // user.py has 3 errors, auth.py has 2 -- user.py should come first
        let user_pos = result.find("user.py").unwrap();
        let auth_pos = result.find("auth.py").unwrap();
        assert!(
            user_pos < auth_pos,
            "user.py (3 errors) should appear before auth.py (2 errors)"
        );
        assert!(result.contains("user.py (3 errors)"));
        assert!(result.contains("auth.py (2 errors)"));
    }

    #[test]
    fn test_filter_mypy_with_column_numbers() {
        let output = "\
src/api.py:10:5: error: Incompatible return value type  [return-value]
";
        let result = filter_mypy_output(output);
        assert!(result.contains("L10:"));
        assert!(result.contains("[return-value]"));
        assert!(result.contains("Incompatible return value type"));
    }

    #[test]
    fn test_filter_mypy_top_codes_summary() {
        let output = "\
a.py:1: error: Error one  [return-value]
a.py:2: error: Error two  [return-value]
a.py:3: error: Error three  [return-value]
b.py:1: error: Error four  [name-defined]
c.py:1: error: Error five  [arg-type]
Found 5 errors in 3 files
";
        let result = filter_mypy_output(output);
        assert!(result.contains("Top codes:"));
        assert!(result.contains("return-value (3x)"));
        assert!(result.contains("name-defined (1x)"));
        assert!(result.contains("arg-type (1x)"));
    }

    #[test]
    fn test_filter_mypy_single_code_no_summary() {
        let output = "\
a.py:1: error: Error one  [return-value]
a.py:2: error: Error two  [return-value]
b.py:1: error: Error three  [return-value]
Found 3 errors in 2 files
";
        let result = filter_mypy_output(output);
        assert!(
            !result.contains("Top codes:"),
            "Top codes should not appear with only one distinct code"
        );
    }

    #[test]
    fn test_filter_mypy_every_error_shown() {
        let output = "\
src/api.py:10: error: Type \"str\" not assignable to \"int\"  [assignment]
src/api.py:20: error: Missing return statement  [return]
src/api.py:30: error: Name \"bar\" is not defined  [name-defined]
";
        let result = filter_mypy_output(output);
        assert!(result.contains("Type \"str\" not assignable to \"int\""));
        assert!(result.contains("Missing return statement"));
        assert!(result.contains("Name \"bar\" is not defined"));
        assert!(result.contains("L10:"));
        assert!(result.contains("L20:"));
        assert!(result.contains("L30:"));
    }

    #[test]
    fn test_filter_mypy_note_continuation() {
        let output = "\
src/app.py:10: error: Incompatible types in assignment  [assignment]
src/app.py:10: note: Expected type \"int\"
src/app.py:10: note: Got type \"str\"
src/app.py:20: error: Missing return statement  [return]
";
        let result = filter_mypy_output(output);
        assert!(result.contains("Incompatible types in assignment"));
        assert!(result.contains("Expected type \"int\""));
        assert!(result.contains("Got type \"str\""));
        assert!(result.contains("L10:"));
        assert!(result.contains("L20:"));
    }

    #[test]
    fn test_filter_mypy_fileless_errors() {
        let output = "\
mypy: error: No module named 'nonexistent'
src/api.py:10: error: Name \"foo\" is not defined  [name-defined]
Found 1 error in 1 file
";
        let result = filter_mypy_output(output);
        // File-less error should appear verbatim before grouped output
        assert!(result.contains("mypy: error: No module named 'nonexistent'"));
        assert!(result.contains("api.py (1 error"));
        let fileless_pos = result.find("No module named").unwrap();
        let grouped_pos = result.find("api.py").unwrap();
        assert!(
            fileless_pos < grouped_pos,
            "File-less errors should appear before grouped file errors"
        );
    }

    #[test]
    fn test_filter_mypy_no_errors() {
        let output = "Success: no issues found in 5 source files\n";
        let result = filter_mypy_output(output);
        assert_eq!(result, "✓ mypy: No issues found");
    }

    #[test]
    fn test_filter_mypy_no_file_limit() {
        let mut output = String::new();
        for i in 1..=15 {
            output.push_str(&format!(
                "src/file{}.py:{}: error: Error in file {}.  [assignment]\n",
                i, i, i
            ));
        }
        output.push_str("Found 15 errors in 15 files\n");
        let result = filter_mypy_output(&output);
        assert!(result.contains("15 errors in 15 files"));
        for i in 1..=15 {
            assert!(
                result.contains(&format!("file{}.py", i)),
                "file{}.py missing from output",
                i
            );
        }
    }
}
