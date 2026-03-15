use std::sync::OnceLock;

fn collapse_internal_frames(text: &str) -> String {
    let mut result = Vec::new();
    let mut internal_count = 0usize;
    for line in text.lines() {
        let is_internal =
            line.contains("/rustc/") || line.contains("std::") || line.contains("<core::");
        if is_internal {
            internal_count += 1;
        } else {
            if internal_count > 0 {
                result.push(format!("   ... ({} internal frames)", internal_count));
                internal_count = 0;
            }
            result.push(line.to_string());
        }
    }
    if internal_count > 0 {
        result.push(format!("   ... ({} internal frames)", internal_count));
    }
    result.join("\n")
}

/// Aggregated test results for compact display
#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub(crate) struct AggregatedTestResult {
    passed: usize,
    failed: usize,
    ignored: usize,
    measured: usize,
    filtered_out: usize,
    suites: usize,
    duration_secs: f64,
    has_duration: bool,
}

impl AggregatedTestResult {
    /// Parse a test result summary line
    /// Format: "test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
    pub(crate) fn parse_line(line: &str) -> Option<Self> {
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = RE.get_or_init(|| {
            regex::Regex::new(
                r"test result: (\w+)\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored;\s+(\d+) measured;\s+(\d+) filtered out(?:;\s+finished in ([\d.]+)s)?"
            ).expect("regex: cargo test result summary")
        });

        let caps = re.captures(line)?;
        let status = caps.get(1)?.as_str();

        // Only aggregate if status is "ok" (all tests passed)
        if status != "ok" {
            return None;
        }

        let passed = caps.get(2)?.as_str().parse().ok()?;
        let failed = caps.get(3)?.as_str().parse().ok()?;
        let ignored = caps.get(4)?.as_str().parse().ok()?;
        let measured = caps.get(5)?.as_str().parse().ok()?;
        let filtered_out = caps.get(6)?.as_str().parse().ok()?;

        let (duration_secs, has_duration) = if let Some(duration_match) = caps.get(7) {
            (duration_match.as_str().parse().unwrap_or(0.0), true)
        } else {
            (0.0, false)
        };

        Some(Self {
            passed,
            failed,
            ignored,
            measured,
            filtered_out,
            suites: 1,
            duration_secs,
            has_duration,
        })
    }

    /// Merge another test result into this one
    pub(crate) fn merge(&mut self, other: &Self) {
        self.passed += other.passed;
        self.failed += other.failed;
        self.ignored += other.ignored;
        self.measured += other.measured;
        self.filtered_out += other.filtered_out;
        self.suites += other.suites;
        self.duration_secs += other.duration_secs;
        self.has_duration = self.has_duration && other.has_duration;
    }

    /// Format as compact single line
    pub(crate) fn format_compact(&self) -> String {
        let mut parts = vec![format!("{} passed", self.passed)];

        if self.ignored > 0 {
            parts.push(format!("{} ignored", self.ignored));
        }
        if self.filtered_out > 0 {
            parts.push(format!("{} filtered out", self.filtered_out));
        }

        let counts = parts.join(", ");

        let suite_text = if self.suites == 1 {
            "1 suite".to_string()
        } else {
            format!("{} suites", self.suites)
        };

        if self.has_duration {
            format!(
                "✓ cargo test: {} ({}, {:.2}s)",
                counts, suite_text, self.duration_secs
            )
        } else {
            format!("✓ cargo test: {} ({})", counts, suite_text)
        }
    }
}

