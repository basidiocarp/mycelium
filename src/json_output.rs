//! JSON envelope wrapper that includes token savings metrics alongside filtered output.
use serde_json::json;
use std::borrow::Cow;

use crate::{rewrite_cmd::RuntimeResolution, tracking};

/// Redact sensitive content patterns from text without allocating if no secrets match.
///
/// Uses a set of high-precision patterns to identify and mask credentials, tokens,
/// and private keys. Returns `Cow::Borrowed` (zero-copy) when no secret matches,
/// or `Cow::Owned` with redacted content when matches are found.
fn redact_secrets(text: &str) -> Cow<'_, str> {
    use std::sync::LazyLock;

    // Compile all patterns once and reuse
    static PATTERNS: LazyLock<Vec<(regex::Regex, &str)>> = LazyLock::new(|| {
        vec![
            // AWS access key ids (AKIA or ASIA prefix)
            (
                regex::Regex::new(r"\b(AKIA|ASIA)[0-9A-Z]{16}\b").expect("aws_key regex"),
                "****REDACTED****",
            ),
            // GitHub tokens (multiple formats)
            (
                regex::Regex::new(r"\bgh[posru]_[A-Za-z0-9]{36,}\b").expect("gh_token regex"),
                "****REDACTED****",
            ),
            (
                regex::Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{22,}\b").expect("github_pat regex"),
                "****REDACTED****",
            ),
            // OpenAI/Anthropic-style keys. Require a 20+ char unbroken alphanumeric run so
            // hyphen-segmented infrastructure slugs (e.g. `sk-prod-us-east-1-cluster-name`)
            // are not redacted, while real keys — legacy `sk-`+alnum, `sk-proj-…`, `sk-ant-…`,
            // all of which contain a long unbroken run — still match.
            (
                regex::Regex::new(r"\bsk-[A-Za-z0-9_\-]*[A-Za-z0-9]{20,}\b").expect("sk_key regex"),
                "****REDACTED****",
            ),
            // Slack tokens
            (
                regex::Regex::new(r"\bxox[baprs]-[A-Za-z0-9\-]{10,}\b").expect("slack_token regex"),
                "****REDACTED****",
            ),
            // JWT (three base64url parts separated by dots)
            (
                regex::Regex::new(r"\beyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\b")
                    .expect("jwt regex"),
                "****REDACTED****",
            ),
            // PEM private key blocks (any type)
            (
                regex::Regex::new(
                    r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
                )
                .expect("pem_key regex"),
                "****REDACTED****",
            ),
            // Bearer tokens: match 'Bearer', capture the original whitespace run, then the token.
            // The token class includes base64 / base64url chars (`/ + =`) so opaque tokens
            // (Google `ya29.`, OAuth) are masked whole — omitting them leaks the token's tail.
            // It still stops at whitespace/quote/comma/brace (none of which are in the class).
            // A 16-char floor keeps ordinary prose ("Bearer bonds") from being redacted while
            // covering all credential-grade tokens. The captured whitespace is re-emitted so the
            // surrounding structure (e.g. tab-delimited log lines) is preserved.
            (
                regex::Regex::new(r"\bBearer(\s+)[A-Za-z0-9._/+=\-]{16,}")
                    .expect("bearer_token regex"),
                "Bearer${1}****REDACTED****",
            ),
        ]
    });

    // Check if any pattern matches
    if PATTERNS.iter().any(|(re, _)| re.is_match(text)) {
        let mut result = text.to_string();
        for (re, replacement) in PATTERNS.iter() {
            result = re.replace_all(&result, *replacement).to_string();
        }
        Cow::Owned(result)
    } else {
        Cow::Borrowed(text)
    }
}

