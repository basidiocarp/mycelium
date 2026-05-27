//! Token savings tracking and analytics system.
//!
//! This module provides comprehensive tracking of Mycelium command executions,
//! recording token savings, execution times, and providing aggregation APIs
//! for daily/weekly/monthly statistics.
//!
//! # Architecture
//!
//! - Storage: `SQLite` database (~/.local/share/mycelium/history.db)
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
mod telemetry;
mod timer;
mod types;
pub(crate) mod utils;
mod write;

use anyhow::Result;
use rusqlite::Connection;

use utils::get_db_path;

#[allow(unused_imports)]
pub use queries::ParseHealthRow;
pub use telemetry::TelemetrySummarySurface;
pub use timer::TimedExecution;
pub use types::*;
pub(crate) use utils::project_filter_params;
#[allow(unused_imports)]
pub use utils::{
    DbPathInfo, DbPathSource, args_display, estimate_tokens, record_parse_failure_silent,
    resolve_db_path_info,
};

/// Main tracking interface for recording and querying command history.
///
/// Manages `SQLite` database connection and provides methods for:
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

impl Tracker {
    /// Create a new tracker instance.
    ///
    /// Opens or creates the `SQLite` database at the platform-specific location.
    /// Automatically creates the `commands` table if it doesn't exist and runs
    /// any necessary schema migrations.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Cannot determine database path
    /// - Cannot create parent directories
    /// - Cannot open/create `SQLite` database
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
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;",
        )?;
        schema::init_schema(&conn)?;

        let tracker = Self { conn };
        // Prune stale rows once per process start rather than per write.
        if let Err(e) = tracker.cleanup_old() {
            tracing::warn!("cleanup_old failed at startup: {e}");
        }
        Ok(tracker)
    }
}

#[cfg(test)]
mod tests;
