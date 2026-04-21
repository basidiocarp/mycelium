//! Standalone utility functions for the tracking module.
//!
//! These are free functions (not methods on `Tracker`) used across the module
//! and by the rest of the crate.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use anyhow::Result;
use spore::logging::{SpanContext, tool_span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbPathSource {
    Override,
    Environment,
    Config,
    Default,
}

impl fmt::Display for DbPathSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Override => write!(f, "override"),
            Self::Environment => write!(f, "MYCELIUM_DB_PATH"),
            Self::Config => write!(f, "config"),
            Self::Default => write!(f, "default"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DbPathInfo {
    pub path: PathBuf,
    pub source: DbPathSource,
    pub config_path: PathBuf,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Get the canonical project path string for the current working directory.
pub(super) fn current_project_path_string() -> String {
    if let Ok(path) = std::env::var("MYCELIUM_PROJECT_PATH")
        && !path.trim().is_empty()
    {
        return canonicalize_project_path(path);
    }

    std::env::current_dir()
        .ok()
        .map(canonicalize_pathbuf)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub(super) fn current_runtime_session_id() -> Option<String> {
    spore::claude_session_id()
}

/// Detect the git repository root for the current working directory.
/// Returns an empty string if not inside a git repository.
pub(super) fn current_project_root() -> String {
    std::env::current_dir()
        .ok()
        .map(canonicalize_pathbuf)
        .and_then(|canonical| {
            canonical
                .ancestors()
                .find(|candidate| candidate.join(".git").exists())
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_default()
}

fn canonicalize_project_path(path: String) -> String {
    canonicalize_pathbuf(PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}

fn canonicalize_pathbuf(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

/// Resolve the SQLite database path.
///
/// Priority:
/// 1. `override_path` argument (used in tests to avoid mutating the environment)
/// 2. `MYCELIUM_DB_PATH` environment variable
/// 3. `tracking.database_path` from `~/.config/mycelium/config.toml`
/// 4. Platform-specific default data directory
pub fn resolve_db_path_info(override_path: Option<&str>) -> Result<DbPathInfo> {
    let config_path = crate::config::config_path()?;

    if let Some(path) = override_path {
        return Ok(DbPathInfo {
            path: PathBuf::from(path),
            source: DbPathSource::Override,
            config_path,
        });
    }

    if let Ok(custom_path) = std::env::var("MYCELIUM_DB_PATH")
        && !custom_path.trim().is_empty()
    {
        return Ok(DbPathInfo {
            path: PathBuf::from(custom_path),
            source: DbPathSource::Environment,
            config_path,
        });
    }

    if let Ok(config) = crate::config::Config::load()
        && let Some(db_path) = config.tracking.database_path
    {
        return Ok(DbPathInfo {
            path: db_path,
            source: DbPathSource::Config,
            config_path,
        });
    }

    let data_dir =
        crate::platform::mycelium_data_dir().unwrap_or_else(|| PathBuf::from(".").join("mycelium"));
    Ok(DbPathInfo {
        path: data_dir.join("history.db"),
        source: DbPathSource::Default,
        config_path,
    })
}

pub(super) fn get_db_path(override_path: Option<&str>) -> Result<PathBuf> {
    Ok(resolve_db_path_info(override_path)?.path)
}

// ── Crate-visible helpers ─────────────────────────────────────────────────────

/// Build SQL filter params for project-scoped queries.
///
/// Returns `(exact_match, glob_prefix)` for use in a WHERE clause of the form:
/// `WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)`
///
/// Uses `GLOB` instead of `LIKE` so that `_` and `%` in paths are treated
/// as literal characters rather than wildcard patterns.
///
/// The glob pattern is properly escaped to prevent injection of glob metacharacters.
///
/// # Security Note
///
/// Path traversal risk: SQL-only. The project path is used only in parameterized SQL
/// GLOB queries, never in filesystem operations. `escape_glob_pattern` ensures
/// GLOB metacharacters cannot inject unintended query semantics.
pub(crate) fn project_filter_params(
    project_path: Option<&str>,
) -> (Option<String>, Option<String>) {
    match project_path {
        Some(p) => {
            // Escape GLOB metacharacters: *, ?, [, ]
            // We need to escape these so they are treated as literals in the GLOB pattern.
            let escaped = escape_glob_pattern(p);
            (
                Some(p.to_string()),
                Some(format!("{}{}*", escaped, std::path::MAIN_SEPARATOR)),
            )
        }
        None => (None, None),
    }
}

/// Escape special characters in a path for use in SQLite GLOB patterns.
///
/// GLOB patterns use *, ?, [, and ] as metacharacters. This function escapes
/// them so they are treated as literal characters.
fn escape_glob_pattern(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            '*' | '?' | '[' | ']' => format!("[{}]", c),
            _ => c.to_string(),
        })
        .collect()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Record a parse failure without ever crashing.
///
/// Silently ignores all errors — intended for use in fallback paths where a
/// secondary failure must not interrupt the primary command execution.
///
/// If `tracker` is provided, it will be reused. Otherwise, a new Tracker will
/// be created. This allows callers with an existing tracker to avoid creating
/// a second one for consistency and performance.
pub fn record_parse_failure_silent(
    raw_command: &str,
    error_message: &str,
    succeeded: bool,
    tracker: Option<&super::Tracker>,
) {
    let _tool_span = tool_span("tracking_parse_failure", &span_context(raw_command)).entered();
    let owned_tracker;
    let tracker_ref = if let Some(t) = tracker {
        t
    } else {
        match super::Tracker::new() {
            Ok(t) => {
                owned_tracker = t;
                &owned_tracker
            }
            Err(_) => return,
        }
    };

    let _ = tracker_ref.record_parse_failure(raw_command, error_message, succeeded);
}

fn span_context(command: &str) -> SpanContext {
    let context = SpanContext::for_app("mycelium").with_tool(command.to_string());
    match std::env::current_dir() {
        Ok(path) => context.with_workspace_root(path.display().to_string()),
        Err(_) => context,
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
    spore::tokens::estimate(text)
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
