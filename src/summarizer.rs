//! Heuristic summarization for large command outputs.
//!
//! Produces compact summaries of outputs above a configurable token threshold,
//! allowing users to retrieve the full output via `mycelium proxy` if needed.

use crate::tracking::utils::estimate_tokens;

pub const DEFAULT_SUMMARY_THRESHOLD_TOKENS: usize = 4000;

/// Summary of command output with token metrics.
pub struct OutputSummary {
    /// The compact summary text
    pub summary: String,
    /// Estimated tokens in original output
    pub input_tokens: usize,
    /// Estimated tokens in summary
    pub output_tokens: usize,
}

/// Summarize command output if above the token threshold.
///
/// Returns `None` if output is below the threshold (no summarization needed).
/// Returns `Some(OutputSummary)` with a compact summary if above threshold.
///
/// The summary includes:
/// - Line count and token count of original output
/// - Key stats (error count, warning count if detectable)
/// - Instruction for retrieving full output via `mycelium proxy`
pub fn summarize(
    raw: &str,
    command: &str,
    threshold_tokens: usize,
    exit_code: i32,
) -> Option<OutputSummary> {
    let input_tokens = estimate_tokens(raw);

    // Below threshold — no summarization needed
    if input_tokens < threshold_tokens {
        return None;
    }

    let summary_text = build_summary(raw, command, input_tokens, exit_code);
    let output_tokens = estimate_tokens(&summary_text);

    Some(OutputSummary {
        summary: summary_text,
        input_tokens,
        output_tokens,
    })
}

fn build_summary(raw: &str, command: &str, input_tokens: usize, exit_code: i32) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let line_count = lines.len();

    // Count errors and warnings with basic pattern matching
    let error_count = raw.lines().filter(|l| is_error_line(l)).count();
    let warning_count = raw.lines().filter(|l| is_warning_line(l)).count();

    // Detect output kind from first ~1KB
    let output_kind = detect_output_kind(raw);

    let mut result = Vec::new();

    // Header with command and stats
    result.push(format!(
        "[mycelium summary] {}: {} lines, {} tokens, kind: {}",
        command, line_count, input_tokens, output_kind
    ));

    // Key stats — use exit code as ground truth; error keyword scan is supplemental.
    if error_count > 0 {
        result.push(format!("  FAIL: {} errors", error_count));
    } else if exit_code != 0 {
        result.push(format!("  FAIL: exit code {exit_code}"));
    }
    if warning_count > 0 {
        result.push(format!("  [!] {} warnings", warning_count));
    }
    if exit_code == 0 && error_count == 0 && warning_count == 0 {
        result.push("  ok: Completed without errors".to_string());
    }

    // Instruction for full output
    result.push(String::new());
    result.push(format!(
        "[Retrieve full output: mycelium proxy {}]",
        command
    ));

    result.join("\n")
}

/// Return the largest slice of `raw` whose byte length is at most `PREFIX_LIMIT`,
/// always ending on a valid UTF-8 char boundary.
///
/// `is_char_boundary` is always true at 0 and at `raw.len()`, so this never
/// panics and the returned slice length is always `<= PREFIX_LIMIT`.
fn bounded_prefix(raw: &str) -> &str {
    const PREFIX_LIMIT: usize = 1024;
    // Walk down from PREFIX_LIMIT (or raw.len(), whichever is smaller) to find
    // the largest char boundary that does not exceed PREFIX_LIMIT bytes.
    let end = (0..=PREFIX_LIMIT.min(raw.len()))
        .rev()
        .find(|&i| raw.is_char_boundary(i))
        .unwrap_or(0);
    &raw[..end]
}

