//! Standalone utility functions for the tracking module.
//!
//! These are free functions (not methods on `Tracker`) used across the module
//! and by the rest of the crate.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Result;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Get the canonical project path string for the current working directory.
pub(super) fn current_project_path_string() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Resolve the SQLite database path.
///
/// Priority:
/// 1. `MYCELIUM_DB_PATH` environment variable
/// 2. `tracking.database_path` from `~/.config/mycelium/config.toml`
/// 3. Platform-specific default (`~/.local/share/mycelium/history.db` on Linux)
pub(super) fn get_db_path() -> Result<PathBuf> {
    if let Ok(custom_path) = std::env::var("MYCELIUM_DB_PATH") {
        return Ok(PathBuf::from(custom_path));
    }

    if let Ok(config) = crate::config::Config::load()
        && let Some(db_path) = config.tracking.database_path
    {
        return Ok(db_path);
    }

    let data_dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    Ok(data_dir.join("mycelium").join("history.db"))
}

// ── Crate-visible helpers ─────────────────────────────────────────────────────

/// Build SQL filter params for project-scoped queries.
///
/// Returns `(exact_match, glob_prefix)` for use in a WHERE clause of the form:
/// `WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)`
///
/// Uses `GLOB` instead of `LIKE` so that `_` and `%` in paths are treated
/// as literal characters rather than wildcard patterns.
pub(crate) fn project_filter_params(
    project_path: Option<&str>,
) -> (Option<String>, Option<String>) {
    match project_path {
        Some(p) => (
            Some(p.to_string()),
            Some(format!("{}{}*", p, std::path::MAIN_SEPARATOR)),
        ),
        None => (None, None),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Record a parse failure without ever crashing.
///
/// Silently ignores all errors — intended for use in fallback paths where a
/// secondary failure must not interrupt the primary command execution.
pub fn record_parse_failure_silent(raw_command: &str, error_message: &str, succeeded: bool) {
    if let Ok(tracker) = super::Tracker::new() {
        let _ = tracker.record_parse_failure(raw_command, error_message, succeeded);
    }
}

/// Estimate token count from text using the ~4 chars = 1 token heuristic.
///
/// This is a fast approximation suitable for tracking purposes.
/// For precise counts, integrate with your LLM's tokenizer API.
///
/// # Formula
///
/// `tokens = ceil(chars / 4)`
///
/// # Examples
///
/// ```
/// use mycelium::tracking::estimate_tokens;
///
/// assert_eq!(estimate_tokens(""), 0);
/// assert_eq!(estimate_tokens("abcd"), 1);  // 4 chars = 1 token
/// assert_eq!(estimate_tokens("abcde"), 2); // 5 chars = ceil(1.25) = 2
/// assert_eq!(estimate_tokens("hello world"), 3); // 11 chars = ceil(2.75) = 3
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize
}

/// Format OsString args for tracking display.
///
/// Joins arguments with spaces, converting each to UTF-8 (lossy).
///
/// # Examples
///
/// ```
/// use std::ffi::OsString;
/// use mycelium::tracking::args_display;
///
/// let args = vec![OsString::from("status"), OsString::from("--short")];
/// assert_eq!(args_display(&args), "status --short");
/// ```
pub fn args_display(args: &[OsString]) -> String {
    args.iter()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}
