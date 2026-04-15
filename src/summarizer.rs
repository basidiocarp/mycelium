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
pub fn summarize(raw: &str, command: &str, threshold_tokens: usize) -> Option<OutputSummary> {
    let input_tokens = estimate_tokens(raw);

    // Below threshold — no summarization needed
    if input_tokens < threshold_tokens {
        return None;
    }

    let summary_text = build_summary(raw, command, input_tokens);
    let output_tokens = estimate_tokens(&summary_text);

    Some(OutputSummary {
        summary: summary_text,
        input_tokens,
        output_tokens,
    })
}

fn build_summary(raw: &str, command: &str, input_tokens: usize) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let line_count = lines.len();

    // Count errors and warnings with basic pattern matching
    let error_count = raw.lines().filter(|l| is_error_line(l)).count();
    let warning_count = raw.lines().filter(|l| is_warning_line(l)).count();

    let mut result = Vec::new();

    // Header with command and stats
    result.push(format!(
        "[mycelium summary] {}: {} lines, {} tokens",
        command, line_count, input_tokens
    ));

    // Key stats
    if error_count > 0 {
        result.push(format!("  FAIL: {} errors", error_count));
    }
    if warning_count > 0 {
        result.push(format!("  [!] {} warnings", warning_count));
    }
    if error_count == 0 && warning_count == 0 {
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

fn is_error_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("error") || lower.contains("failed") || lower.contains("fatal")
}

/// Check if a line looks like a warning. Lines that also match error patterns
/// are classified as errors instead (error takes priority over warning).
fn is_warning_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    (lower.contains("warning") || lower.contains("warn")) && !lower.contains("error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_returns_none_below_threshold() {
        let small_output = "hello world";
        let result = summarize(small_output, "echo", 4000);
        assert!(result.is_none());
    }

    #[test]
    fn test_summarize_returns_some_above_threshold() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "test", 4000);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[mycelium summary]"));
        assert!(summary.summary.contains("test"));
        assert!(summary.input_tokens >= 4000);
    }

    #[test]
    fn test_summary_contains_retrieval_notice() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "mycelium ls", 4000);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[Retrieve full output:"));
        assert!(summary.summary.contains("mycelium proxy"));
    }

    #[test]
    fn test_summary_detects_errors() {
        let mut large_output = "line\n".repeat(5000);
        large_output.push_str("error: something failed\n");
        let result = summarize(&large_output, "build", 4000);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("FAIL"));
    }

    #[test]
    fn test_summary_detects_warnings() {
        let mut large_output = "line\n".repeat(5000);
        large_output.push_str("warning: be careful\n");
        let result = summarize(&large_output, "build", 4000);
        assert!(result.is_some());

        let summary = result.unwrap();
        assert!(summary.summary.contains("[!]"));
        assert!(summary.summary.contains("warnings"));
    }

    #[test]
    fn test_summary_token_counts() {
        let large_output = "line\n".repeat(5000);
        let result = summarize(&large_output, "test", 4000);
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
    fn test_is_warning_line() {
        assert!(is_warning_line("warning: be careful"));
        assert!(is_warning_line("WARNING: uppercase"));
        assert!(is_warning_line("warn: short form"));
        assert!(!is_warning_line("error: not a warning"));
        assert!(!is_warning_line("normal output"));
    }

    #[test]
    fn test_mixed_error_warning_classified_as_error() {
        assert!(is_error_line("warning: deprecated, may cause error"));
        assert!(!is_warning_line("warning: deprecated, may cause error"));
    }
}
