//! Token savings tracking and analytics system.
//!
//! This module provides comprehensive tracking of Mycelium command executions,
//! recording token savings, execution times, and providing aggregation APIs
//! for daily/weekly/monthly statistics.
//!
//! # Architecture
//!
//! - Storage: SQLite database (~/.local/share/mycelium/history.db)
//! - Retention: 90-day automatic cleanup
//! - Metrics: Input/output tokens, savings %, execution time
//!
//! # Quick Start
//!
//! ```no_run
//! use mycelium::tracking::{TimedExecution, Tracker};
//!
//! // Track a command execution
//! let timer = TimedExecution::start();
//! let input = "raw output";
//! let output = "filtered output";
//! timer.track("ls -la", "mycelium ls", input, output);
//!
//! // Query statistics
//! let tracker = Tracker::new().unwrap();
//! let summary = tracker.get_summary_filtered(None).unwrap();
//! println!("Saved {} tokens", summary.total_saved);
//! ```
//!

mod queries;
mod schema;
mod timer;
pub(crate) mod utils;

use anyhow::Result;
use jiff::{SignedDuration, Timestamp};
use rusqlite::{Connection, params};
use serde::Serialize;

use utils::{current_project_path_string, get_db_path};

#[allow(unused_imports)]
pub use queries::ParseHealthRow;
pub use timer::TimedExecution;
pub(crate) use utils::project_filter_params;
#[allow(unused_imports)]
pub use utils::{
    DbPathInfo, DbPathSource, args_display, estimate_tokens, record_parse_failure_silent,
    resolve_db_path_info,
};

/// Number of days to retain tracking history before automatic cleanup.
const HISTORY_DAYS: i64 = 90;

/// Main tracking interface for recording and querying command history.
///
/// Manages SQLite database connection and provides methods for:
/// - Recording command executions with token counts and timing
/// - Querying aggregated statistics (summary, daily, weekly, monthly)
/// - Retrieving recent command history
///
/// # Database Location
///
/// - Linux: `~/.local/share/mycelium/history.db`
/// - macOS: `~/Library/Application Support/mycelium/history.db`
/// - Windows: `%APPDATA%\mycelium\history.db`
///
/// # Examples
///
/// ```no_run
/// use mycelium::tracking::Tracker;
///
/// let tracker = Tracker::new()?;
/// tracker.record("ls -la", "mycelium ls", 1000, 200, 50)?;
///
/// let summary = tracker.get_summary_filtered(None)?;
/// println!("Total saved: {} tokens", summary.total_saved);
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct Tracker {
    pub(crate) conn: Connection,
}

/// Individual command record from tracking history.
///
/// Contains timestamp, command name, and savings metrics for a single execution.
#[derive(Debug)]
pub struct CommandRecord {
    /// UTC timestamp when command was executed
    pub timestamp: Timestamp,
    /// Mycelium command that was executed (e.g., "mycelium ls")
    pub mycelium_cmd: String,
    /// Number of tokens saved (input - output)
    pub saved_tokens: usize,
    /// Savings percentage ((saved / input) * 100)
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
    /// Last 30 days of activity: (date, saved_tokens)
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
/// Weeks start on Sunday (SQLite default).
#[derive(Debug, Serialize)]
pub struct WeekStats {
    /// Week start date (YYYY-MM-DD)
    pub week_start: String,
    /// Week end date (YYYY-MM-DD)
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
    /// Month identifier (YYYY-MM)
    pub month: String,
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
#[derive(Debug, Clone)]
pub struct CommandStats {
    /// The Mycelium command (e.g., "mycelium ls", "mycelium gh pr view")
    pub command: String,
    /// Number of times this command was executed
    pub count: usize,
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
#[allow(dead_code)]
pub struct ParseFailureRecord {
    pub timestamp: String,
    pub raw_command: String,
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

impl Tracker {
    /// Create a new tracker instance.
    ///
    /// Opens or creates the SQLite database at the platform-specific location.
    /// Automatically creates the `commands` table if it doesn't exist and runs
    /// any necessary schema migrations.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Cannot determine database path
    /// - Cannot create parent directories
    /// - Cannot open/create SQLite database
    /// - Schema creation/migration fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new() -> Result<Self> {
        Self::new_with_override(None)
    }

    pub fn new_with_override(override_path: Option<&str>) -> Result<Self> {
        let db_path = get_db_path(override_path)?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        schema::init_schema(&conn)?;

        Ok(Self { conn })
    }

