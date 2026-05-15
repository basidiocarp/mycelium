//! Type definitions for tracking system.

use serde::Serialize;

/// Number of days to retain tracking history before automatic cleanup.
pub const HISTORY_DAYS: i64 = 90;

/// Individual command record from tracking history.
///
/// Contains timestamp, command name, and savings metrics for a single execution.
#[derive(Debug)]
pub struct CommandRecord {
    /// UTC timestamp when command was executed
    pub timestamp: String,
    /// Mycelium command that was executed (e.g., "mycelium ls")
    pub mycelium_cmd: String,
    /// Number of tokens saved (input - output)
    pub saved_tokens: usize,
    /// Savings percentage ((saved / input) * 100)
    pub savings_pct: f64,
}

/// Detailed command history record for CLI JSON export and dashboard consumers.
#[derive(Debug, Clone, Serialize)]
pub struct DetailedCommandRecord {
    /// UTC timestamp when the command was executed.
    pub timestamp: String,
    /// The Mycelium command that was executed (e.g., "mycelium ls").
    pub command: String,
    /// Canonical project path captured when the command was recorded.
    pub project_path: String,
    /// Runtime session identifier propagated from the calling agent when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Estimated input tokens before Mycelium filtering.
    pub input_tokens: usize,
    /// Output tokens after Mycelium filtering.
    pub output_tokens: usize,
    /// Number of tokens saved (input - output).
    pub saved_tokens: usize,
    /// Savings percentage ((saved / input) * 100).
    pub savings_pct: f64,
}

/// Aggregated statistics across all recorded commands.
///
/// Provides overall metrics and breakdowns by command and by day.
/// Returned by [`Tracker::get_summary_filtered`].
#[derive(Debug)]
pub struct GainSummary {
    /// Total number of commands recorded
    pub total_commands: usize,
    /// Total input tokens across all commands
    pub total_input: usize,
    /// Total output tokens across all commands
    pub total_output: usize,
    /// Total tokens saved (input - output)
    pub total_saved: usize,
    /// Average savings percentage across all commands
    pub avg_savings_pct: f64,
    /// Total execution time across all commands (milliseconds)
    pub total_time_ms: u64,
    /// Average execution time per command (milliseconds)
    pub avg_time_ms: u64,
    /// Top 10 commands by tokens saved
    pub by_command: Vec<CommandStats>,
    /// Last 30 days of activity: (date, `saved_tokens`)
    pub by_day: Vec<(String, usize)>,
}

/// Daily statistics for token savings and execution metrics.
///
/// Serializable to JSON for export via `mycelium gain --daily --format json`.
///
/// # JSON Schema
///
/// ```json
/// {
///   "date": "2026-02-03",
///   "commands": 42,
///   "input_tokens": 15420,
///   "output_tokens": 3842,
///   "saved_tokens": 11578,
///   "savings_pct": 75.08,
///   "total_time_ms": 8450,
///   "avg_time_ms": 201
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct DayStats {
    /// ISO date (YYYY-MM-DD)
    pub date: String,
    /// Number of commands executed this day
    pub commands: usize,
    /// Total input tokens for this day
    pub input_tokens: usize,
    /// Total output tokens for this day
    pub output_tokens: usize,
    /// Total tokens saved this day
    pub saved_tokens: usize,
    /// Savings percentage for this day
    pub savings_pct: f64,
    /// Total execution time for this day (milliseconds)
    pub total_time_ms: u64,
    /// Average execution time per command (milliseconds)
    pub avg_time_ms: u64,
}

