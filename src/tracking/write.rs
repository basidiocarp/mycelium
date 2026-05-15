//! Write methods for tracking persistence.

use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::params;

use super::types::{ParseFailureRecord, ParseFailureSummary, HISTORY_DAYS};
use super::utils::{
    current_project_path_string, current_project_root, current_runtime_session_id,
    derive_project_name, project_filter_params,
};
use super::Tracker;

impl Tracker {
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
        #[allow(clippy::cast_precision_loss)]
        let pct = if input_tokens > 0 {
            (saved as f64 / input_tokens as f64) * 100.0
        } else {
            0.0
        };

        let project_path = current_project_path_string();
        let project_name = derive_project_name();
        let session_id = current_runtime_session_id();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, project_name, session_id, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                Utc::now().to_rfc3339(),
                original_cmd,
                mycelium_cmd,
                project_path,
                project_name,
                session_id,
                i64::try_from(input_tokens).unwrap_or(i64::MAX),
                i64::try_from(output_tokens).unwrap_or(i64::MAX),
                i64::try_from(saved).unwrap_or(i64::MAX),
                pct,
                i64::try_from(exec_time_ms).unwrap_or(i64::MAX),
                "filtered",
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    /// Record a command execution with parse tier and format mode tracking.
    ///
    /// Use this for commands that use the parser framework (`OutputParser` trait).
    /// Legacy commands should continue using `record()`.
    ///
    /// # Arguments
    ///
    /// - `parse_tier`: Parser result tier (1=Full, 2=Degraded, 3=Passthrough, 0=legacy)
    /// - `format_mode`: Format mode used ("compact", "verbose", "ultra", or "")
    #[allow(clippy::too_many_arguments)]
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
        #[allow(clippy::cast_precision_loss)]
        let pct = if input_tokens > 0 {
            (saved as f64 / input_tokens as f64) * 100.0
        } else {
            0.0
        };

        let project_path = current_project_path_string();
        let project_name = derive_project_name();
        let session_id = current_runtime_session_id();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, project_name, session_id, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, parse_tier, format_mode, execution_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                Utc::now().to_rfc3339(),
                original_cmd,
                mycelium_cmd,
                project_path,
                project_name,
                session_id,
                i64::try_from(input_tokens).unwrap_or(i64::MAX),
                i64::try_from(output_tokens).unwrap_or(i64::MAX),
                i64::try_from(saved).unwrap_or(i64::MAX),
                pct,
                i64::try_from(exec_time_ms).unwrap_or(i64::MAX),
                i64::from(parse_tier),
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
        let project_name = derive_project_name();
        let session_id = current_runtime_session_id();

        self.conn.execute(
            "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, project_name, session_id, input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, 0.0, ?7, 'passthrough')",
            params![
                Utc::now().to_rfc3339(),
                original_cmd,
                mycelium_cmd,
                project_path,
                project_name,
                session_id,
                i64::try_from(exec_time_ms).unwrap_or(i64::MAX),
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    /// Record a command output summary for analytics.
    pub fn record_summary(
        &self,
        command: &str,
        summary: &str,
        input_tokens: usize,
        output_tokens: usize,
        exec_time_ms: u64,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let saved = input_tokens.saturating_sub(output_tokens);
        #[allow(clippy::cast_precision_loss)]
        let pct = if input_tokens > 0 {
            (saved as f64 / input_tokens as f64) * 100.0
        } else {
            0.0
        };

        let project_path = current_project_path_string();
        let session_id = current_runtime_session_id();
        let project_root = current_project_root();

        self.conn.execute(
            "INSERT INTO summaries (captured_at, command, summary, project_path, session_id, project_root, input_tokens, output_tokens, tokens_saved, savings_pct, exec_time_ms, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                Utc::now().to_rfc3339(),
                command,
                summary,
                project_path,
                session_id,
                project_root,
                i64::try_from(input_tokens).unwrap_or(i64::MAX),
                i64::try_from(output_tokens).unwrap_or(i64::MAX),
                i64::try_from(saved).unwrap_or(i64::MAX),
                pct,
                i64::try_from(exec_time_ms).unwrap_or(i64::MAX),
                exit_code,
            ],
        )?;

        self.cleanup_old()?;
        Ok(())
    }

    fn cleanup_old(&self) -> Result<()> {
        let cutoff = Utc::now() - Duration::days(HISTORY_DAYS);
        self.conn.execute(
            "DELETE FROM commands WHERE timestamp < ?1",
            params![cutoff.to_rfc3339()],
        )?;
        self.conn.execute(
            "DELETE FROM parse_failures WHERE timestamp < ?1",
            params![cutoff.to_rfc3339()],
        )?;
        self.conn.execute(
            "DELETE FROM summaries WHERE captured_at < ?1",
            params![cutoff.to_rfc3339()],
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
        let project_path = current_project_path_string();

        self.conn.execute(
            "INSERT INTO parse_failures (timestamp, raw_command, error_message, fallback_succeeded, project_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Utc::now().to_rfc3339(),
                raw_command,
                error_message,
                i32::from(fallback_succeeded),
                project_path,
            ],
        )?;
        self.cleanup_old()?;
        Ok(())
    }

    /// Get parse failure summary for `mycelium gain --failures`.
    #[allow(
        dead_code,
        reason = "Round-trip summary coverage lives in tests while the API remains available to callers"
    )]
    pub fn get_parse_failure_summary(&self) -> Result<ParseFailureSummary> {
        self.get_parse_failure_summary_filtered(None)
    }

    /// Get parse failure summary filtered by project path.
    pub fn get_parse_failure_summary_filtered(
        &self,
        project_path: Option<&str>,
    ) -> Result<ParseFailureSummary> {
        let (project_exact, project_glob) = project_filter_params(project_path);

        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM parse_failures
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;

        let succeeded: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM parse_failures
             WHERE fallback_succeeded = 1
               AND (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;

        #[allow(clippy::cast_precision_loss)]
        let recovery_rate = if total > 0 {
            (succeeded as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Top commands by frequency
        let mut stmt = self.conn.prepare(
            "SELECT raw_command, COUNT(*) as cnt
             FROM parse_failures
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY raw_command
             ORDER BY cnt DESC, raw_command ASC
             LIMIT 10",
        )?;
        let top_commands = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok((row.get::<_, String>(0)?, usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0)))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Recent 10
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, raw_command, error_message, fallback_succeeded
             FROM parse_failures
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY timestamp DESC
             LIMIT 10",
        )?;
        let recent = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(ParseFailureRecord {
                    timestamp: row.get(0)?,
                    raw_command: row.get(1)?,
                    error_message: row.get(2)?,
                    fallback_succeeded: row.get::<_, i32>(3)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ParseFailureSummary {
            total: usize::try_from(total).unwrap_or(0),
            recovery_rate,
            top_commands,
            recent,
        })
    }
}
