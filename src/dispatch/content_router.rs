//! Content-aware routing for command output filtering.
//!
//! Detects output type (JSON, code/diff, structured text, or general text)
//! and applies appropriate filtering to reduce token usage while preserving
//! essential information.

use serde_json::Value;

/// Check whether input looks like unified diff output.
fn looks_like_diff(input: &str) -> bool {
    input.lines().any(|line| {
        line.starts_with("diff --git")
            || line.starts_with("--- a/")
            || line.starts_with("+++ b/")
            || line.starts_with("@@ ")
    })
}

/// Detected content type of command output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// JSON object or array output.
    Json,
    /// Code, diffs, or structured markup output.
    Code,
    /// Structured text with consistent delimiters (tables, YAML-like).
    StructuredText,
    /// General prose, logs, or unstructured output.
    GeneralText,
}

/// Routes command output through content-appropriate filters.
pub struct ContentRouter {
    /// Maximum number of array items to keep in JSON output (default: 20).
    pub json_max_array_items: usize,
    /// Maximum number of context lines to keep in code/diff output (default: 50).
    pub code_max_hunk_lines: usize,
}

impl Default for ContentRouter {
    fn default() -> Self {
        Self {
            json_max_array_items: 20,
            code_max_hunk_lines: 50,
        }
    }
}

impl ContentRouter {
    /// Creates a new ContentRouter with custom limits.
    #[allow(dead_code)] // Used in tests
    pub fn new(json_max_array_items: usize, code_max_hunk_lines: usize) -> Self {
        Self {
            json_max_array_items,
            code_max_hunk_lines,
        }
    }

    /// Detects the content type of the given output string.
    pub fn detect_content_type(output: &str) -> ContentType {
        let trimmed = output.trim_start();

        // Check for JSON — require stronger evidence than just a leading { or [
        // to reduce false positives on prose that starts with those characters.
        // Walk backward from 512 to find a valid char boundary, so the probe slice
        // never splits a multi-byte UTF-8 sequence.
        let probe_end = {
            let limit = trimmed.len().min(512);
            let mut end = limit;
            while end > 0 && !trimmed.is_char_boundary(end) {
                end -= 1;
            }
            end
        };
        if trimmed.starts_with('{') {
            // JSON objects start with `{"key"`, so the second byte being `"` is a fast path
            // for the common case. The fast path may over-accept (e.g. `{"not json`), but
            // filter_json performs a full parse and passes through invalid JSON unchanged,
            // so routing accuracy here does not affect output correctness.
            let looks_like_json = trimmed.as_bytes().get(1) == Some(&b'"')
                || serde_json::from_str::<Value>(&trimmed[..probe_end]).is_ok();
            if looks_like_json {
                return ContentType::Json;
            }
        } else if trimmed.starts_with('[') {
            // For arrays: if it parses as valid JSON, treat it as JSON.
            // This catches both single-element and multi-element arrays that are
            // legitimately JSON structures, while avoiding prose that happens to start
            // with `[` but doesn't form valid JSON.
            if serde_json::from_str::<Value>(&trimmed[..probe_end]).is_ok() {
                return ContentType::Json;
            }
        }

        // Check for code/diffs (hunk markers, code blocks, structural diff markers)
        if output.contains("```") || looks_like_diff(output) {
            return ContentType::Code;
        }

        // Structured heuristic: check if majority of first 100 lines contain : or |
        let all_lines: Vec<&str> = output.lines().take(100).collect();
        if !all_lines.is_empty() {
            let structured = all_lines
                .iter()
                .filter(|l| l.contains(':') || l.contains('|'))
                .count();
            if structured * 2 > all_lines.len() {
                return ContentType::StructuredText;
            }
        }

        ContentType::GeneralText
    }

    /// Filters JSON output: removes nulls and truncates large arrays.
    fn filter_json(&self, output: &str) -> String {
        match serde_json::from_str::<Value>(output) {
            Ok(value) => {
                let filtered = self.filter_json_value(&value);
                match serde_json::to_string_pretty(&filtered) {
                    Ok(formatted) => formatted,
                    Err(_) => output.to_string(),
                }
            }
            Err(_) => output.to_string(), // Passthrough if parsing fails
        }
    }