    /// Record a command execution with token counts and timing.
    ///
    /// Calculates savings metrics and stores the record in the database.
    /// Automatically cleans up records older than 90 days after insertion.
    ///
    /// # Arguments
    ///
    /// - `original_cmd`: The standard command (e.g., "ls -la")
    /// - `mycelium_cmd`: The Mycelium command used (e.g., "mycelium ls")
    /// - `input_tokens`: Estimated tokens from standard command output
    /// - `output_tokens`: Actual tokens from Mycelium output
    /// - `exec_time_ms`: Execution time in milliseconds
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// tracker.record("ls -la", "mycelium ls", 1000, 200, 50)?;
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn record(
        &self,
        original_cmd: &str,
        mycelium_cmd: &str,
        input_tokens: usize,
        output_tokens: usize,
        exec_time_ms: u64,
    ) -> Result<()> {
        let saved = input_tokens.saturating_sub(output_tokens);
        let pct = if input_tokens > 0 {
            (saved as f64 / input_tokens as f64) * 100.0
        } else {
            0.0
        };

        let project_path = current_project_path_string();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Timestamp::now().to_string(),
                original_cmd,
                mycelium_cmd,
                project_path,
                input_tokens as i64,
                output_tokens as i64,
                saved as i64,
                pct,
                exec_time_ms as i64,
                "filtered",
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    /// Record a command execution with parse tier and format mode tracking.
    ///
    /// Use this for commands that use the parser framework (OutputParser trait).
    /// Legacy commands should continue using `record()`.
    ///
    /// # Arguments
    ///
    /// - `parse_tier`: Parser result tier (1=Full, 2=Degraded, 3=Passthrough, 0=legacy)
    /// - `format_mode`: Format mode used ("compact", "verbose", "ultra", or "")
    #[allow(clippy::too_many_arguments, dead_code)]
    pub fn record_with_parse_info(
        &self,
        original_cmd: &str,
        mycelium_cmd: &str,
        input_tokens: usize,
        output_tokens: usize,
        exec_time_ms: u64,
        parse_tier: u8,
        format_mode: &str,
    ) -> Result<()> {
        let saved = input_tokens.saturating_sub(output_tokens);
        let pct = if input_tokens > 0 {
            (saved as f64 / input_tokens as f64) * 100.0
        } else {
            0.0
        };

        let project_path = current_project_path_string();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, parse_tier, format_mode, execution_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                Timestamp::now().to_string(),
                original_cmd,
                mycelium_cmd,
                project_path,
                input_tokens as i64,
                output_tokens as i64,
                saved as i64,
                pct,
                exec_time_ms as i64,
                parse_tier as i64,
                format_mode,
                "filtered",
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    /// Record a passthrough command execution with timing only.
    pub fn record_passthrough(
        &self,
        original_cmd: &str,
        mycelium_cmd: &str,
        exec_time_ms: u64,
    ) -> Result<()> {
        let project_path = current_project_path_string();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind)
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 0.0, ?5, 'passthrough')",
            params![
                Timestamp::now().to_string(),
                original_cmd,
                mycelium_cmd,
                project_path,
                exec_time_ms as i64,
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    fn cleanup_old(&self) -> Result<()> {
        let cutoff = Timestamp::now()
            .checked_sub(SignedDuration::from_hours(HISTORY_DAYS * 24))
            .map_err(|e| anyhow::anyhow!("timestamp overflow in cleanup: {}", e))?;
        self.conn.execute(
            "DELETE FROM commands WHERE timestamp < ?1",
            params![cutoff.to_string()],
        )?;
        self.conn.execute(
            "DELETE FROM parse_failures WHERE timestamp < ?1",
            params![cutoff.to_string()],
        )?;
        Ok(())
    }

    /// Record a parse failure for analytics.
    pub fn record_parse_failure(
        &self,
        raw_command: &str,
        error_message: &str,
        fallback_succeeded: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO parse_failures (timestamp, raw_command, error_message, fallback_succeeded)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                Timestamp::now().to_string(),
                raw_command,
                error_message,
                fallback_succeeded as i32,
            ],
        )?;
        self.cleanup_old()?;
        Ok(())
    }

    /// Get parse failure summary for `mycelium gain --failures`.
    pub fn get_parse_failure_summary(&self) -> Result<ParseFailureSummary> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM parse_failures", [], |row| row.get(0))?;

        let succeeded: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM parse_failures WHERE fallback_succeeded = 1",
            [],
            |row| row.get(0),
        )?;

        let recovery_rate = if total > 0 {
            (succeeded as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Top commands by frequency
        let mut stmt = self.conn.prepare(
            "SELECT raw_command, COUNT(*) as cnt
             FROM parse_failures
             GROUP BY raw_command
             ORDER BY cnt DESC
             LIMIT 10",
        )?;
        let top_commands = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Recent 10
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, raw_command, error_message, fallback_succeeded
             FROM parse_failures
             ORDER BY timestamp DESC
             LIMIT 10",
        )?;
        let recent = stmt
            .query_map([], |row| {
                Ok(ParseFailureRecord {
                    timestamp: row.get(0)?,
                    raw_command: row.get(1)?,
                    error_message: row.get(2)?,
                    fallback_succeeded: row.get::<_, i32>(3)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ParseFailureSummary {
            total: total as usize,
            recovery_rate,
            top_commands,
            recent,
        })
    }
}

#[cfg(test)]
mod tests;