/// Classify output into one of six categories based on heuristics from the first ~1KB.
///
/// Inspects only a bounded prefix to avoid performance regression on large outputs.
/// Returns a static string label: TestResults, BuildOutput, LogOutput, ListOutput, JsonOutput, or Generic.
fn detect_output_kind(raw: &str) -> &'static str {
    let prefix = bounded_prefix(raw);

    let lower = prefix.to_lowercase();

    // JsonOutput: starts with {, [, or contains structured json-like patterns
    // Check early since it's unambiguous
    let trimmed = prefix.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return "JsonOutput";
    }

    // BuildOutput: cargo build, gcc, rustc, etc. (check before TestResults)
    // "Compiling" and "Finished" are Build-specific
    if lower.contains("compiling")
        || lower.contains("building")
        || lower.contains("finished ")
        || (lower.contains("error:")
            && (lower.contains("could not") || lower.contains("failed to")))
    {
        return "BuildOutput";
    }

    // TestResults: cargo test, pytest, npm test, etc.
    if lower.contains("test result")
        || lower.contains("tests run")
        || (lower.contains("passed") && (lower.contains("failed") || lower.contains("test")))
    {
        return "TestResults";
    }

    // LogOutput: level-prefixed log lines (not build/error related)
    if (lower.contains("info:") || lower.contains("debug:") || lower.contains("trace:"))
        || (lower.contains("warn:") && !lower.contains("warning:"))
    {
        return "LogOutput";
    }

    // ListOutput: lists of files, directories, packages, etc.
    // Look for lines that start with common list markers
    if prefix.lines().take(20).any(|line| {
        let l = line.trim_start();
        l.starts_with('-') || l.starts_with('•') || l.starts_with('*') || l.starts_with('|')
    }) {
        return "ListOutput";
    }

    // Generic: default fallback
    "Generic"
}

// "error" and "fatal" are rarely substrings of non-error words, so substring
// matching is safe. "failed" is common in test result counts ("0 failed"), so
// it uses context-aware whole-word matching to avoid false positives.
fn is_error_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("fatal") {
        return true;
    }
    for (i, _) in lower.match_indices("failed") {
        // Skip if "failed" is a suffix of another word (no word boundary before)
        if lower[..i].chars().last().is_some_and(|c| c.is_alphabetic()) {
            continue;
        }
        // Skip count context: "0 failed", "12 failed"
        if lower[..i].trim_end().ends_with(|c: char| c.is_numeric()) {
            continue;
        }
        return true;
    }
    false
}