    /// Recursively filters JSON values.
    fn filter_json_value(&self, value: &Value) -> Value {
        match value {
            Value::Object(obj) => {
                let mut filtered = serde_json::Map::new();
                for (k, v) in obj.iter() {
                    // Skip null values
                    if !v.is_null() {
                        filtered.insert(k.clone(), self.filter_json_value(v));
                    }
                }
                Value::Object(filtered)
            }
            Value::Array(arr) => {
                let truncated: Vec<_> = arr
                    .iter()
                    .take(self.json_max_array_items)
                    .map(|v| self.filter_json_value(v))
                    .collect();

                let truncated_len = truncated.len();
                let arr_len = arr.len();

                if truncated_len < arr_len {
                    let mut result = truncated;
                    result.push(Value::String(format!(
                        "... ({} more items)",
                        arr_len - truncated_len
                    )));
                    Value::Array(result)
                } else {
                    Value::Array(truncated)
                }
            }
            other => other.clone(),
        }
    }

    /// Filters code/diff output: keeps error/warning lines and condenses context.
    ///
    /// Up to `code_max_hunk_lines` consecutive non-important lines are kept verbatim.
    /// Lines beyond that limit are counted and emitted as `[... N context lines ...]`
    /// at the next important line or end of input.
    fn filter_code(&self, output: &str) -> String {
        let mut result = Vec::new();
        // Total consecutive non-important lines in the current run.
        let mut context_run: usize = 0;
        // How many of those lines were pushed to result (within the limit).
        let mut context_kept: usize = 0;

        for line in output.lines() {
            let is_important = line.contains("error")
                || line.contains("Error")
                || line.contains("ERROR")
                || line.contains("warning")
                || line.contains("Warning")
                || line.contains("WARNING")
                || line.contains("FAIL")
                || line.contains("panic")
                || line.contains("PANIC")
                || line.contains("fatal")
                || line.contains("FATAL")
                || line.contains("exception")
                || line.contains("Exception")
                || line.contains("traceback")
                || line.contains("Traceback")
                || line.contains("assertion")
                || line.contains("Assertion")
                || line.contains("killed")
                || line.contains("Killed")
                || line.contains("denied")
                || line.contains("Denied")
                || line.contains("aborted")
                || line.contains("Aborted")
                || line.contains("segfault")
                || line.contains("SIGSEGV")
                || line.starts_with("@@");

            if is_important {
                // Emit ellipsis for any context lines that exceeded the limit.
                let skipped = context_run.saturating_sub(context_kept);
                if skipped > 0 {
                    result.push(format!("[... {} context lines ...]", skipped));
                }
                context_run = 0;
                context_kept = 0;
                result.push(line.to_string());
            } else {
                context_run += 1;
                if context_run <= self.code_max_hunk_lines {
                    result.push(line.to_string());
                    context_kept += 1;
                }
                // Lines beyond the limit are counted in context_run but not pushed.
            }
        }

        // Flush any remaining context lines that exceeded the limit.
        let skipped = context_run.saturating_sub(context_kept);
        if skipped > 0 {
            result.push(format!("[... {} context lines ...]", skipped));
        }

        result.join("\n")
    }

    /// Routes output through appropriate filter based on detected type.
    pub fn route(&self, output: &str) -> String {
        let content_type = Self::detect_content_type(output);

        match content_type {
            ContentType::Json => self.filter_json(output),
            ContentType::Code => self.filter_code(output),
            ContentType::StructuredText | ContentType::GeneralText => output.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_json_object() {
        let output = r#"{"name": "test", "value": 42}"#;
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::Json
        );
    }

    #[test]
    fn detect_json_array() {
        let output = r#"[1, 2, 3, 4, 5]"#;
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::Json
        );
    }