/// Weekly statistics for token savings and execution metrics.
///
/// Serializable to JSON for export via `mycelium gain --weekly --format json`.
/// Weeks start on Sunday (`SQLite` default).
#[derive(Debug, Serialize)]
pub struct WeekStats {
    /// ISO week start date (YYYY-MM-DD)
    pub date: String,
    /// Week end date (YYYY-MM-DD) - internal use only
    #[serde(skip_serializing)]
    pub week_end: String,
    /// Number of commands executed this week
    pub commands: usize,
    /// Total input tokens for this week
    pub input_tokens: usize,
    /// Total output tokens for this week
    pub output_tokens: usize,
    /// Total tokens saved this week
    pub saved_tokens: usize,
    /// Savings percentage for this week
    pub savings_pct: f64,
    /// Total execution time for this week (milliseconds)
    pub total_time_ms: u64,
    /// Average execution time per command (milliseconds)
    pub avg_time_ms: u64,
}

/// Monthly statistics for token savings and execution metrics.
///
/// Serializable to JSON for export via `mycelium gain --monthly --format json`.
#[derive(Debug, Serialize)]
pub struct MonthStats {
    /// ISO month start date (YYYY-MM-01)
    pub date: String,
    /// Number of commands executed this month
    pub commands: usize,
    /// Total input tokens for this month
    pub input_tokens: usize,
    /// Total output tokens for this month
    pub output_tokens: usize,
    /// Total tokens saved this month
    pub saved_tokens: usize,
    /// Savings percentage for this month
    pub savings_pct: f64,
    /// Total execution time for this month (milliseconds)
    pub total_time_ms: u64,
    /// Average execution time per command (milliseconds)
    pub avg_time_ms: u64,
}

/// Per-project aggregated statistics for the `--projects` breakdown table.
#[derive(Debug, Serialize)]
pub struct ProjectStats {
    /// Canonical project directory path
    pub project_path: String,
    /// Human-readable project name (from `BASIDIOCARP_PROJECT`, git remote, or directory name)
    pub project_name: String,
    /// Total commands executed in this project
    pub commands: i64,
    /// Total tokens saved in this project
    pub saved_tokens: i64,
    /// Average savings percentage across commands
    pub avg_savings_pct: f64,
    /// ISO timestamp of most recent command in this project
    pub last_used: String,
}

/// Statistics for a single command aggregated across all executions.
///
/// Used in `GainSummary::by_command` to break down token savings by command type.
#[derive(Debug, Clone, Serialize)]
pub struct CommandStats {
    /// The Mycelium command (e.g., "mycelium ls", "mycelium gh pr view")
    pub command: String,
    /// Number of times this command was executed
    pub count: usize,
    /// Total input tokens across all executions of this command.
    pub input_tokens: usize,
    /// Total tokens saved across all executions of this command
    pub tokens_saved: usize,
    /// Average savings percentage for this command
    pub savings_pct: f64,
    /// Average execution time in milliseconds
    pub exec_time_ms: u64,
}

/// Aggregated passthrough usage statistics.
#[derive(Debug, Clone)]
pub struct PassthroughSummary {
    /// Number of passthrough command executions recorded.
    pub total_commands: usize,
    /// Total passthrough execution time in milliseconds.
    pub total_exec_time_ms: u64,
    /// Top passthrough commands by frequency.
    pub top_commands: Vec<PassthroughCommandStat>,
}

/// Statistics for a single passthrough command.
#[derive(Debug, Clone)]
pub struct PassthroughCommandStat {
    /// Original raw command that Mycelium passed through.
    pub command: String,
    /// Number of times the command ran in passthrough mode.
    pub count: usize,
    /// Total passthrough execution time in milliseconds.
    pub total_exec_time_ms: u64,
}

/// Individual parse failure record.
#[derive(Debug)]
pub struct ParseFailureRecord {
    pub timestamp: String,
    pub raw_command: String,
    #[allow(
        dead_code,
        reason = "Failure detail is surfaced by reporting consumers outside the bin target"
    )]
    pub error_message: String,
    pub fallback_succeeded: bool,
}

/// Aggregated parse failure summary.
#[derive(Debug)]
pub struct ParseFailureSummary {
    pub total: usize,
    pub recovery_rate: f64,
    pub top_commands: Vec<(String, usize)>,
    pub recent: Vec<ParseFailureRecord>,
}
