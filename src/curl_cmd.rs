//! Token-optimized curl proxy that auto-formats JSON responses and truncates large outputs.
use crate::tracking;
use anyhow::{Context, Result};
use std::process::Command;

/// Execute curl with automatic JSON detection and response compression.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let mut cmd = Command::new("curl");
    cmd.arg("-s"); // Silent mode (no progress bar)

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: curl -s {}", args.join(" "));
    }

    let output = cmd.output().context("Failed to run curl")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let msg = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        eprintln!("FAILED: curl {}", msg);
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let raw = stdout.to_string();

    // Auto-detect JSON and pipe through filter (or route to Hyphae for large output)
    let filtered = crate::hyphae::route_or_filter(&format!("curl {}", args.join(" ")), &raw, |r| {
        filter_curl_output(r)
    });
    println!("{}", filtered);

    timer.track(
        &format!("curl {}", args.join(" ")),
        &format!("mycelium curl {}", args.join(" ")),
        &raw,
        &filtered,
    );

    Ok(())
}

fn filter_curl_output(output: &str) -> String {
    let trimmed = output.trim();

    // Try JSON detection
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && (trimmed.ends_with('}') || trimmed.ends_with(']'))
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed)
    {
        // HTTP error responses: always pass through in full
        if is_error_response(&parsed) {
            return serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| trimmed.to_string());
        }
        // Small JSON (<5KB): pretty-print, keep all values
        if trimmed.len() < 5120 {
            return serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| trimmed.to_string());
        }
        // Large JSON: truncate values but keep structure
        let truncated = truncate_json_values(&parsed, 0, 3);
        return serde_json::to_string_pretty(&truncated).unwrap_or_else(|_| trimmed.to_string());
    }

    // Non-JSON: cap at 100 lines
    let lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() > 100 {
        let mut result = lines[..100].join("\n");
        result.push_str(&format!("\n... ({} more lines)", lines.len() - 100));
        result
    } else {
        trimmed.to_string()
    }
}

fn is_error_response(value: &serde_json::Value) -> bool {
    if let serde_json::Value::Object(map) = value {
        map.contains_key("error") || map.contains_key("errors")
    } else {
        false
    }
}

fn truncate_json_values(
    value: &serde_json::Value,
    depth: usize,
    max_depth: usize,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) if s.len() > 100 => {
            serde_json::Value::String(format!("{}...({}chars)", &s[..100], s.len()))
        }
        serde_json::Value::Array(arr) if arr.len() > 3 => {
            let mut truncated: Vec<serde_json::Value> = arr[..3]
                .iter()
                .map(|v| truncate_json_values(v, depth + 1, max_depth))
                .collect();
            truncated.push(serde_json::Value::String(format!(
                "...+{} more",
                arr.len() - 3
            )));
            serde_json::Value::Array(truncated)
        }
        serde_json::Value::Object(map) if depth >= max_depth => {
            serde_json::Value::String(format!("{{...{} keys}}", map.len()))
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| truncate_json_values(v, depth + 1, max_depth))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), truncate_json_values(v, depth + 1, max_depth)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_curl_json() {
        // Small JSON: pretty-printed with actual values preserved
        let output = r#"{"name": "Alice", "count": 42, "status": "active"}"#;
        let result = filter_curl_output(output);
        assert!(result.contains("name"));
        assert!(result.contains("Alice"));
        assert!(result.contains("42"));
    }

    #[test]
    fn test_filter_curl_json_array() {
        let output = r#"[{"id": 1}, {"id": 2}]"#;
        let result = filter_curl_output(output);
        assert!(result.contains("id"));
    }

    #[test]
    fn test_filter_curl_non_json() {
        let output = "Hello, World!\nThis is plain text.";
        let result = filter_curl_output(output);
        assert!(result.contains("Hello, World!"));
        assert!(result.contains("plain text"));
    }

    #[test]
    fn test_filter_curl_json_small_returns_original() {
        // Small JSON: should be pretty-printed with values intact
        let output = r#"{"r2Ready":true,"status":"ok"}"#;
        let result = filter_curl_output(output);
        assert!(result.contains("true"));
        assert!(result.contains("ok"));
    }

    #[test]
    fn test_filter_curl_long_output() {
        let lines: Vec<String> = (0..110).map(|i| format!("Line {}", i)).collect();
        let output = lines.join("\n");
        let result = filter_curl_output(&output);
        assert!(result.contains("Line 0"));
        assert!(result.contains("Line 99"));
        assert!(result.contains("more lines"));
    }

    #[test]
    fn test_json_snapshot() {
        let input = include_str!("../tests/fixtures/curl_json_raw.txt");
        let output = filter_curl_output(input);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_non_json_passthrough() {
        let input =
            "<html><body><h1>Hello World</h1><p>This is a plain text response.</p></body></html>";
        let output = filter_curl_output(input);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_curl_large_json_truncates_values() {
        let fixture = include_str!("../tests/fixtures/curl_large_json.json");
        let result = filter_curl_output(fixture);
        // Should not contain the full long strings
        assert!(result.len() < fixture.len(), "should be smaller than input");
        // Should still be valid JSON
        assert!(
            serde_json::from_str::<serde_json::Value>(&result).is_ok(),
            "should be valid JSON"
        );
        insta::assert_snapshot!(result);
    }

    #[test]
    fn test_empty_input() {
        let output = filter_curl_output("");
        assert_eq!(output, "");
    }
}