    #[test]
    fn detect_json_with_whitespace() {
        let output = "  \n\n  [{\"a\": 1}]";
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::Json
        );
    }

    #[test]
    fn detect_code_with_hunk_marker() {
        let output = "diff --git a/file.txt b/file.txt\n@@-1,5+1,6@@\n+added line";
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::Code
        );
    }

    #[test]
    fn detect_code_with_code_fence() {
        let output = "Here's some code:\n```rust\nfn main() {}\n```";
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::Code
        );
    }

    #[test]
    fn detect_structured_text() {
        let output = "Name: John\nAge: 30\nCity: NYC\nEmail: test@example.com";
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::StructuredText
        );
    }

    #[test]
    fn detect_general_text() {
        let output = "This is just plain prose with no special structure or formatting.";
        assert_eq!(
            ContentRouter::detect_content_type(output),
            ContentType::GeneralText
        );
    }

    #[test]
    fn filter_json_strips_nulls() {
        let router = ContentRouter::default();
        let input = r#"{"name": "test", "value": null, "id": 1}"#;
        let output = router.filter_json(input);

        // Verify null is removed and valid fields remain
        assert!(!output.contains("null"));
        assert!(output.contains("test"));
        assert!(output.contains("1"));
    }

    #[test]
    fn filter_json_truncates_arrays() {
        let router = ContentRouter::new(5, 50);
        let mut arr = vec![];
        for i in 0..25 {
            arr.push(serde_json::json!({"id": i}));
        }
        let input = serde_json::to_string(&arr).unwrap();
        let output = router.filter_json(&input);

        // Should contain only 5 items + ellipsis message
        assert!(output.contains("more items"));
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 6); // 5 items + 1 ellipsis string
    }

    #[test]
    fn filter_json_keeps_small_arrays() {
        let router = ContentRouter::default();
        let input = r#"[1, 2, 3]"#;
        let output = router.filter_json(input);

        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 3);
    }

    #[test]
    fn filter_code_keeps_error_lines() {
        let router = ContentRouter::default();
        let input = "line 1\nline 2\nerror: something failed\nline 4";
        let output = router.filter_code(input);

        assert!(output.contains("error: something failed"));
        assert!(output.contains("line 1"));
    }

    #[test]
    fn filter_code_keeps_warning_lines() {
        let router = ContentRouter::default();
        let input = "info line\nwarning: be careful\nmore info";
        let output = router.filter_code(input);

        assert!(output.contains("warning: be careful"));
    }

    #[test]
    fn filter_code_keeps_hunk_markers() {
        let router = ContentRouter::default();
        let input = " context\n@@ -1,5 +1,6 @@\n+added\n context";
        let output = router.filter_code(input);

        assert!(output.contains("@@"));
    }

    #[test]
    fn route_returns_string() {
        let router = ContentRouter::default();
        // Should not panic on any input
        let result = router.route("random text");
        assert!(!result.is_empty());

        let json_result = router.route(r#"{"key": "value"}"#);
        assert!(!json_result.is_empty());

        let empty = router.route("");
        assert_eq!(empty, "");
    }

    #[test]
    fn route_json_detects_and_filters() {
        let router = ContentRouter::new(2, 50);
        let input = serde_json::json!({
            "name": "test",
            "items": [1, 2, 3, 4, 5],
            "meta": null
        })
        .to_string();

        let output = router.route(&input);

        // Should be valid JSON
        let _: Value =
            serde_json::from_str(&output).expect("output should be valid JSON after filtering");

        // Should have removed null
        assert!(!output.contains("\"meta\""));

        // Should truncate array
        assert!(output.contains("more items"));
    }

    #[test]
    fn route_code_detects_and_filters() {
        let router = ContentRouter::default();
        let input = "normal output\nerror: test failed\nmore output";
        let output = router.route(input);

        assert!(output.contains("error: test failed"));
    }

    #[test]
    fn route_passthrough_for_structured() {
        let router = ContentRouter::default();
        let input = "Key: value\nFoo: bar\nAnother: field";
        let output = router.route(input);

        // Structured text should pass through unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn route_passthrough_for_general() {
        let router = ContentRouter::default();
        let input = "Just some random text without any special structure";
        let output = router.route(input);

        // General text should pass through unchanged
        assert_eq!(output, input);
    }

    #[test]
    fn filter_code_keeps_panic_lines() {
        let router = ContentRouter::default();
        let input = "normal line\nthread 'main' panicked at 'assertion failed'\nmore output";
        let output = router.filter_code(input);

        assert!(output.contains("panicked"));
    }

    #[test]
    fn filter_code_keeps_traceback_lines() {
        let router = ContentRouter::default();
        let input = "line 1\nTraceback (most recent call last):\n  File \"test.py\"\nline 4";
        let output = router.filter_code(input);

        assert!(output.contains("Traceback"));
    }

    #[test]
    fn filter_code_keeps_sigsegv_lines() {
        let router = ContentRouter::default();
        let input = "execution started\nSegmentation fault: SIGSEGV\ncleanup";
        let output = router.filter_code(input);

        assert!(output.contains("SIGSEGV"));
    }

    #[test]
    fn email_address_not_detected_as_diff() {
        // @@ in an email address must not trigger diff detection
        let content_type = ContentRouter::detect_content_type("Contact: user@example.com or admin@@corp.org");
        assert_ne!(content_type, ContentType::Code);
    }
}