/// Filter cargo test output - show failures + summary only
#[allow(dead_code)]
pub(crate) fn filter_cargo_test(output: &str, show_passing: bool) -> String {
    let mut failures: Vec<String> = Vec::new();
    let mut summary_lines: Vec<String> = Vec::new();
    let mut passing_lines: Vec<String> = Vec::new();
    let mut in_failure_section = false;
    let mut current_failure = Vec::new();

    for line in output.lines() {
        // Skip compilation lines
        if line.trim_start().starts_with("Compiling")
            || line.trim_start().starts_with("Downloading")
            || line.trim_start().starts_with("Downloaded")
            || line.trim_start().starts_with("Finished")
        {
            continue;
        }

        // Handle individual test result lines
        if line.starts_with("running ") {
            continue;
        }

        if line.starts_with("test ") && line.ends_with("... ok") {
            if show_passing {
                passing_lines.push(format!("✓ {}", line));
            }
            continue;
        }

        // Detect failures section
        if line == "failures:" {
            in_failure_section = true;
            continue;
        }

        if in_failure_section {
            if line.starts_with("test result:") {
                in_failure_section = false;
                summary_lines.push(line.to_string());
            } else if line.starts_with("    ") || line.starts_with("---- ") {
                current_failure.push(line.to_string());
            } else if line.trim().is_empty() && !current_failure.is_empty() {
                failures.push(current_failure.join("\n"));
                current_failure.clear();
            } else if !line.trim().is_empty() {
                current_failure.push(line.to_string());
            }
        }

        // Capture test result summary
        if !in_failure_section && line.starts_with("test result:") {
            summary_lines.push(line.to_string());
        }
    }

    if !current_failure.is_empty() {
        failures.push(current_failure.join("\n"));
    }

    let mut result = String::new();

    // Show passing tests only if enabled AND there are no failures
    if !passing_lines.is_empty() && failures.is_empty() {
        for line in &passing_lines {
            result.push_str(&format!("{}\n", line));
        }
    }

    if failures.is_empty() && !summary_lines.is_empty() {
        // All passed - try to aggregate
        let mut aggregated: Option<AggregatedTestResult> = None;
        let mut all_parsed = true;

        for line in &summary_lines {
            if let Some(parsed) = AggregatedTestResult::parse_line(line) {
                if let Some(ref mut agg) = aggregated {
                    agg.merge(&parsed);
                } else {
                    aggregated = Some(parsed);
                }
            } else {
                all_parsed = false;
                break;
            }
        }

        // If all lines parsed successfully and we have at least one suite, return compact format
        if all_parsed
            && let Some(agg) = aggregated
            && agg.suites > 0
        {
            if show_passing && !passing_lines.is_empty() {
                result.push_str(&agg.format_compact());
            } else {
                return agg.format_compact();
            }
        }

        // Fallback: use original behavior if regex failed
        for line in &summary_lines {
            result.push_str(&format!("✓ {}\n", line));
        }
        return result.trim().to_string();
    }

    if !failures.is_empty() {
        result.push_str(&format!("FAILURES ({}):\n", failures.len()));
        result.push_str("═══════════════════════════════════════\n");
        let mut tail_names: Vec<String> = Vec::new();
        for (i, failure) in failures.iter().enumerate() {
            if i < 5 {
                // Full failure block with internal frame collapsing
                result.push_str(&format!(
                    "{}. {}\n",
                    i + 1,
                    collapse_internal_frames(failure)
                ));
            } else if i < 10 {
                // First line only
                let first_line = failure.lines().next().unwrap_or(failure);
                result.push_str(&format!("{}. {}\n", i + 1, first_line));
            } else {
                // Collect remaining names
                let name = failure.lines().next().unwrap_or(failure);
                tail_names.push(name.to_string());
            }
        }
        if !tail_names.is_empty() {
            result.push_str(&format!("... also failed: {}\n", tail_names.join(", ")));
        }
        result.push('\n');
    }

    // Collect passing names from output lines matching "test <name> ... ok"
    let mut passing_names: Vec<String> = Vec::new();
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("test ")
            && let Some(name) = rest.strip_suffix(" ... ok")
        {
            passing_names.push(name.trim().to_string());
        }
    }
    if failures.is_empty() && !passing_names.is_empty() {
        if passing_names.len() <= 20 {
            result.push_str(&format!(
                "passed: {} ({} total)\n",
                passing_names.join(", "),
                passing_names.len()
            ));
        } else {
            result.push_str(&format!("passed: {} tests\n", passing_names.len()));
        }
    }

    for line in &summary_lines {
        result.push_str(&format!("{}\n", line));
    }

    if result.trim().is_empty() {
        // Fallback: show last meaningful lines
        let meaningful: Vec<&str> = output
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("Compiling"))
            .collect();
        for line in meaningful.iter().rev().take(5).rev() {
            result.push_str(&format!("{}\n", line));
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cargo_test_all_pass() {
        let output = r#"   Compiling mycelium v0.5.0
    Finished test [unoptimized + debuginfo] target(s) in 2.53s
     Running target/debug/deps/mycelium-abc123

running 15 tests
test utils::tests::test_truncate_short_string ... ok
test utils::tests::test_truncate_long_string ... ok
test utils::tests::test_strip_ansi_simple ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
"#;
        let result = filter_cargo_test(output, false);
        assert!(
            result.contains("✓ cargo test: 15 passed (1 suite, 0.01s)"),
            "Expected compact format, got: {}",
            result
        );
        assert!(!result.contains("Compiling"));
        assert!(!result.contains("test utils"));
    }

    #[test]
    fn test_filter_cargo_test_failures() {
        let output = r#"running 5 tests
test foo::test_a ... ok
test foo::test_b ... FAILED
test foo::test_c ... ok

failures:

---- foo::test_b stdout ----
thread 'foo::test_b' panicked at 'assert_eq!(1, 2)'

failures:
    foo::test_b

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
"#;
        let result = filter_cargo_test(output, false);
        assert!(result.contains("FAILURES"));
        assert!(result.contains("test_b"));
        assert!(result.contains("test result:"));
    }

    #[test]
    fn test_filter_cargo_test_multi_suite_all_pass() {
        let output = r#"   Compiling mycelium v0.5.0
    Finished test [unoptimized + debuginfo] target(s) in 2.53s
     Running unittests src/lib.rs (target/debug/deps/mycelium-abc123)

running 50 tests
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

     Running unittests src/main.rs (target/debug/deps/mycelium-def456)

running 30 tests
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

     Running tests/integration.rs (target/debug/deps/integration-ghi789)

running 25 tests
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s

   Doc-tests mycelium

running 32 tests
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
"#;
        let result = filter_cargo_test(output, false);
        assert!(
            result.contains("✓ cargo test: 137 passed (4 suites, 1.45s)"),
            "Expected aggregated format, got: {}",
            result
        );
        assert!(!result.contains("running"));
    }

    #[test]
    fn test_filter_cargo_test_multi_suite_with_failures() {
        let output = r#"     Running unittests src/lib.rs

running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

     Running unittests src/main.rs

running 15 tests
test foo::test_bad ... FAILED

failures:

---- foo::test_bad stdout ----
thread panicked at 'assertion failed'

test result: FAILED. 14 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

     Running tests/integration.rs

running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
"#;
        let result = filter_cargo_test(output, false);
        // Should NOT aggregate when there are failures
        assert!(result.contains("FAILURES"), "got: {}", result);
        assert!(result.contains("test_bad"), "got: {}", result);
        assert!(result.contains("test result:"), "got: {}", result);
        // Should show individual summaries
        assert!(result.contains("20 passed"), "got: {}", result);
        assert!(result.contains("14 passed"), "got: {}", result);
        assert!(result.contains("10 passed"), "got: {}", result);
    }

    #[test]
    fn test_filter_cargo_test_all_suites_zero_tests() {
        let output = r#"     Running unittests src/empty1.rs

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/empty2.rs

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/empty3.rs

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"#;
        let result = filter_cargo_test(output, false);
        assert!(
            result.contains("✓ cargo test: 0 passed (3 suites, 0.00s)"),
            "Expected compact format for zero tests, got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_test_with_ignored_and_filtered() {
        let output = r#"     Running unittests src/lib.rs

running 50 tests
test result: ok. 45 passed; 0 failed; 3 ignored; 0 measured; 2 filtered out; finished in 0.50s

     Running tests/integration.rs

running 20 tests
test result: ok. 18 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.20s
"#;
        let result = filter_cargo_test(output, false);
        assert!(
            result.contains("✓ cargo test: 63 passed, 5 ignored, 2 filtered out (2 suites, 0.70s)"),
            "Expected compact format with ignored and filtered, got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_test_single_suite_compact() {
        let output = r#"     Running unittests src/main.rs

running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
"#;
        let result = filter_cargo_test(output, false);
        assert!(
            result.contains("✓ cargo test: 15 passed (1 suite, 0.01s)"),
            "Expected singular 'suite', got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_test_regex_fallback() {
        let output = r#"     Running unittests src/main.rs

running 15 tests
test result: MALFORMED LINE WITHOUT PROPER FORMAT
"#;
        let result = filter_cargo_test(output, false);
        // Should fallback to original behavior (show line with checkmark)
        assert!(
            result.contains("✓ test result: MALFORMED"),
            "Expected fallback format, got: {}",
            result
        );
    }

    #[test]
    fn test_filter_cargo_test_show_passing_false() {
        let output = r#"     Running unittests src/lib.rs

running 3 tests
test utils::test_a ... ok
test utils::test_b ... ok
test utils::test_c ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
"#;
        let result = filter_cargo_test(output, false);
        // With show_passing=false, should not show individual passing tests
        assert!(result.contains("✓ cargo test"));
        assert!(!result.contains("test utils::test_a"));
        assert!(!result.contains("test utils::test_b"));
    }

    #[test]
    fn test_filter_cargo_test_show_passing_true() {
        let output = r#"     Running unittests src/lib.rs

running 3 tests
test utils::test_a ... ok
test utils::test_b ... ok
test utils::test_c ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
"#;
        let result = filter_cargo_test(output, true);
        // With show_passing=true, should show individual passing tests
        assert!(result.contains("✓ test utils::test_a"));
        assert!(result.contains("✓ test utils::test_b"));
        assert!(result.contains("✓ test utils::test_c"));
        assert!(result.contains("✓ cargo test"));
    }

    #[test]
    fn test_filter_cargo_test_show_passing_with_failures() {
        let output = r#"running 3 tests
test foo::test_a ... ok
test foo::test_b ... FAILED
test foo::test_c ... ok

failures:

---- foo::test_b stdout ----
thread 'foo::test_b' panicked at 'assert_eq!(1, 2)'

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
"#;
        let result = filter_cargo_test(output, true);
        // With show_passing=true and failures, should show failures and summary (not passing tests)
        assert!(result.contains("FAILURES"));
        assert!(result.contains("test_b"));
        assert!(result.contains("test result:"));
        // When there are failures, we still don't show individual passing tests
        assert!(!result.contains("✓ test foo::test_a"));
    }
}
