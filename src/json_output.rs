//! JSON envelope wrapper that includes token savings metrics alongside filtered output.
use serde_json::json;

use crate::tracking;

/// Wrap filtered command output in a JSON envelope with token savings metrics.
pub fn wrap_output(
    command: &str,
    mycelium_command: &str,
    filtered_output: &str,
    raw_output: &str,
) -> String {
    let tokens_raw = tracking::estimate_tokens(raw_output);
    let tokens_filtered = tracking::estimate_tokens(filtered_output);
    let savings_pct = if tokens_raw > 0 && tokens_raw > tokens_filtered {
        ((tokens_raw - tokens_filtered) as f64 / tokens_raw as f64) * 100.0
    } else {
        0.0
    };

    serde_json::to_string(&json!({
        "command": command,
        "mycelium_command": mycelium_command,
        "output": filtered_output,
        "tokens_raw": tokens_raw,
        "tokens_filtered": tokens_filtered,
        "savings_pct": (savings_pct * 10.0).round() / 10.0
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Wrap an error in a JSON envelope.
pub fn wrap_error(message: &str, exit_code: i32) -> String {
    serde_json::to_string(&json!({
        "error": message,
        "exit_code": exit_code
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_output_basic() {
        let result = wrap_output(
            "git status",
            "mycelium git status",
            "filtered output",
            "raw output",
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["command"], "git status");
        assert_eq!(v["mycelium_command"], "mycelium git status");
        assert_eq!(v["output"], "filtered output");
    }

    #[test]
    fn test_wrap_error() {
        let result = wrap_error("something went wrong", 1);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["error"], "something went wrong");
        assert_eq!(v["exit_code"], 1);
    }

    #[test]
    fn test_savings_pct_calculated() {
        // raw_output is longer → filtered_output saves tokens
        let raw = "a".repeat(400); // 100 tokens
        let filtered = "a".repeat(40); // 10 tokens → 90% savings
        let result = wrap_output("cmd", "mycelium cmd", &filtered, &raw);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        let savings = v["savings_pct"].as_f64().expect("f64");
        assert!(savings > 80.0, "expected >80% savings, got {savings}");
        assert_eq!(v["tokens_raw"], 100);
        assert_eq!(v["tokens_filtered"], 10);
    }

    #[test]
    fn test_valid_json_parseable_by_serde() {
        let result = wrap_output(
            "cargo test",
            "mycelium cargo test",
            "1 passed",
            "running 1 test\n1 passed",
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("must be valid JSON");
        assert!(v.is_object());
        assert!(v.get("savings_pct").is_some());
        assert!(v.get("tokens_raw").is_some());
        assert!(v.get("tokens_filtered").is_some());
    }
}
