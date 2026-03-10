//! Core types, regex patterns, and error classification for learn/detector.
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
    RE.get_or_init(|| Regex::new(
        r"(?i)(unexpected argument|unknown (option|flag)|unrecognized (option|flag)|invalid (option|flag))"
    ).unwrap())
}

fn cmd_not_found_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
        r"(?i)(command not found|not recognized as an internal|no such file or directory.*command)"
    ).unwrap()
    })
}

fn wrong_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(no such file or directory|cannot find the path|file not found)").unwrap()
    })
}

fn missing_arg_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"(?i)(requires a value|requires an argument|missing (required )?argument|expected.*argument)"
    ).unwrap())
}

fn permission_denied_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(permission denied|access denied|not permitted)").unwrap())
}

// User rejection patterns - NOT actual errors
fn user_rejection_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"(?i)(user (doesn't want|declined|rejected|cancelled)|operation (cancelled|aborted) by user)"
    ).unwrap())
}

/// Filters out user rejections - requires actual error-indicating content.
pub fn is_command_error(is_error: bool, output: &str) -> bool {
    if !is_error {
        return false;
    }

    // Reject if it's a user rejection
    if user_rejection_re().is_match(output) {
        return false;
    }

    // Must contain error-indicating content
    let output_lower = output.to_lowercase();
    output_lower.contains("error")
        || output_lower.contains("failed")
        || output_lower.contains("unknown")
        || output_lower.contains("invalid")
        || output_lower.contains("not found")
        || output_lower.contains("permission denied")
        || output_lower.contains("cannot")
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