/// Wrap filtered command output in a JSON envelope with token savings metrics.
#[allow(clippy::cast_precision_loss)]
pub fn wrap_output(
    command: &str,
    mycelium_command: &str,
    filtered_output: &str,
    raw_output: &str,
    project_path: Option<&str>,
    resolution: Option<&RuntimeResolution>,
) -> String {
    let tokens_raw = tracking::estimate_tokens(raw_output);
    let redacted = redact_secrets(filtered_output);
    let tokens_filtered = tracking::estimate_tokens(&redacted);
    let savings_pct = if tokens_raw > 0 && tokens_raw > tokens_filtered {
        ((tokens_raw - tokens_filtered) as f64 / tokens_raw as f64) * 100.0
    } else {
        0.0
    };

    serde_json::to_string(&json!({
        "command": command,
        "mycelium_command": mycelium_command,
        "output": &*redacted,
        "tokens_raw": tokens_raw,
        "tokens_filtered": tokens_filtered,
        "savings_pct": (savings_pct * 10.0).round() / 10.0,
        "project_path": project_path,
        "rewrite": resolution.map(|resolution| json!({
            "input": resolution.input,
            "execute": resolution.command,
            "rewritten": resolution.rewritten,
            "source": resolution.source,
            "reason": resolution.reason,
            "estimated_savings_pct": resolution.estimated_savings_pct,
        }))
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// Wrap an error in a JSON envelope.
///
/// `exit_code` must be the proxied tool's exit code, not mycelium's own process exit code.
/// `mycelium_error` is `true` only when the failure originated inside mycelium itself
/// (spawn failure, empty output after inner process error) rather than from the proxied tool.
pub fn wrap_error(message: &str, exit_code: i32, mycelium_error: bool) -> String {
    let redacted = redact_secrets(message);
    serde_json::to_string(&json!({
        "error": &*redacted,
        "exit_code": exit_code,
        "mycelium_error": mycelium_error
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
            Some("/tmp/project"),
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["command"], "git status");
        assert_eq!(v["mycelium_command"], "mycelium git status");
        assert_eq!(v["output"], "filtered output");
        assert_eq!(v["project_path"], "/tmp/project");
    }

    #[test]
    fn test_wrap_error() {
        let result = wrap_error("something went wrong", 1, true);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["error"], "something went wrong");
        assert_eq!(v["exit_code"], 1);
        assert_eq!(v["mycelium_error"], true);
    }

    #[test]
    fn test_wrap_error_tool_failure_not_mycelium_error() {
        let result = wrap_error("tool exited with non-zero status", 2, false);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["exit_code"], 2);
        assert_eq!(v["mycelium_error"], false);
    }

    #[test]
    fn test_wrap_error_exit_code_is_proxied_tool_code_not_inner_mycelium() {
        // raw_exit_code (proxied tool) = 2, inner mycelium exit = 1 (different).
        // The envelope must carry the proxied tool's code, not mycelium's.
        let result = wrap_error("tool output on failure", 2, false);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["exit_code"], 2);
        assert_eq!(v["mycelium_error"], false);
    }

    #[test]
    fn test_savings_pct_calculated() {
        // raw_output is longer → filtered_output saves tokens
        let raw = "a".repeat(400); // 100 tokens
        let filtered = "a".repeat(40); // 10 tokens → 90% savings
        let result = wrap_output("cmd", "mycelium cmd", &filtered, &raw, None, None);
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
            None,
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("must be valid JSON");
        assert!(v.is_object());
        assert!(v.get("savings_pct").is_some());
        assert!(v.get("tokens_raw").is_some());
        assert!(v.get("tokens_filtered").is_some());
    }

    #[test]
    fn test_wrap_output_includes_rewrite_metadata() {
        let resolution = RuntimeResolution {
            input: "git status".to_string(),
            command: "mycelium git status".to_string(),
            rewritten: true,
            source: "built-in registry".to_string(),
            reason: "matched Git rule".to_string(),
            estimated_savings_pct: Some(92.0),
        };
        let result = wrap_output(
            "git status",
            "mycelium git status",
            "filtered output",
            "raw output",
            None,
            Some(&resolution),
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        assert_eq!(v["rewrite"]["rewritten"], true);
        assert_eq!(v["rewrite"]["source"], "built-in registry");
        assert_eq!(v["rewrite"]["estimated_savings_pct"], 92.0);
    }

    // Tests for redact_secrets function

    #[test]
    fn test_redact_aws_access_key() {
        let input = "Your AWS key is AKIA1234567890ABCDEF";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("AKIA1234567890ABCDEF"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_github_token() {
        // 36-char suffix — matches the real GitHub PAT shape the pattern enforces (`{36,}`).
        let input = "Token: ghp_abc123def456ghi789jkl012mno345pqr678";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("ghp_abc123def456ghi789jkl012mno345pqr678"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_github_pat() {
        let input = "Token: github_pat_11234567890abcdefghijk";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("github_pat_11234567890abcdefghijk"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_openai_key() {
        let input = "API key: sk-proj-abc123def456ghi789jkl";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("sk-proj-abc123def456ghi789jkl"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_slack_token() {
        // Fixture deliberately breaks Slack's real xoxb-<digits>-<digits>- numeric
        // structure (avoids GitHub push-protection false positives) while still
        // exercising our prefix-anchored shape `xox[baprs]-[A-Za-z0-9-]{10,}`.
        let input = "Slack token: xoxb-EXAMPLEfaketokenNOTREAL000";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("xoxb-EXAMPLEfaketokenNOTREAL000"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_jwt() {
        let input = "JWT: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_pem_private_key() {
        let input =
            "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA...\n-----END RSA PRIVATE KEY-----";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("BEGIN RSA PRIVATE KEY"));
        assert!(redacted.contains("****REDACTED****"));
    }

    #[test]
    fn test_redact_bearer_token() {
        let input = "Authorization: Bearer sk_live_abc123def456ghi789jkl";
        let redacted = redact_secrets(input);
        assert!(redacted.contains("Bearer ****REDACTED****"));
        assert!(!redacted.contains("sk_live_abc123def456ghi789jkl"));
    }

    #[test]
    fn test_no_secret_returns_borrowed() {
        let input = "normal output without secrets";
        let redacted = redact_secrets(input);
        // Check that it's borrowed (same pointer)
        assert_eq!(redacted.as_ptr(), input.as_ptr());
        assert_eq!(&*redacted, input);
    }

    #[test]
    fn test_multiple_bearer_tokens_on_one_line() {
        let input = "Bearer abc123def456ghi789 and Bearer xyz789uvw012mno345";
        let redacted = redact_secrets(input);
        assert_eq!(redacted.matches("Bearer ****REDACTED****").count(), 2);
        assert!(!redacted.contains("abc123def456ghi789"));
        assert!(!redacted.contains("xyz789uvw012mno345"));
    }

    #[test]
    fn test_bearer_base64_token_fully_masked() {
        // Opaque base64 tokens contain `+`, `/`, `=` — the token class must cover them or the
        // tail leaks. Assert no fragment of the secret survives.
        let input = "Authorization: Bearer ya29.A0ARrdaM+9xZ/q==tWlongsecretvalue";
        let redacted = redact_secrets(input);
        assert!(redacted.contains("Bearer ****REDACTED****"));
        assert!(!redacted.contains("ya29"));
        assert!(!redacted.contains("9xZ"));
        assert!(!redacted.contains("tWlongsecretvalue"));
    }

    #[test]
    fn test_bearer_prose_word_not_redacted() {
        // Short word after "Bearer" (below the 16-char credential floor) is ordinary prose.
        let input = "I bought Bearer bonds yesterday";
        let redacted = redact_secrets(input);
        assert_eq!(&*redacted, input);
    }

    #[test]
    fn test_false_positive_guard_git_sha_and_version() {
        let input = "commit a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b version 1.2.3";
        let redacted = redact_secrets(input);
        // Git SHA and version should not be redacted
        assert_eq!(&*redacted, input);
    }

    #[test]
    fn test_false_positive_guard_sk_infra_slug() {
        // Hyphen-segmented infrastructure names share the `sk-` prefix but have no 20-char
        // unbroken alphanumeric run, so they must NOT be redacted (precision invariant).
        let input = "cluster sk-prod-us-east-1-cluster-name-v2 ready";
        let redacted = redact_secrets(input);
        assert_eq!(&*redacted, input);
    }

    #[test]
    fn test_bearer_token_stops_at_json_delimiters_and_preserves_whitespace() {
        // Token must stop at the closing quote/comma/brace, and the original tab whitespace
        // between `Bearer` and the token must be preserved.
        let input = "{\"auth\":\"Bearer\tabc123def456ghi789\",\"next\":\"keep-me\"}";
        let redacted = redact_secrets(input);
        assert!(redacted.contains("Bearer\t****REDACTED****"));
        assert!(!redacted.contains("abc123def456ghi789"));
        // Trailing fields are untouched — the redaction did not consume past the token.
        assert!(redacted.contains("\"next\":\"keep-me\""));
    }

    #[test]
    fn test_wrap_output_with_aws_secret() {
        let filtered = "AWS_ACCESS_KEY_ID=AKIA1234567890ABCDEF";
        let result = wrap_output(
            "aws sts get-session-token",
            "mycelium aws sts get-session-token",
            filtered,
            filtered,
            None,
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        let output_field = v["output"].as_str().expect("output field is string");
        assert!(!output_field.contains("AKIA1234567890ABCDEF"));
        assert!(output_field.contains("****REDACTED****"));
    }

    #[test]
    fn test_wrap_error_with_secret() {
        let error_msg = "failed with secret: AKIA1234567890ABCDEF";
        let result = wrap_error(error_msg, 1, false);
        let v: serde_json::Value = serde_json::from_str(&result).expect("valid JSON");
        let error_field = v["error"].as_str().expect("error field is string");
        assert!(!error_field.contains("AKIA1234567890ABCDEF"));
        assert!(error_field.contains("****REDACTED****"));
    }
}
