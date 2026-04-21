//! Database schema initialization and migrations.
//!
//! Extracted from `Tracker::new()` to keep `mod.rs` focused on the public API.

use anyhow::{Context, Result};
use rusqlite::Connection;
use tracing;

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
    let _ = conn.execute("ALTER TABLE commands ADD COLUMN session_id TEXT", []);
    // One-time migration: normalize NULLs from pre-default schema
    let has_nulls: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE project_path IS NULL)",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if has_nulls {
        if let Err(e) = conn.execute(
            "UPDATE commands SET project_path = '' WHERE project_path IS NULL",
            [],
        ) {
            tracing::warn!("migration UPDATE on commands failed: {e}");
        }
    }
    // Migration slot: previously attempted to rename mycelium_cmd to itself.
    // This was a no-op and has been skipped. The column mycelium_cmd already
    // exists from schema creation and does not require renaming.
    // Index for fast project-scoped gain queries
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_project_path_timestamp ON commands(project_path, timestamp)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_commands_session_id_timestamp ON commands(session_id, timestamp)",
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
    // Migration: add execution_kind for reliable passthrough diagnostics
    let _ = conn.execute(
        "ALTER TABLE commands ADD COLUMN execution_kind TEXT DEFAULT 'filtered'",
        [],
    );
    let _ = conn.execute(
        "UPDATE commands
         SET execution_kind = 'passthrough'
         WHERE execution_kind IS NULL
            OR (
                execution_kind = 'filtered'
                AND input_tokens = 0
                AND output_tokens = 0
                AND mycelium_cmd LIKE '%(passthrough)%'
            )",
        [],
    );

    conn.execute(
        "CREATE TABLE IF NOT EXISTS parse_failures (
            id INTEGER PRIMARY KEY,
            timestamp TEXT NOT NULL,
            raw_command TEXT NOT NULL,
            error_message TEXT NOT NULL,
            fallback_succeeded INTEGER NOT NULL DEFAULT 0,
            project_path TEXT DEFAULT ''
        )",
        [],
    )
    .context("Failed to create parse_failures table")?;

    let _ = conn.execute(
        "ALTER TABLE parse_failures ADD COLUMN project_path TEXT DEFAULT ''",
        [],
    );
    if let Err(e) = conn.execute(
        "UPDATE parse_failures SET project_path = '' WHERE project_path IS NULL",
        [],
    ) {
        tracing::warn!("migration UPDATE on parse_failures failed: {e}");
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pf_timestamp ON parse_failures(timestamp)",
        [],
    )
    .context("Failed to create idx_pf_timestamp")?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_pf_project_path_timestamp ON parse_failures(project_path, timestamp)",
        [],
    )
    .context("Failed to create idx_pf_project_path_timestamp")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS summaries (
            id INTEGER PRIMARY KEY,
            captured_at TEXT NOT NULL,
            command TEXT NOT NULL,
            summary TEXT NOT NULL,
            input_tokens INTEGER NOT NULL,
            output_tokens INTEGER NOT NULL,
            tokens_saved INTEGER NOT NULL,
            savings_pct REAL NOT NULL,
            exit_code INTEGER,
            exec_time_ms INTEGER,
            project_path TEXT DEFAULT '',
            project_root TEXT DEFAULT '',
            worktree_id TEXT DEFAULT '',
            session_id TEXT
        )",
        [],
    )
    .context("Failed to create summaries table")?;

    // Migrations: add columns that were absent in older summaries table schemas.
    // Uses the silent-error pattern (let _ =) so these no-op on fresh databases
    // where the columns already exist from CREATE TABLE above.
    let _ = conn.execute(
        "ALTER TABLE summaries ADD COLUMN captured_at TEXT DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE summaries ADD COLUMN project_root TEXT DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE summaries ADD COLUMN worktree_id TEXT DEFAULT ''",
        [],
    );

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_summaries_captured_at ON summaries(captured_at)",
        [],
    )
    .context("Failed to create idx_summaries_captured_at")?;

    Ok(())
}
