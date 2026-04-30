//! Query methods for the Tracker.
//!
//! Contains all read-only query methods that aggregate and retrieve
//! command history from the tracking database.

use anyhow::Result;
use rusqlite::params;

use super::{
    CommandRecord, CommandStats, DayStats, DetailedCommandRecord, GainSummary, MonthStats,
    PassthroughCommandStat, PassthroughSummary, ProjectStats, Tracker, WeekStats,
    project_filter_params,
};

/// A row from the parse health query.
#[derive(Debug, Clone)]
pub struct ParseHealthRow {
    pub command: String,
    pub tier: u8,
    pub count: usize,
}

impl Tracker {
    /// Get overall summary statistics across all recorded commands.
    ///
    /// Returns aggregated metrics including:
    /// - Total commands, tokens (input/output/saved)
    /// - Average savings percentage and execution time
    /// - Top 10 commands by tokens saved
    /// - Last 30 days of activity
    ///
    /// # Examples
    ///
    /// Get summary statistics filtered by project path.
    ///
    /// When `project_path` is `Some`, matches the exact working directory
    /// or any subdirectory (prefix match with path separator).
    pub fn get_summary_filtered(&self, project_path: Option<&str>) -> Result<GainSummary> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut total_commands = 0usize;
        let mut total_input = 0usize;
        let mut total_output = 0usize;
        let mut total_saved = 0usize;
        let mut total_time_ms = 0u64;

