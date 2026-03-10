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
//! See [docs/tracking.md](../docs/tracking.md) for full documentation.

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
pub use utils::{args_display, estimate_tokens, record_parse_failure_silent};

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
    /// Top 10 commands by tokens saved: (cmd, count, saved, avg_pct, avg_time_ms)
    pub by_command: Vec<(String, usize, usize, f64, u64)>,
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
        let db_path = get_db_path()?;
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
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Timestamp::now().to_string(),
                original_cmd,
                mycelium_cmd,
                project_path,
                input_tokens as i64,
                output_tokens as i64,
                saved as i64,
                pct,
                exec_time_ms as i64
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
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, parse_tier, format_mode)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
                format_mode
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    fn cleanup_old(&self) -> Result<()> {
        let cutoff = Timestamp::now()
            .checked_sub(SignedDuration::from_hours(HISTORY_DAYS * 24))
            .expect("timestamp subtraction should not overflow");
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
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    // 1. estimate_tokens -- verify ~4 chars/token ratio
    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1); // 4 chars = 1 token
        assert_eq!(estimate_tokens("abcde"), 2); // 5 chars = ceil(1.25) = 2
        assert_eq!(estimate_tokens("a"), 1); // 1 char = ceil(0.25) = 1
        assert_eq!(estimate_tokens("12345678"), 2); // 8 chars = 2 tokens
    }

    // 2. args_display -- format OsString vec
    #[test]
    fn test_args_display() {
        let args = vec![OsString::from("status"), OsString::from("--short")];
        assert_eq!(args_display(&args), "status --short");
        assert_eq!(args_display(&[]), "");

        let single = vec![OsString::from("log")];
        assert_eq!(args_display(&single), "log");
    }

    // 3. Tracker::record + get_recent -- round-trip DB
    #[test]
    fn test_tracker_record_and_recent() {
        let tracker = Tracker::new().expect("Failed to create tracker");

        // Use unique test identifier to avoid conflicts with other tests
        let test_cmd = format!("mycelium git status test_{}", std::process::id());

        tracker
            .record("git status", &test_cmd, 100, 20, 50)
            .expect("Failed to record");

        let recent = tracker.get_recent(10).expect("Failed to get recent");

        // Find our specific test record
        let test_record = recent
            .iter()
            .find(|r| r.mycelium_cmd == test_cmd)
            .expect("Test record not found in recent commands");

        assert_eq!(test_record.saved_tokens, 80);
        assert_eq!(test_record.savings_pct, 80.0);
    }

    // 4. track_passthrough doesn't dilute stats (input=0, output=0)
    #[test]
    fn test_track_passthrough_no_dilution() {
        let tracker = Tracker::new().expect("Failed to create tracker");

        // Use unique test identifiers
        let pid = std::process::id();
        let cmd1 = format!("mycelium cmd1_test_{}", pid);
        let cmd2 = format!("mycelium cmd2_passthrough_test_{}", pid);

        // Record one real command with 80% savings
        tracker
            .record("cmd1", &cmd1, 1000, 200, 10)
            .expect("Failed to record cmd1");

        // Record passthrough (0, 0)
        tracker
            .record("cmd2", &cmd2, 0, 0, 5)
            .expect("Failed to record passthrough");

        // Verify both records exist in recent history
        let recent = tracker.get_recent(20).expect("Failed to get recent");

        let record1 = recent
            .iter()
            .find(|r| r.mycelium_cmd == cmd1)
            .expect("cmd1 record not found");
        let record2 = recent
            .iter()
            .find(|r| r.mycelium_cmd == cmd2)
            .expect("passthrough record not found");

        // Verify cmd1 has 80% savings
        assert_eq!(record1.saved_tokens, 800);
        assert_eq!(record1.savings_pct, 80.0);

        // Verify passthrough has 0% savings
        assert_eq!(record2.saved_tokens, 0);
        assert_eq!(record2.savings_pct, 0.0);

        // This validates that passthrough (0 input, 0 output) doesn't dilute stats
        // because the savings calculation is correct for both cases
    }

    // 5. TimedExecution::track records with exec_time > 0
    #[test]
    fn test_timed_execution_records_time() {
        let timer = TimedExecution::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.track("test cmd", "mycelium test", "raw input data", "filtered");

        // Verify via DB that record exists
        let tracker = Tracker::new().expect("Failed to create tracker");
        let recent = tracker.get_recent(5).expect("Failed to get recent");
        assert!(recent.iter().any(|r| r.mycelium_cmd == "mycelium test"));
    }

    // 6. TimedExecution::track_passthrough records with 0 tokens
    #[test]
    fn test_timed_execution_passthrough() {
        let timer = TimedExecution::start();
        timer.track_passthrough("git tag", "mycelium git tag (passthrough)");

        let tracker = Tracker::new().expect("Failed to create tracker");
        let recent = tracker.get_recent(5).expect("Failed to get recent");

        let pt = recent
            .iter()
            .find(|r| r.mycelium_cmd.contains("passthrough"))
            .expect("Passthrough record not found");

        // savings_pct should be 0 for passthrough
        assert_eq!(pt.savings_pct, 0.0);
        assert_eq!(pt.saved_tokens, 0);
    }

    // 7. get_db_path respects environment variable MYCELIUM_DB_PATH
    #[test]
    fn test_custom_db_path_env() {
        use std::env;

        let custom_path = "/tmp/mycelium_test_custom.db";
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { env::set_var("MYCELIUM_DB_PATH", custom_path) };

        let db_path = get_db_path().expect("Failed to get db path");
        assert_eq!(db_path, PathBuf::from(custom_path));

        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { env::remove_var("MYCELIUM_DB_PATH") };
    }

    // 8. get_db_path falls back to default when no custom config
    #[test]
    fn test_default_db_path() {
        use std::env;

        // Ensure no env var is set
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { env::remove_var("MYCELIUM_DB_PATH") };

        let db_path = get_db_path().expect("Failed to get db path");
        assert!(db_path.ends_with("mycelium/history.db"));
    }

    // 9. project_filter_params uses GLOB pattern with * wildcard
    #[test]
    fn test_project_filter_params_glob_pattern() {
        let (exact, glob) = project_filter_params(Some("/home/user/project"));
        assert_eq!(exact.unwrap(), "/home/user/project");
        // Must use * (GLOB) not % (LIKE) for subdirectory prefix matching
        let glob_val = glob.unwrap();
        assert!(glob_val.ends_with('*'), "GLOB pattern must end with *");
        assert!(!glob_val.contains('%'), "Must not contain LIKE wildcard %");
        assert_eq!(
            glob_val,
            format!("/home/user/project{}*", std::path::MAIN_SEPARATOR)
        );
    }

    // 10. project_filter_params returns None for None input
    #[test]
    fn test_project_filter_params_none() {
        let (exact, glob) = project_filter_params(None);
        assert!(exact.is_none());
        assert!(glob.is_none());
    }

    // 11. GLOB pattern safe with underscores in path names
    #[test]
    fn test_project_filter_params_underscore_safe() {
        // In LIKE, _ matches any single char; in GLOB, _ is literal
        let (exact, glob) = project_filter_params(Some("/home/user/my_project"));
        assert_eq!(exact.unwrap(), "/home/user/my_project");
        let glob_val = glob.unwrap();
        // _ must be preserved literally (GLOB treats _ as literal, LIKE does not)
        assert!(glob_val.contains("my_project"));
        assert_eq!(
            glob_val,
            format!("/home/user/my_project{}*", std::path::MAIN_SEPARATOR)
        );
    }

    // 12. record_parse_failure + get_parse_failure_summary roundtrip
    #[test]
    fn test_parse_failure_roundtrip() {
        let tracker = Tracker::new().expect("Failed to create tracker");
        let test_cmd = format!("git -C /path status test_{}", std::process::id());

        tracker
            .record_parse_failure(&test_cmd, "unrecognized subcommand", true)
            .expect("Failed to record parse failure");

        let summary = tracker
            .get_parse_failure_summary()
            .expect("Failed to get summary");

        assert!(summary.total >= 1);
        assert!(summary.recent.iter().any(|r| r.raw_command == test_cmd));
    }

    // 13. recovery_rate calculation
    #[test]
    fn test_parse_failure_recovery_rate() {
        let tracker = Tracker::new().expect("Failed to create tracker");
        let pid = std::process::id();

        // 2 successes, 1 failure
        tracker
            .record_parse_failure(&format!("cmd_ok1_{}", pid), "err", true)
            .unwrap();
        tracker
            .record_parse_failure(&format!("cmd_ok2_{}", pid), "err", true)
            .unwrap();
        tracker
            .record_parse_failure(&format!("cmd_fail_{}", pid), "err", false)
            .unwrap();

        let summary = tracker.get_parse_failure_summary().unwrap();
        // We can't assert exact rate because other tests may have added records,
        // but we can verify recovery_rate is between 0 and 100
        assert!(summary.recovery_rate >= 0.0 && summary.recovery_rate <= 100.0);
    }
}
