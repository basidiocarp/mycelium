//! Types and classification logic for CLI error detection.
use regex::Regex;
use std::sync::OnceLock;

/// Classification of CLI error types detected in command output.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorType {
    UnknownFlag,
    CommandNotFound,
    WrongPath,
    MissingArg,
    PermissionDenied,
    Other(String),
}

impl ErrorType {
    /// Return a human-readable label for this error type.
    pub fn as_str(&self) -> &str {
        match self {
            ErrorType::UnknownFlag => "Unknown Flag",
            ErrorType::CommandNotFound => "Command Not Found",
            ErrorType::WrongPath => "Wrong Path",
            ErrorType::MissingArg => "Missing Argument",
            ErrorType::PermissionDenied => "Permission Denied",
            ErrorType::Other(s) => s,
        }
    }
}

/// A detected wrong-command / right-command pair with confidence score.
#[derive(Debug, Clone)]
pub struct CorrectionPair {
    pub wrong_command: String,
    pub right_command: String,
    pub error_output: String,
    pub error_type: ErrorType,
    pub confidence: f64,
}

/// A deduplicated correction rule aggregated from one or more `CorrectionPair` instances.
#[derive(Debug, Clone)]
pub struct CorrectionRule {
    pub wrong_pattern: String,
    pub right_pattern: String,
    pub error_type: ErrorType,
    pub occurrences: usize,
    pub base_command: String,
    pub example_error: String,
}

fn unknown_flag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(unexpected argument|unknown (option|flag)|unrecognized (option|flag)|invalid (option|flag))",
        )
        .expect("regex: invalid flag pattern")
    })
}

fn cmd_not_found_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(command not found|not recognized as an internal|no such file or directory.*command)",
        )
        .expect("regex: command not found pattern")
    })
}

fn wrong_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(no such file or directory|cannot find the path|file not found)")
            .expect("valid regex")
    })
}

fn missing_arg_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(requires a value|requires an argument|missing (required )?argument|expected.*argument)",
        )
        .expect("regex: missing argument pattern")
    })
}

fn permission_denied_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(permission denied|access denied|not permitted)").expect("valid regex")
    })
}

fn user_rejection_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(user (doesn't want|declined|rejected|cancelled)|operation (cancelled|aborted) by user)",
        )
        .expect("regex: user declined pattern")
    })
}

/// Filters out user rejections - requires actual error-indicating content.
pub fn is_command_error(is_error: bool, output: &str) -> bool {
    if !is_error {
        return false;
    }

    if user_rejection_re().is_match(output) {
        return false;
    }

    let output_lower = output.to_lowercase();
    output_lower.contains("error")
        || output_lower.contains("failed")
        || output_lower.contains("unknown")
        || output_lower.contains("invalid")
        || output_lower.contains("not found")
        || output_lower.contains("permission denied")
        || output_lower.contains("cannot")
}

/// Check if error is a compilation/test error (TDD cycle, not CLI correction).
pub fn is_tdd_cycle_error(_error_type: &ErrorType, output: &str) -> bool {
    if output.contains("error[E") || output.contains("aborting due to") {
        return true;
    }

    if output.contains("test result: FAILED") {
        return true;
    }

    if output.contains("short test summary info")
        || (output.contains("FAILED ") && output.contains("::"))
    {
        return true;
    }

    if output.contains("Traceback (most recent call last)")
        || output.contains("SyntaxError:")
        || output.contains("IndentationError:")
        || output.contains("NameError:")
        || output.contains("TypeError:")
        || output.contains("AttributeError:")
    {
        return true;
    }

    if (output.contains("FAIL ") || output.contains("Tests failed:"))
        && (output.contains(".test.") || output.contains(".spec."))
    {
        return true;
    }

    if output.contains("npm ERR! Test failed") || output.contains("ERR_PNPM_RECURSIVE_FAIL") {
        return true;
    }

    if output.contains("--- FAIL:") || (output.contains("FAIL\t") && output.contains("---")) {
        return true;
    }

    false
}

/// Classify command output into a specific error type using regex patterns.
pub fn classify_error(output: &str) -> ErrorType {
    if unknown_flag_re().is_match(output) {
        ErrorType::UnknownFlag
    } else if cmd_not_found_re().is_match(output) {
        ErrorType::CommandNotFound
    } else if missing_arg_re().is_match(output) {
        ErrorType::MissingArg
    } else if permission_denied_re().is_match(output) {
        ErrorType::PermissionDenied
    } else if wrong_path_re().is_match(output) {
        ErrorType::WrongPath
    } else {
        ErrorType::Other("General Error".to_string())
    }
}

