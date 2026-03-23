//! Reads and parses Claude Code and Codex JSONL session files to extract command executions.
mod claude;
mod codex;
mod shared;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::path::{Path, PathBuf};

pub use claude::ClaudeProvider;
pub use codex::CodexProvider;
use shared::project_filter_looks_like_path;

/// A command extracted from a session file.
#[derive(Debug)]
pub struct ExtractedCommand {
    pub command: String,
    pub output_len: Option<usize>,
    #[allow(dead_code)]
    pub session_id: String,
    /// Actual output content (first ~1000 chars for error detection)
    pub output_content: Option<String>,
    /// Whether the tool_result indicated an error
    pub is_error: bool,
    /// Chronological sequence index within the session
    #[allow(dead_code)]
    pub sequence_index: usize,
}

/// Trait for session providers (Claude Code and Codex).
pub trait SessionProvider {
    fn discover_sessions(
        &self,
        project_filter: Option<&str>,
        since_days: Option<u64>,
    ) -> Result<Vec<PathBuf>>;
    fn extract_commands(&self, path: &Path) -> Result<Vec<ExtractedCommand>>;
}

/// Source of session history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSource {
    ClaudeCode,
    CodexCli,
}

impl SessionSource {
    /// Human-readable label for user-facing output.
    pub fn label(self) -> &'static str {
        match self {
            SessionSource::ClaudeCode => "Claude Code",
            SessionSource::CodexCli => "Codex CLI",
        }
    }

    /// Whether this source has a history root on the current machine.
    pub fn is_available(self) -> bool {
        match self {
            SessionSource::ClaudeCode => ClaudeProvider::history_root_exists(),
            SessionSource::CodexCli => CodexProvider::history_root_exists(),
        }
    }
}

/// Return the session sources that are available on this machine.
pub fn available_sources() -> Vec<SessionSource> {
    [SessionSource::ClaudeCode, SessionSource::CodexCli]
        .into_iter()
        .filter(|source| source.is_available())
        .collect()
}

/// Build the default project filter for a given source.
pub fn project_filter_for_source(
    source: SessionSource,
    project: Option<&str>,
    all: bool,
    cwd: &str,
) -> Option<String> {
    if all {
        return None;
    }

    if let Some(project) = project {
        return Some(match source {
            SessionSource::ClaudeCode if project_filter_looks_like_path(project) => {
                ClaudeProvider::encode_project_path(project)
            }
            _ => project.to_string(),
        });
    }

    Some(match source {
        SessionSource::ClaudeCode => ClaudeProvider::encode_project_path(cwd),
        SessionSource::CodexCli => cwd.to_string(),
    })
}

/// Discover sessions for the selected source.
pub fn discover_sessions(
    source: SessionSource,
    project_filter: Option<&str>,
    since_days: Option<u64>,
) -> Result<Vec<PathBuf>> {
    match source {
        SessionSource::ClaudeCode => ClaudeProvider.discover_sessions(project_filter, since_days),
        SessionSource::CodexCli => CodexProvider.discover_sessions(project_filter, since_days),
    }
}

/// Extract commands for the selected source.
pub fn extract_commands(source: SessionSource, path: &Path) -> Result<Vec<ExtractedCommand>> {
    match source {
        SessionSource::ClaudeCode => ClaudeProvider.extract_commands(path),
        SessionSource::CodexCli => CodexProvider.extract_commands(path),
    }
}
