//! Database schema initialization and migrations.
//!
//! Extracted from `Tracker::new()` to keep `mod.rs` focused on the public API.

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Initialize the database schema, creating tables and running all migrations.
///
/// Safe to call on an existing database; all operations are idempotent or
/// use `ALTER TABLE … ADD COLUMN` (which no-ops when the column already exists).
pub(super) fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS commands (
            id INTEGER PRIMARY KEY,
            timestamp TEXT NOT NULL,
            original_cmd TEXT NOT NULL,
            mycelium_cmd TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            saved_tokens INTEGER NOT NULL,
            savings_pct REAL NOT NULL
        )",
        [],
    )
    .context("Failed to create commands table")?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON commands(timestamp)",
        [],
    )
    .context("Failed to create idx_timestamp")?;

    // Migration: add exec_time_ms column if it doesn't exist
    let _ = conn.execute(
        "ALTER TABLE commands ADD COLUMN exec_time_ms INTEGER DEFAULT 0",
        [],
    );
    // Migration: add project_path column with DEFAULT '' for new rows
    let _ = conn.execute(
        "ALTER TABLE commands ADD COLUMN project_path TEXT DEFAULT ''",
        [],
    );
    // One-time migration: normalize NULLs from pre-default schema
    let has_nulls: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE project_path IS NULL)",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if has_nulls {
        let _ = conn.execute(
            "UPDATE commands SET project_path = '' WHERE project_path IS NULL",
            [],
        );
    }
    // Migration: rename mycelium_cmd column to mycelium_cmd
    let _ = conn.execute(
        "ALTER TABLE commands RENAME COLUMN mycelium_cmd TO mycelium_cmd",
        [],
    );
    // Index for fast project-scoped gain queries
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_project_path_timestamp ON commands(project_path, timestamp)",
        [],
    );
    // Migration: add parse_tier column for parser framework observability
    let _ = conn.execute(
        "ALTER TABLE commands ADD COLUMN parse_tier INTEGER DEFAULT 0",
        [],
    );
    // Migration: add format_mode column for parser framework observability
    let _ = conn.execute(
        "ALTER TABLE commands ADD COLUMN format_mode TEXT DEFAULT ''",
        [],
    );

    conn.execute(
        "CREATE TABLE IF NOT EXISTS parse_failures (
            id INTEGER PRIMARY KEY,
            timestamp TEXT NOT NULL,
            raw_command TEXT NOT NULL,
            error_message TEXT NOT NULL,
            fallback_succeeded INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .context("Failed to create parse_failures table")?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pf_timestamp ON parse_failures(timestamp)",
        [],
    )
    .context("Failed to create idx_pf_timestamp")?;

    Ok(())
}