/// Check if a line looks like a warning. Lines that also match error patterns
/// are classified as errors instead (error takes priority over warning).
///
/// Uses whole-word matching for the short form "warn" to avoid false positives
/// on words like "forward", "awkward", or "downward".
fn is_warning_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    // "warning" as a substring is unambiguous; "warn" requires a word boundary.
    let has_warn = lower.contains("warning")
        || lower
            .split(|c: char| !c.is_alphabetic())
            .any(|w| w == "warn");
    has_warn && !lower.contains("error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_returns_none_below_threshold() {
        let small_output = "hello world";
        let result = summarize(small_output, "echo", 4000, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_summarize_returns_some_above_threshold() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "test", 4000, 0);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[mycelium summary]"));
        assert!(summary.summary.contains("test"));
        assert!(summary.input_tokens >= 4000);
    }

    #[test]
    fn test_summary_contains_retrieval_notice() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "mycelium ls", 4000, 0);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[Retrieve full output:"));
        assert!(summary.summary.contains("mycelium proxy"));
    }

    #[test]
    fn test_summary_detects_errors() {
        let mut large_output = "line\n".repeat(5000);
        large_output.push_str("error: something failed\n");
        let result = summarize(&large_output, "build", 4000, 1);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("FAIL"));
    }

    #[test]
    fn test_summary_nonzero_exit_without_error_keywords() {
        // A failed command with no error lines in output should still show FAIL.
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "build", 4000, 2);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(
            summary.summary.contains("FAIL"),
            "expected FAIL for exit_code=2, got: {}",
            summary.summary
        );
        assert!(!summary.summary.contains("ok:"));
    }

    #[test]
    fn test_summary_detects_warnings() {
        let mut large_output = "line\n".repeat(5000);
        large_output.push_str("warning: be careful\n");
        let result = summarize(&large_output, "build", 4000, 0);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[!]"));
        assert!(summary.summary.contains("warnings"));
    }

    #[test]
    fn test_summary_token_counts() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "test", 4000, 0);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.input_tokens > 0);
        assert!(summary.output_tokens > 0);
        assert!(summary.input_tokens > summary.output_tokens);
    }

    #[test]
    fn test_is_error_line() {
        assert!(is_error_line("error: something"));
        assert!(is_error_line("ERROR: uppercase"));
        assert!(is_error_line("failed attempt"));
        assert!(is_error_line("fatal error"));
        assert!(!is_error_line("warning only"));
        assert!(!is_error_line("normal output"));
    }

    #[test]
    fn test_is_error_line_no_false_positives_on_test_counts() {
        assert!(!is_error_line(
            "test result: ok. 12 passed; 0 failed; 0 ignored"
        ));
        assert!(!is_error_line("5 passed, 0 failed"));
        assert!(!is_error_line("ran 30 tests, 0 failed"));
        assert!(is_error_line("FAILED: test_foo"));
        assert!(is_error_line("failed to compile"));
    }

    #[test]
    fn test_is_warning_line() {
        assert!(is_warning_line("warning: be careful"));
        assert!(is_warning_line("WARNING: uppercase"));
        assert!(is_warning_line("warn: short form"));
        assert!(!is_warning_line("error: not a warning"));
        assert!(!is_warning_line("normal output"));
    }

    #[test]
    fn test_is_warning_line_no_false_positives() {
        assert!(!is_warning_line("forward pass complete"));
        assert!(!is_warning_line("downward trend"));
        assert!(!is_warning_line("awkward silence"));
    }

    #[test]
    fn test_mixed_error_warning_classified_as_error() {
        assert!(is_error_line("warning: deprecated, may cause error"));
        assert!(!is_warning_line("warning: deprecated, may cause error"));
    }

    #[test]
    fn test_detect_output_kind_test_results() {
        let test_output = "test result: ok. 12 passed; 0 failed; 0 ignored";
        assert_eq!(detect_output_kind(test_output), "TestResults");

        let test_output2 = "running 15 tests\n...\ntest result: ok";
        assert_eq!(detect_output_kind(test_output2), "TestResults");

        let test_output3 = "running tests...\n3 passed, 0 failed";
        assert_eq!(detect_output_kind(test_output3), "TestResults");
    }

    #[test]
    fn test_detect_output_kind_build_output() {
        let build_output = "Compiling mycelium v0.1.0\nFinished `release` profile [optimized]";
        assert_eq!(detect_output_kind(build_output), "BuildOutput");

        let build_output2 = "error: failed to compile\nwarning: unused variable";
        assert_eq!(detect_output_kind(build_output2), "BuildOutput");

        let build_output3 = "Building...\nFinished successfully";
        assert_eq!(detect_output_kind(build_output3), "BuildOutput");
    }

    #[test]
    fn test_detect_output_kind_log_output() {
        let log_output = "TRACE: entering function\nDEBUG: variable x = 42";
        assert_eq!(detect_output_kind(log_output), "LogOutput");

        let log_output2 = "INFO: Server started\nDEBUG: Loaded config";
        assert_eq!(detect_output_kind(log_output2), "LogOutput");

        let log_output3 = "WARN: deprecated API\nERROR: something failed";
        assert_eq!(detect_output_kind(log_output3), "LogOutput");
    }

    #[test]
    fn test_detect_output_kind_json_output() {
        let json_output = r#"{"status": "ok", "data": [1, 2, 3]}"#;
        assert_eq!(detect_output_kind(json_output), "JsonOutput");

        let json_output2 = "[{\"name\": \"alice\"}, {\"name\": \"bob\"}]";
        assert_eq!(detect_output_kind(json_output2), "JsonOutput");

        let json_output3 = "  \n  {\n\"key\": \"value\"\n}";
        assert_eq!(detect_output_kind(json_output3), "JsonOutput");
    }

    #[test]
    fn test_detect_output_kind_list_output() {
        let list_output = "- file1.txt\n- file2.txt\n- file3.txt";
        assert_eq!(detect_output_kind(list_output), "ListOutput");

        let list_output2 = "• item 1\n• item 2\n• item 3";
        assert_eq!(detect_output_kind(list_output2), "ListOutput");

        let list_output3 = "| name  | version |\n| pkg1  | 1.0.0   |\n| pkg2  | 2.0.0   |";
        assert_eq!(detect_output_kind(list_output3), "ListOutput");

        let list_output4 = "* first\n* second\n* third";
        assert_eq!(detect_output_kind(list_output4), "ListOutput");
    }

    #[test]
    fn test_detect_output_kind_generic() {
        // Empty input should return Generic
        assert_eq!(detect_output_kind(""), "Generic");

        // Plain text with no markers should return Generic
        let plain_text = "This is just plain output without any special patterns";
        assert_eq!(detect_output_kind(plain_text), "Generic");

        // Short non-diagnostic text
        let short_text = "hello world\nfoo bar\nbaz qux";
        assert_eq!(detect_output_kind(short_text), "Generic");
    }

    #[test]
    fn test_detect_output_kind_large_input_respects_limit() {
        // Create an output larger than 1KB where the kind marker is beyond 1KB
        let large_input = "normal output\n".repeat(100); // >1KB
        let mut input_with_marker = large_input.clone();
        input_with_marker.push_str("test result: ok"); // marker at end, beyond 1KB

        // Should return Generic because the marker is outside the prefix
        assert_eq!(detect_output_kind(&input_with_marker), "Generic");
    }

    #[test]
    fn test_detect_output_kind_within_limit() {
        // Create output where kind marker is within first 1KB
        let mut input = String::new();
        input.push_str("test result: ok\n");
        for _ in 0..50 {
            input.push_str("normal output\n"); // keep total under 1KB
        }

        assert_eq!(detect_output_kind(&input), "TestResults");
    }

    #[test]
    fn test_detect_output_kind_multibyte_boundary_no_panic() {
        // 'é' is 2 bytes in UTF-8, so 600 copies = 1200 bytes (> PREFIX_LIMIT
        // of 1024). The 1024 cutoff lands mid-codepoint, which would panic on a
        // raw byte slice. detect_output_kind must classify without panicking.
        let multibyte = "\u{00e9}".repeat(600);
        assert_eq!(multibyte.len(), 1200);
        // Should return without panicking; content has no markers, so Generic.
        assert_eq!(detect_output_kind(&multibyte), "Generic");
    }

    #[test]
    fn test_bounded_prefix_length_never_exceeds_limit() {
        // '\u{00e9}' ('é') is 2 bytes in UTF-8.
        // 2000 copies = 4000 bytes, well over PREFIX_LIMIT (1024).
        // Byte index 1024 falls mid-codepoint (odd offset into 2-byte sequences),
        // so the old get(..1024).unwrap_or(raw) path would have fallen back to the
        // full 4000-byte buffer. bounded_prefix must return a slice of <= 1024 bytes.
        let huge = "\u{00e9}".repeat(2000);
        assert_eq!(huge.len(), 4000);
        let prefix = bounded_prefix(&huge);
        assert!(
            prefix.len() <= 1024,
            "bounded_prefix returned {} bytes, expected <= 1024",
            prefix.len()
        );
        // The result must still be valid UTF-8 (Rust &str invariant is enforced
        // by the type, but we verify the boundary is sound via char count).
        assert_eq!(prefix.chars().count(), prefix.len() / 2);
    }
}