fn env_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(?:sudo\s+|[A-Z_][A-Z0-9_]*=[^\s]*\s+)+").expect("valid regex"))
}

/// Extract base command (first 1-2 tokens, stripping any KEY=VALUE env prefixes and sudo).
pub fn extract_base_command(cmd: &str) -> String {
    let trimmed = cmd.trim();
    let stripped = env_prefix_re().replace(trimmed, "");
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => format!("{} {}", parts[0], parts[1]),
    }
}

/// Calculate similarity between two commands using Jaccard similarity.
/// Same base command = 0.5 base score + up to 0.5 from argument similarity.
pub fn command_similarity(a: &str, b: &str) -> f64 {
    let base_a = extract_base_command(a);
    let base_b = extract_base_command(b);

    if base_a != base_b {
        return 0.0;
    }

    let args_a: std::collections::HashSet<&str> = a
        .strip_prefix(&base_a)
        .unwrap_or("")
        .split_whitespace()
        .collect();

    let args_b: std::collections::HashSet<&str> = b
        .strip_prefix(&base_b)
        .unwrap_or("")
        .split_whitespace()
        .collect();

    if args_a.is_empty() && args_b.is_empty() {
        return 1.0;
    }

    let intersection = args_a.intersection(&args_b).count();
    let union = args_a.union(&args_b).count();

    if union == 0 {
        return 0.5;
    }

    0.5 + (intersection as f64 / union as f64) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_command_error_requires_error_flag() {
        assert!(!is_command_error(false, "error: unknown flag"));
        assert!(is_command_error(true, "error: unknown flag"));
    }

    #[test]
    fn test_is_command_error_filters_user_rejection() {
        assert!(!is_command_error(true, "The user doesn't want to proceed"));
        assert!(!is_command_error(true, "Operation cancelled by user"));
        assert!(is_command_error(true, "error: permission denied"));
    }

    #[test]
    fn test_is_command_error_requires_error_content() {
        assert!(!is_command_error(true, "All good, success!"));
        assert!(is_command_error(true, "error: something failed"));
        assert!(is_command_error(true, "unknown flag --foo"));
        assert!(is_command_error(true, "invalid option"));
    }

    #[test]
    fn test_classify_error_unknown_flag() {
        assert_eq!(
            classify_error("error: unexpected argument '--foo'"),
            ErrorType::UnknownFlag
        );
        assert_eq!(
            classify_error("unknown option: --bar"),
            ErrorType::UnknownFlag
        );
        assert_eq!(
            classify_error("unrecognized flag: -x"),
            ErrorType::UnknownFlag
        );
    }

    #[test]
    fn test_classify_error_command_not_found() {
        assert_eq!(
            classify_error("bash: foobar: command not found"),
            ErrorType::CommandNotFound
        );
        assert_eq!(
            classify_error("'xyz' is not recognized as an internal or external command"),
            ErrorType::CommandNotFound
        );
    }

    #[test]
    fn test_classify_error_all_types() {
        assert_eq!(
            classify_error("No such file or directory: foo.txt"),
            ErrorType::WrongPath
        );
        assert_eq!(
            classify_error("error: --output requires a value"),
            ErrorType::MissingArg
        );
        assert_eq!(
            classify_error("permission denied: /etc/shadow"),
            ErrorType::PermissionDenied
        );
        assert!(matches!(
            classify_error("something went wrong"),
            ErrorType::Other(_)
        ));
    }

    #[test]
    fn test_extract_base_command() {
        assert_eq!(extract_base_command("git commit"), "git commit");
        assert_eq!(extract_base_command("cargo test"), "cargo test");
        assert_eq!(
            extract_base_command("git commit --amend -m 'fix'"),
            "git commit"
        );
        assert_eq!(
            extract_base_command("RUST_BACKTRACE=1 cargo test"),
            "cargo test"
        );
        assert_eq!(
            extract_base_command("NODE_ENV=test CI=1 npx vitest run"),
            "npx vitest"
        );
        assert_eq!(
            extract_base_command("GIT_PAGER=cat git log --oneline"),
            "git log"
        );
        assert_eq!(
            extract_base_command("sudo cargo install foo"),
            "cargo install"
        );
    }

    #[test]
    fn test_command_similarity_same_base() {
        assert_eq!(command_similarity("git commit", "git commit"), 1.0);
        assert_eq!(command_similarity("git status", "npm install"), 0.0);
        let sim = command_similarity("git commit --amend", "git commit --ammend");
        // Same base (0.5) + both have 1 arg, 0 intersection = 0.5 + 0 = 0.5
        assert_eq!(sim, 0.5);
    }
}
