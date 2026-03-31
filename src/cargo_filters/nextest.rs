use std::sync::OnceLock;

/// Push a completed failure block (header + body) into the failures list, then clear the buffers.
fn flush_failure_block(header: &mut String, body: &mut Vec<String>, failures: &mut Vec<String>) {
    if header.is_empty() {
        return;
    }
    let mut block = header.clone();
    if !body.is_empty() {
        block.push('\n');
        block.push_str(&body.join("\n"));
    }
    failures.push(block);
    header.clear();
    body.clear();
}

/// Filter cargo nextest output - show failures + compact summary
pub(crate) fn filter_cargo_nextest(output: &str) -> String {
    static SUMMARY_RE: OnceLock<regex::Regex> = OnceLock::new();
    let summary_re = SUMMARY_RE.get_or_init(|| {
        regex::Regex::new(
            r"Summary \[\s*([\d.]+)s]\s+(\d+) tests? run:\s+(\d+) passed(?:,\s+(\d+) failed)?(?:,\s+(\d+) skipped)?"
        ).expect("invalid nextest summary regex")
    });

    static STARTING_RE: OnceLock<regex::Regex> = OnceLock::new();
    let starting_re = STARTING_RE.get_or_init(|| {
        regex::Regex::new(r"Starting \d+ tests? across (\d+) binar(?:y|ies)")
            .expect("invalid nextest starting regex")
    });

    let mut failures: Vec<String> = Vec::new();
    let mut in_failure_block = false;
    let mut past_summary = false;
    let mut current_failure_header = String::new();
    let mut current_failure_body = Vec::new();
    let mut summary_line = String::new();
    let mut binaries: u32 = 0;
    let mut has_cancel_line = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // Strip compilation noise
        if trimmed.starts_with("Compiling")
            || trimmed.starts_with("Downloading")
            || trimmed.starts_with("Downloaded")
            || trimmed.starts_with("Finished")
            || trimmed.starts_with("Locking")
            || trimmed.starts_with("Updating")
        {
            continue;
        }

        // Strip separator lines (────)
        if trimmed.starts_with("────") {
            continue;
        }

        // Skip post-summary recap lines (FAIL duplicates + "error: test run failed")
        if past_summary {
            continue;
        }

        // Parse binary count from Starting line
        if trimmed.starts_with("Starting") {
            if let Some(caps) = starting_re.captures(trimmed)
                && let Some(m) = caps.get(1)
            {
                binaries = m.as_str().parse().unwrap_or(0);
            }
            continue;
        }

        // Strip PASS lines
        if trimmed.starts_with("PASS") {
            if in_failure_block {
                flush_failure_block(
                    &mut current_failure_header,
                    &mut current_failure_body,
                    &mut failures,
                );
                in_failure_block = false;
            }
            continue;
        }

        // Detect FAIL lines
        if trimmed.starts_with("FAIL") {
            // Close previous failure block if any
            if in_failure_block {
                flush_failure_block(
                    &mut current_failure_header,
                    &mut current_failure_body,
                    &mut failures,
                );
            }
            current_failure_header = trimmed.to_string();
            in_failure_block = true;
            continue;
        }

        // Cancellation notice
        if trimmed.starts_with("Cancelling") || trimmed.starts_with("Canceling") {
            has_cancel_line = true;
            continue;
        }

        // Nextest run ID line
        if trimmed.starts_with("Nextest run ID") {
            continue;
        }

        // Parse summary
        if trimmed.starts_with("Summary") {
            summary_line = trimmed.to_string();
            if in_failure_block {
                flush_failure_block(
                    &mut current_failure_header,
                    &mut current_failure_body,
                    &mut failures,
                );
                in_failure_block = false;
            }
            past_summary = true;
            continue;
        }

        // Collect failure body lines (stdout/stderr sections)
        if in_failure_block {
            current_failure_body.push(line.to_string());
        }
    }

    // Close last failure block
    if in_failure_block {
        flush_failure_block(
            &mut current_failure_header,
            &mut current_failure_body,
            &mut failures,
        );
    }

    // Parse summary with regex
    if let Some(caps) = summary_re.captures(&summary_line) {
        let duration = caps.get(1).map_or("?", |m| m.as_str());
        let passed: u32 = caps
            .get(3)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let failed: u32 = caps
            .get(4)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let skipped: u32 = caps
            .get(5)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        let binary_text = if binaries == 1 {
            "1 binary".to_string()
        } else if binaries > 1 {
            format!("{} binaries", binaries)
        } else {
            String::new()
        };

        if failed == 0 {
            // All pass - compact single line
            let mut parts = vec![format!("{} passed", passed)];
            if skipped > 0 {
                parts.push(format!("{} skipped", skipped));
            }
            let meta = if binary_text.is_empty() {
                format!("{}s", duration)
            } else {
                format!("{}, {}s", binary_text, duration)
            };
            return format!("✓ cargo nextest: {} ({})", parts.join(", "), meta);
        }

        // With failures - show failure details then summary
        let mut result = String::new();

        for failure in &failures {
            result.push_str(failure);
            result.push('\n');
        }

        if has_cancel_line {
            result.push_str("Cancelling due to test failure\n");
        }

        let mut summary_parts = vec![format!("{} passed", passed)];
        if failed > 0 {
            summary_parts.push(format!("{} failed", failed));
        }
        if skipped > 0 {
            summary_parts.push(format!("{} skipped", skipped));
        }
        let meta = if binary_text.is_empty() {
            format!("{}s", duration)
        } else {
            format!("{}, {}s", binary_text, duration)
        };
        result.push_str(&format!(
            "cargo nextest: {} ({})",
            summary_parts.join(", "),
            meta
        ));

        return result.trim().to_string();
    }

    // Fallback: if summary regex didn't match, show what we have
    if !failures.is_empty() {
        let mut result = String::new();
        for failure in &failures {
            result.push_str(failure);
            result.push('\n');
        }
        if !summary_line.is_empty() {
            result.push_str(&summary_line);
        }
        return result.trim().to_string();
    }

    if !summary_line.is_empty() {
        return summary_line;
    }

    // Empty or unrecognized
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cargo_nextest_all_pass() {
        let output = r#"   Compiling mycelium v0.15.2
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
────────────────────────────
    Starting 301 tests across 1 binary
        PASS [   0.009s] (1/301) mycelium::bin/mycelium cargo_cmd::tests::test_one
        PASS [   0.008s] (2/301) mycelium::bin/mycelium cargo_cmd::tests::test_two
        PASS [   0.007s] (301/301) mycelium::bin/mycelium cargo_cmd::tests::test_last
────────────────────────────
     Summary [   0.192s] 301 tests run: 301 passed, 0 skipped
"#;
        let result = filter_cargo_nextest(output);
        assert_eq!(
            result, "✓ cargo nextest: 301 passed (1 binary, 0.192s)",
            "got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_with_failures() {
        let output = r#"    Starting 4 tests across 1 binary (1 test skipped)
        PASS [   0.006s] (1/4) test-proj tests::passing_test
        FAIL [   0.006s] (2/4) test-proj tests::failing_test

  stderr ───

    thread 'tests::failing_test' panicked at src/lib.rs:15:9:
    assertion `left == right` failed
      left: 1
     right: 2

  Cancelling due to test failure: 2 tests still running
        PASS [   0.007s] (3/4) test-proj tests::another_passing
        FAIL [   0.006s] (4/4) test-proj tests::another_failing

  stderr ───

    thread 'tests::another_failing' panicked at src/lib.rs:20:9:
    something went wrong

────────────────────────────
     Summary [   0.007s] 4 tests run: 2 passed, 2 failed, 1 skipped
        FAIL [   0.006s] (2/4) test-proj tests::failing_test
        FAIL [   0.006s] (4/4) test-proj tests::another_failing
error: test run failed
"#;
        let result = filter_cargo_nextest(output);
        assert!(
            result.contains("tests::failing_test"),
            "should contain first failure: {}",
            result
        );
        assert!(
            result.contains("tests::another_failing"),
            "should contain second failure: {}",
            result
        );
        assert!(
            result.contains("panicked"),
            "should contain stderr detail: {}",
            result
        );
        assert!(
            result.contains("2 passed, 2 failed, 1 skipped"),
            "should contain summary: {}",
            result
        );
        assert!(
            !result.contains("PASS"),
            "should not contain PASS lines: {}",
            result
        );
        // Post-summary FAIL recaps must not create duplicate FAIL header entries
        // (test names may appear in both header and stderr body naturally)
        assert_eq!(
            result.matches("FAIL [").count(),
            2,
            "should have exactly 2 FAIL headers (no post-summary duplicates): {}",
            result
        );
        assert!(
            !result.contains("error: test run failed"),
            "should not contain post-summary error line: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_with_skipped() {
        let output = r#"    Starting 50 tests across 2 binaries (3 tests skipped)
        PASS [   0.010s] (1/50) mycelium::bin/mycelium test_one
        PASS [   0.010s] (50/50) mycelium::bin/mycelium test_last
────────────────────────────
     Summary [   0.500s] 50 tests run: 50 passed, 3 skipped
"#;
        let result = filter_cargo_nextest(output);
        assert_eq!(
            result, "✓ cargo nextest: 50 passed, 3 skipped (2 binaries, 0.500s)",
            "got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_single_failure_detail() {
        let output = r#"    Starting 2 tests across 1 binary
        PASS [   0.005s] (1/2) proj tests::good
        FAIL [   0.005s] (2/2) proj tests::bad

  stderr ───

    thread 'tests::bad' panicked at src/lib.rs:5:9:
    assertion failed: false

────────────────────────────
     Summary [   0.010s] 2 tests run: 1 passed, 1 failed
        FAIL [   0.005s] (2/2) proj tests::bad
error: test run failed
"#;
        let result = filter_cargo_nextest(output);
        assert!(
            result.contains("assertion failed: false"),
            "should show panic message: {}",
            result
        );
        assert!(
            result.contains("1 passed, 1 failed"),
            "should show summary: {}",
            result
        );
        // Post-summary recap must not duplicate FAIL headers
        assert_eq!(
            result.matches("FAIL [").count(),
            1,
            "should have exactly 1 FAIL header (no post-summary duplicate): {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_multiple_binaries() {
        let output = r#"    Starting 100 tests across 5 binaries
        PASS [   0.010s] (100/100) test_last
────────────────────────────
     Summary [   1.234s] 100 tests run: 100 passed, 0 skipped
"#;
        let result = filter_cargo_nextest(output);
        assert_eq!(
            result, "✓ cargo nextest: 100 passed (5 binaries, 1.234s)",
            "got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_compilation_stripped() {
        let output = r#"   Compiling serde v1.0.200
   Compiling mycelium v0.15.2
   Downloading crates ...
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.00s
────────────────────────────
    Starting 10 tests across 1 binary
        PASS [   0.010s] (10/10) test_last
────────────────────────────
     Summary [   0.050s] 10 tests run: 10 passed, 0 skipped
"#;
        let result = filter_cargo_nextest(output);
        assert!(
            !result.contains("Compiling"),
            "should strip Compiling: {}",
            result
        );
        assert!(
            !result.contains("Downloading"),
            "should strip Downloading: {}",
            result
        );
        assert!(
            !result.contains("Finished"),
            "should strip Finished: {}",
            result
        );
        assert!(
            result.contains("✓ cargo nextest: 10 passed"),
            "got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_empty() {
        let result = filter_cargo_nextest("");
        assert!(result.is_empty(), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_nextest_cancellation_notice() {
        let output = r#"    Starting 3 tests across 1 binary
        FAIL [   0.005s] (1/3) proj tests::bad

  stderr ───

    thread panicked at 'oops'

  Cancelling due to test failure: 2 tests still running
────────────────────────────
     Summary [   0.010s] 3 tests run: 2 passed, 1 failed
        FAIL [   0.005s] (1/3) proj tests::bad
error: test run failed
"#;
        let result = filter_cargo_nextest(output);
        assert!(
            result.contains("Cancelling due to test failure"),
            "should include cancel notice: {}",
            result
        );
        assert!(
            result.contains("1 failed"),
            "should show failure count: {}",
            result
        );
        // Post-summary recap must not duplicate FAIL headers
        assert_eq!(
            result.matches("FAIL [").count(),
            1,
            "should have exactly 1 FAIL header (no post-summary duplicate): {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_nextest_summary_regex_fallback() {
        let output = r#"    Starting 5 tests across 1 binary
        PASS [   0.005s] (5/5) test_last
────────────────────────────
     Summary MALFORMED LINE
"#;
        let result = filter_cargo_nextest(output);
        assert!(
            result.contains("Summary MALFORMED"),
            "should fall back to raw summary: {}",
            result
        );
    }

    fn count_tokens(text: &str) -> usize {
        crate::tracking::estimate_tokens(text)
    }

    #[test]
    fn test_cargo_nextest_token_savings() {
        let input = include_str!("../../tests/fixtures/cargo_nextest_raw.txt");
        let output = filter_cargo_nextest(input);
        let input_tokens = count_tokens(input);
        let output_tokens = count_tokens(&output);
        let savings = if input_tokens > 0 {
            (input_tokens.saturating_sub(output_tokens)) * 100 / input_tokens
        } else {
            0
        };
        assert!(
            savings >= 60,
            "Expected >= 60% token savings, got {}% ({} -> {} tokens)",
            savings,
            input_tokens,
            output_tokens
        );
    }
}