        let mut stmt = self.conn.prepare(
            "SELECT input_tokens, output_tokens, saved_tokens, exec_time_ms
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, i64>(3)? as u64,
            ))
        })?;

        for row in rows {
            let (input, output, saved, time_ms) = row?;
            total_commands += 1;
            total_input += input;
            total_output += output;
            total_saved += saved;
            total_time_ms += time_ms;
        }

        let avg_savings_pct = if total_input > 0 {
            (total_saved as f64 / total_input as f64) * 100.0
        } else {
            0.0
        };

        let avg_time_ms = if total_commands > 0 {
            total_time_ms / total_commands as u64
        } else {
            0
        };

        let by_command = self.get_by_command_limited(project_path, 10)?;
        let by_day = self.get_by_day(project_path)?;

        Ok(GainSummary {
            total_commands,
            total_input,
            total_output,
            total_saved,
            avg_savings_pct,
            total_time_ms,
            avg_time_ms,
            by_command,
            by_day,
        })
    }

    pub fn get_by_command_limited(
        &self,
        project_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<CommandStats>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT mycelium_cmd, COUNT(*), SUM(input_tokens), SUM(saved_tokens), AVG(savings_pct), AVG(exec_time_ms)
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY mycelium_cmd
             ORDER BY SUM(saved_tokens) DESC, mycelium_cmd ASC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob, limit as i64], |row| {
            Ok(CommandStats {
                command: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
                input_tokens: row.get::<_, i64>(2)? as usize,
                tokens_saved: row.get::<_, i64>(3)? as usize,
                savings_pct: row.get(4)?,
                exec_time_ms: row.get::<_, f64>(5)? as u64,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn get_by_day(&self, project_path: Option<&str>) -> Result<Vec<(String, usize)>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT DATE(timestamp), SUM(saved_tokens)
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY DATE(timestamp)
             ORDER BY DATE(timestamp) DESC
             LIMIT 30",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        let mut result: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        result.reverse();
        Ok(result)
    }

    /// Get aggregate passthrough statistics filtered by project path.
    pub fn get_passthrough_summary_filtered(
        &self,
        project_path: Option<&str>,
    ) -> Result<PassthroughSummary> {
        let (project_exact, project_glob) = project_filter_params(project_path);

        let total_commands: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM commands
             WHERE execution_kind = 'passthrough'
               AND (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;

        let total_exec_time_ms: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(exec_time_ms), 0)
             FROM commands
             WHERE execution_kind = 'passthrough'
               AND (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT original_cmd, COUNT(*), COALESCE(SUM(exec_time_ms), 0)
             FROM commands
             WHERE execution_kind = 'passthrough'
               AND (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY original_cmd
             ORDER BY COUNT(*) DESC, original_cmd ASC
             LIMIT 5",
        )?;

        let top_commands = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(PassthroughCommandStat {
                    command: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                    total_exec_time_ms: row.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PassthroughSummary {
            total_commands: total_commands as usize,
            total_exec_time_ms: total_exec_time_ms as u64,
            top_commands,
        })
    }

    /// Get daily statistics for all recorded days.
    ///
    /// Returns one [`DayStats`] per day with commands executed, tokens saved,
    /// and execution time metrics. Results are ordered chronologically (oldest first).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// let days = tracker.get_all_days()?;
    /// for day in days.iter().take(7) {
    ///     println!("{}: {} commands, {} tokens saved",
    ///         day.date, day.commands, day.saved_tokens);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn get_all_days(&self) -> Result<Vec<DayStats>> {
        self.get_all_days_filtered(None)
    }

    /// Get daily statistics filtered by project path.
    pub fn get_all_days_filtered(&self, project_path: Option<&str>) -> Result<Vec<DayStats>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT
                DATE(timestamp) as date,
                COUNT(*) as commands,
                SUM(input_tokens) as input,
                SUM(output_tokens) as output,
                SUM(saved_tokens) as saved,
                SUM(exec_time_ms) as total_time
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY DATE(timestamp)
             ORDER BY DATE(timestamp) DESC",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob], |row| {
            let input = row.get::<_, i64>(2)? as usize;
            let saved = row.get::<_, i64>(4)? as usize;
            let commands = row.get::<_, i64>(1)? as usize;
            let total_time = row.get::<_, i64>(5)? as u64;
            let savings_pct = if input > 0 {
                (saved as f64 / input as f64) * 100.0
            } else {
                0.0
            };
            let avg_time_ms = if commands > 0 {
                total_time / commands as u64
            } else {
                0
            };

            Ok(DayStats {
                date: row.get(0)?,
                commands,
                input_tokens: input,
                output_tokens: row.get::<_, i64>(3)? as usize,
                saved_tokens: saved,
                savings_pct,
                total_time_ms: total_time,
                avg_time_ms,
            })
        })?;

        let mut result: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        result.reverse();
        Ok(result)
    }

    /// Get weekly statistics grouped by week.
    ///
    /// Returns one [`WeekStats`] per week with aggregated metrics.
    /// Weeks start on Sunday (SQLite default). Results ordered chronologically.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// let weeks = tracker.get_by_week()?;
    /// for week in weeks {
    ///     println!("{}: {} tokens saved",
    ///         week.date, week.saved_tokens);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn get_by_week(&self) -> Result<Vec<WeekStats>> {
        self.get_by_week_filtered(None)
    }

    /// Get weekly statistics filtered by project path.
    pub fn get_by_week_filtered(&self, project_path: Option<&str>) -> Result<Vec<WeekStats>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT
                DATE(timestamp, 'weekday 0', '-6 days') as week_start,
                DATE(timestamp, 'weekday 0') as week_end,
                COUNT(*) as commands,
                SUM(input_tokens) as input,
                SUM(output_tokens) as output,
                SUM(saved_tokens) as saved,
                SUM(exec_time_ms) as total_time
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY week_start
             ORDER BY week_start DESC",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob], |row| {
            let input = row.get::<_, i64>(3)? as usize;
            let saved = row.get::<_, i64>(5)? as usize;
            let commands = row.get::<_, i64>(2)? as usize;
            let total_time = row.get::<_, i64>(6)? as u64;
            let savings_pct = if input > 0 {
                (saved as f64 / input as f64) * 100.0
            } else {
                0.0
            };
            let avg_time_ms = if commands > 0 {
                total_time / commands as u64
            } else {
                0
            };

            Ok(WeekStats {
                date: row.get(0)?,
                week_end: row.get(1)?,
                commands,
                input_tokens: input,
                output_tokens: row.get::<_, i64>(4)? as usize,
                saved_tokens: saved,
                savings_pct,
                total_time_ms: total_time,
                avg_time_ms,
            })
        })?;

        let mut result: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        result.reverse();
        Ok(result)
    }

    /// Get monthly statistics grouped by month.
    ///
    /// Returns one [`MonthStats`] per month (YYYY-MM-01 format) with aggregated metrics.
    /// Results ordered chronologically.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// let months = tracker.get_by_month()?;
    /// for month in months {
    ///     println!("{}: {} tokens saved ({:.1}%)",
    ///         month.date, month.saved_tokens, month.savings_pct);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn get_by_month(&self) -> Result<Vec<MonthStats>> {
        self.get_by_month_filtered(None)
    }

    /// Get monthly statistics filtered by project path.
    pub fn get_by_month_filtered(&self, project_path: Option<&str>) -> Result<Vec<MonthStats>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT
                strftime('%Y-%m-01', timestamp) as month_start,
                COUNT(*) as commands,
                SUM(input_tokens) as input,
                SUM(output_tokens) as output,
                SUM(saved_tokens) as saved,
                SUM(exec_time_ms) as total_time
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY strftime('%Y-%m', timestamp)
             ORDER BY strftime('%Y-%m', timestamp) DESC",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob], |row| {
            let input = row.get::<_, i64>(2)? as usize;
            let saved = row.get::<_, i64>(4)? as usize;
            let commands = row.get::<_, i64>(1)? as usize;
            let total_time = row.get::<_, i64>(5)? as u64;
            let savings_pct = if input > 0 {
                (saved as f64 / input as f64) * 100.0
            } else {
                0.0
            };
            let avg_time_ms = if commands > 0 {
                total_time / commands as u64
            } else {
                0
            };

            Ok(MonthStats {
                date: row.get(0)?,
                commands,
                input_tokens: input,
                output_tokens: row.get::<_, i64>(3)? as usize,
                saved_tokens: saved,
                savings_pct,
                total_time_ms: total_time,
                avg_time_ms,
            })
        })?;

        let mut result: Vec<_> = rows.collect::<Result<Vec<_>, _>>()?;
        result.reverse();
        Ok(result)
    }

    /// Get recent command history.
    ///
    /// Returns up to `limit` most recent command records, ordered by timestamp (newest first).
    ///
    /// # Arguments
    ///
    /// - `limit`: Maximum number of records to return
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::Tracker;
    ///
    /// let tracker = Tracker::new()?;
    /// let recent = tracker.get_recent(10)?;
    /// for cmd in recent {
    ///     println!("{}: {} saved {:.1}%",
    ///         cmd.timestamp, cmd.mycelium_cmd, cmd.savings_pct);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    #[allow(
        dead_code,
        reason = "Round-trip query coverage lives in tests while the API remains available to callers"
    )]
    pub fn get_recent(&self, limit: usize) -> Result<Vec<CommandRecord>> {
        self.get_recent_filtered(limit, None)
    }

    /// Get recent command history filtered by project path.
    pub fn get_recent_filtered(
        &self,
        limit: usize,
        project_path: Option<&str>,
    ) -> Result<Vec<CommandRecord>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, mycelium_cmd, saved_tokens, savings_pct
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY timestamp DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob, limit as i64], |row| {
            Ok(CommandRecord {
                timestamp: row.get(0)?,
                mycelium_cmd: row.get(1)?,
                saved_tokens: row.get::<_, i64>(2)? as usize,
                savings_pct: row.get(3)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Get detailed recent command history filtered by project path.
    pub fn get_recent_detailed_filtered(
        &self,
        limit: usize,
        project_path: Option<&str>,
    ) -> Result<Vec<DetailedCommandRecord>> {
        let (project_exact, project_glob) = project_filter_params(project_path);
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, mycelium_cmd, project_path, session_id, input_tokens, output_tokens, saved_tokens, savings_pct
             FROM commands
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY timestamp DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![project_exact, project_glob, limit as i64], |row| {
            Ok(DetailedCommandRecord {
                timestamp: row.get(0)?,
                command: row.get(1)?,
                project_path: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                session_id: row.get(3)?,
                input_tokens: row.get::<_, i64>(4)? as usize,
                output_tokens: row.get::<_, i64>(5)? as usize,
                saved_tokens: row.get::<_, i64>(6)? as usize,
                savings_pct: row.get(7)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Get per-project aggregated statistics.
    ///
    /// Returns one [`ProjectStats`] per distinct `project_path`, ordered by
    /// total tokens saved (descending). Rows with empty or NULL project paths
    /// are excluded.
    pub fn get_by_project(&self) -> Result<Vec<ProjectStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT project_path, COUNT(*), SUM(saved_tokens), AVG(savings_pct), MAX(timestamp)
             FROM commands
             WHERE project_path IS NOT NULL AND project_path != ''
             GROUP BY project_path
             ORDER BY SUM(saved_tokens) DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProjectStats {
                project_path: row.get(0)?,
                commands: row.get(1)?,
                saved_tokens: row.get(2)?,
                avg_savings_pct: row.get(3)?,
                last_used: row.get(4)?,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

impl Tracker {
    /// Get parse tier distribution for the parse-health command.
    ///
    /// Returns rows grouped by command and tier, excluding legacy commands (parse_tier=0).
    /// Used by `mycelium parse-health`.
    pub fn get_parse_health(&self, days: u32) -> Result<Vec<ParseHealthRow>> {
        let modifier = format!("-{} days", days);
        let mut stmt = self.conn.prepare(
            "SELECT mycelium_cmd, parse_tier, COUNT(*) as count
             FROM commands
             WHERE timestamp > datetime('now', ?)
               AND parse_tier > 0
             GROUP BY mycelium_cmd, parse_tier
             ORDER BY count DESC",
        )?;

        let rows = stmt.query_map(params![&modifier], |row| {
            Ok(ParseHealthRow {
                command: row.get(0)?,
                tier: row.get::<_, i64>(1)? as u8,
                count: row.get::<_, i64>(2)? as usize,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}
