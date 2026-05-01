use super::*;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tempfile::tempdir;

fn tracking_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_test_db<T>(test_name: &str, f: impl FnOnce(&str) -> T) -> T {
    let _guard = tracking_test_lock()
        .lock()
        .expect("tracking test lock poisoned");
    let dir = tempdir().expect("failed to create tempdir");
    let path = dir.path().join(format!("{test_name}.db"));
    let path_str = path.to_string_lossy().to_string();
    f(&path_str)
}

fn with_test_db_env<T>(test_name: &str, f: impl FnOnce() -> T) -> T {
    with_test_db(test_name, |path| {
        // SAFETY: tracking tests serialize access with a global mutex, so process-wide
        // environment mutation is isolated to this test.
        unsafe { std::env::set_var("MYCELIUM_DB_PATH", path) };
        let result = f();
        // SAFETY: see rationale above for set_var.
        unsafe { std::env::remove_var("MYCELIUM_DB_PATH") };
        result
    })
}

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
    with_test_db("test_tracker_record_and_recent", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

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
    });
}

#[test]
fn test_tracker_applies_sqlite_pragmas() {
    with_test_db("test_tracker_applies_sqlite_pragmas", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

        let journal_mode: String = tracker
            .conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .expect("Failed to read journal_mode");
        let busy_timeout: i64 = tracker
            .conn
            .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
            .expect("Failed to read busy_timeout");
        let foreign_keys: i64 = tracker
            .conn
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("Failed to read foreign_keys");

        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(busy_timeout, 5000);
        assert_eq!(foreign_keys, 1);
    });
}

#[test]
fn test_tracker_recent_detailed_includes_token_counts_and_project_path() {
    with_test_db(
        "test_tracker_recent_detailed_includes_token_counts_and_project_path",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");
            let project_path = PathBuf::from(".")
                .canonicalize()
                .expect("Failed to canonicalize cwd")
                .to_string_lossy()
                .to_string();

            tracker
                .record("cargo test", "mycelium cargo test", 1000, 200, 50)
                .expect("Failed to record");

            let recent = tracker
                .get_recent_detailed_filtered(5, Some(&project_path))
                .expect("Failed to get detailed recent commands");
            let record = recent.first().expect("Expected a detailed record");

            assert_eq!(record.command, "mycelium cargo test");
            assert_eq!(record.project_path, project_path);
            assert!(record.session_id.is_none());
            assert_eq!(record.input_tokens, 1000);
            assert_eq!(record.output_tokens, 200);
            assert_eq!(record.saved_tokens, 800);
            assert_eq!(record.savings_pct, 80.0);
        },
    );
}

#[test]
fn test_get_by_command_limited_honors_limit() {
    with_test_db("test_get_by_command_limited_honors_limit", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

        tracker
            .record("git status", "mycelium git status", 100, 10, 10)
            .expect("Failed to record git status");
        tracker
            .record("cargo test", "mycelium cargo test", 200, 20, 10)
            .expect("Failed to record cargo test");

        let commands = tracker
            .get_by_command_limited(None, 1)
            .expect("Failed to get by-command stats");

        assert_eq!(commands.len(), 1);
    });
}

#[test]
fn test_telemetry_summary_surface_orders_command_breakdown_deterministically() {
    with_test_db(
        "test_telemetry_summary_surface_orders_command_breakdown_deterministically",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            tracker
                .record("git status", "mycelium git status", 100, 20, 5)
                .expect("record git status");
            tracker
                .record("git diff", "mycelium git diff", 100, 20, 4)
                .expect("record git diff");

            let summary = tracker
                .get_telemetry_summary_filtered(None)
                .expect("telemetry summary");

            assert_eq!(
                summary
                    .command_breakdown
                    .iter()
                    .map(|command| command.command.as_str())
                    .collect::<Vec<_>>(),
                vec!["mycelium git diff", "mycelium git status"]
            );
        },
    );
}

#[test]
fn test_telemetry_summary_surface_orders_parse_failures_deterministically() {
    with_test_db(
        "test_telemetry_summary_surface_orders_parse_failures_deterministically",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            tracker
                .record_parse_failure("zeta command", "parse failed", false)
                .expect("record zeta parse failure");
            tracker
                .record_parse_failure("alpha command", "parse failed", false)
                .expect("record alpha parse failure");

            let summary = tracker
                .get_telemetry_summary_filtered(None)
                .expect("telemetry summary");

            assert_eq!(
                summary
                    .parse_failure_summary
                    .top_commands
                    .iter()
                    .map(|command| command.command.as_str())
                    .collect::<Vec<_>>(),
                vec!["alpha command", "zeta command"]
            );
        },
    );
}

// 4. track_passthrough doesn't dilute stats
#[test]
fn test_track_passthrough_no_dilution() {
    with_test_db("test_track_passthrough_no_dilution", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

        // Use unique test identifiers
        let pid = std::process::id();
        let cmd1 = format!("mycelium cmd1_test_{}", pid);
        let cmd2 = format!("mycelium cmd2_passthrough_test_{}", pid);

        // Record one real command with 80% savings
        tracker
            .record("cmd1", &cmd1, 1000, 200, 10)
            .expect("Failed to record cmd1");

        // Record passthrough through the dedicated tracking path
        tracker
            .record_passthrough("cmd2", &cmd2, 5)
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
    });
}

// 5. TimedExecution::track records with exec_time > 0
#[test]
fn test_timed_execution_records_time() {
    with_test_db_env("test_timed_execution_records_time", || {
        let timer = TimedExecution::start();
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.track("test cmd", "mycelium test", "raw input data", "filtered");

        // Verify via DB that record exists
        let tracker = Tracker::new().expect("Failed to create tracker");
        let recent = tracker.get_recent(5).expect("Failed to get recent");
        assert!(recent.iter().any(|r| r.mycelium_cmd == "mycelium test"));
    });
}

// 6. TimedExecution::track_passthrough records with 0 tokens
#[test]
fn test_timed_execution_passthrough() {
    with_test_db_env("test_timed_execution_passthrough", || {
        let timer = TimedExecution::start();
        timer.track_passthrough("git tag", "mycelium git tag (passthrough)");

        let tracker = Tracker::new().expect("Failed to create tracker");
        let recent = tracker.get_recent(5).expect("Failed to get recent");

        let pt = recent
            .iter()
            .find(|r| r.mycelium_cmd.contains("passthrough"))
            .expect("Passthrough record not found");

        assert_eq!(pt.savings_pct, 0.0);
        assert_eq!(pt.saved_tokens, 0);
    });
}

#[test]
fn test_get_passthrough_summary_filtered_groups_passthrough_commands() {
    with_test_db(
        "test_get_passthrough_summary_filtered_groups_passthrough_commands",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            tracker
                .record_passthrough("git tag --list", "mycelium git tag (passthrough)", 5)
                .expect("Failed to record passthrough 1");
            tracker
                .record_passthrough("git tag --list", "mycelium git tag (passthrough)", 7)
                .expect("Failed to record passthrough 2");
            tracker
                .record("git status", "mycelium git status", 100, 20, 3)
                .expect("Failed to record filtered command");

            let summary = tracker
                .get_passthrough_summary_filtered(None)
                .expect("Failed to load passthrough summary");

            assert_eq!(summary.total_commands, 2);
            assert_eq!(summary.total_exec_time_ms, 12);
            assert_eq!(summary.top_commands.len(), 1);
            assert_eq!(summary.top_commands[0].command, "git tag --list");
            assert_eq!(summary.top_commands[0].count, 2);
        },
    );
}

#[test]
fn test_get_passthrough_summary_filtered_respects_project_scope() {
    with_test_db(
        "test_get_passthrough_summary_filtered_respects_project_scope",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            tracker
                .conn
                .execute(
                    "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, \
                 input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        "2026-03-28T12:00:00Z",
                        "git tag --list",
                        "mycelium git tag (passthrough)",
                        "/tmp/project-a",
                        0_i64,
                        0_i64,
                        0_i64,
                        0.0_f64,
                        5_i64,
                        "passthrough",
                    ],
                )
                .expect("Failed to insert project a row");

            tracker
                .conn
                .execute(
                    "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, \
                 input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms, execution_kind) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        "2026-03-28T12:05:00Z",
                        "gh run view",
                        "mycelium gh run view (passthrough)",
                        "/tmp/project-b",
                        0_i64,
                        0_i64,
                        0_i64,
                        0.0_f64,
                        9_i64,
                        "passthrough",
                    ],
                )
                .expect("Failed to insert project b row");

            let summary = tracker
                .get_passthrough_summary_filtered(Some("/tmp/project-a"))
                .expect("Failed to load scoped passthrough summary");

            assert_eq!(summary.total_commands, 1);
            assert_eq!(summary.top_commands[0].command, "git tag --list");
        },
    );
}

// 7. get_db_path respects an explicit override path
#[test]
fn test_custom_db_path_env() {
    let custom_path = "/tmp/mycelium_test_custom.db";
    let db_path = get_db_path(Some(custom_path)).expect("Failed to get db path");
    assert_eq!(db_path, PathBuf::from(custom_path));
}

// 8. get_db_path falls back to default when no override is provided
#[test]
fn test_default_db_path() {
    let db_path = get_db_path(None).expect("Failed to get db path");
    assert!(db_path.ends_with("mycelium/history.db"));
}

#[test]
fn test_resolve_db_path_info_override() {
    let info = resolve_db_path_info(Some("/tmp/mycelium_override.db"))
        .expect("Failed to resolve db path info");
    assert_eq!(info.source, DbPathSource::Override);
    assert_eq!(info.path, PathBuf::from("/tmp/mycelium_override.db"));
    assert!(info.config_path.ends_with("mycelium/config.toml"));
}

#[test]
fn test_resolve_db_path_info_env() {
    with_test_db("test_resolve_db_path_info_env", |db_path| {
        // SAFETY: serialized by global test lock in with_test_db.
        unsafe { std::env::set_var("MYCELIUM_DB_PATH", db_path) };
        let info = resolve_db_path_info(None).expect("Failed to resolve db path info");
        // SAFETY: paired with set_var above.
        unsafe { std::env::remove_var("MYCELIUM_DB_PATH") };
        assert_eq!(info.source, DbPathSource::Environment);
        assert_eq!(info.path, PathBuf::from(db_path));
    });
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
    with_test_db("test_parse_failure_roundtrip", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");
        let test_cmd = format!("git -C /path status test_{}", std::process::id());

        tracker
            .record_parse_failure(&test_cmd, "unrecognized subcommand", true)
            .expect("Failed to record parse failure");

        let summary = tracker
            .get_parse_failure_summary()
            .expect("Failed to get summary");

        assert!(summary.total >= 1);
        assert!(summary.recent.iter().any(|r| r.raw_command == test_cmd));
    });
}

// 13. recovery_rate calculation
#[test]
fn test_parse_failure_recovery_rate() {
    with_test_db("test_parse_failure_recovery_rate", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");
        let pid = std::process::id();

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
        assert!(summary.recovery_rate >= 0.0 && summary.recovery_rate <= 100.0);
    });
}

#[test]
fn test_parse_failure_summary_filtered_respects_project_scope() {
    with_test_db(
        "test_parse_failure_summary_filtered_respects_project_scope",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            tracker
                .conn
                .execute(
                    "INSERT INTO parse_failures (timestamp, raw_command, error_message, fallback_succeeded, project_path)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        "2026-03-28T12:00:00Z",
                        "gh issue list",
                        "json parse failed",
                        1_i32,
                        "/tmp/project-a",
                    ],
                )
                .expect("Failed to insert project a parse failure");

            tracker
                .conn
                .execute(
                    "INSERT INTO parse_failures (timestamp, raw_command, error_message, fallback_succeeded, project_path)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        "2026-03-28T12:05:00Z",
                        "gh repo view",
                        "json parse failed",
                        0_i32,
                        "/tmp/project-b",
                    ],
                )
                .expect("Failed to insert project b parse failure");

            let summary = tracker
                .get_parse_failure_summary_filtered(Some("/tmp/project-a"))
                .expect("Failed to load scoped parse failures");

            assert_eq!(summary.total, 1);
            assert_eq!(summary.top_commands, vec![("gh issue list".to_string(), 1)]);
            assert_eq!(summary.recent.len(), 1);
            assert_eq!(summary.recent[0].raw_command, "gh issue list");
        },
    );
}

#[test]
fn test_current_project_path_string_honors_env_override() {
    let _guard = tracking_test_lock()
        .lock()
        .expect("tracking test lock poisoned");
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("project");
    std::fs::create_dir_all(&path).expect("create project dir");
    let expected = path.canonicalize().expect("canonical path");

    // SAFETY: tracking tests serialize access with a global mutex, so process-wide
    // environment mutation is isolated to this test.
    unsafe { std::env::set_var("MYCELIUM_PROJECT_PATH", &path) };
    let actual = super::utils::current_project_path_string();
    // SAFETY: paired with set_var above.
    unsafe { std::env::remove_var("MYCELIUM_PROJECT_PATH") };

    assert_eq!(PathBuf::from(actual), expected);
}

#[test]
fn test_current_runtime_session_id_honors_claude_env() {
    let _guard = tracking_test_lock()
        .lock()
        .expect("tracking test lock poisoned");

    // SAFETY: tracking tests serialize access with a global mutex, so process-wide
    // environment mutation is isolated to this test.
    unsafe { std::env::set_var("CLAUDE_SESSION_ID", "claude-session-42") };
    let actual = super::utils::current_runtime_session_id();
    // SAFETY: paired with set_var above.
    unsafe { std::env::remove_var("CLAUDE_SESSION_ID") };

    assert_eq!(actual.as_deref(), Some("claude-session-42"));
}

#[test]
fn test_tracker_recent_detailed_includes_runtime_session_id_when_available() {
    with_test_db(
        "test_tracker_recent_detailed_includes_runtime_session_id_when_available",
        |db_path| {
            let tracker =
                Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

            // SAFETY: tracking tests serialize access with a global mutex, so process-wide
            // environment mutation is isolated to this test.
            unsafe { std::env::set_var("CLAUDE_SESSION_ID", "claude-session-99") };
            tracker
                .record("cargo test", "mycelium cargo test", 1000, 200, 50)
                .expect("Failed to record");
            // SAFETY: paired with set_var above.
            unsafe { std::env::remove_var("CLAUDE_SESSION_ID") };

            let recent = tracker
                .get_recent_detailed_filtered(5, None)
                .expect("Failed to get detailed recent commands");
            let record = recent.first().expect("Expected a detailed record");

            assert_eq!(record.session_id.as_deref(), Some("claude-session-99"));
        },
    );
}

// 14. get_by_project groups by project_path and returns sorted results
#[test]
fn test_get_by_project() {
    with_test_db("test_get_by_project", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");
        let pid = std::process::id();

        let ts = chrono::Utc::now().to_rfc3339();
        for (project, saved) in &[
            (format!("/tmp/proj_a_{}", pid), 500),
            (format!("/tmp/proj_a_{}", pid), 300),
            (format!("/tmp/proj_b_{}", pid), 1000),
        ] {
            tracker
                .conn
                .execute(
                    "INSERT INTO commands (timestamp, original_cmd, mycelium_cmd, project_path, \
                     input_tokens, output_tokens, saved_tokens, savings_pct, exec_time_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        ts,
                        "test cmd",
                        "mycelium test",
                        project,
                        *saved * 2,
                        *saved,
                        *saved,
                        50.0,
                        10
                    ],
                )
                .expect("insert failed");
        }

        let results = tracker.get_by_project().expect("get_by_project failed");

        let proj_a = results
            .iter()
            .find(|r| r.project_path == format!("/tmp/proj_a_{}", pid))
            .expect("proj_a not found in results");
        let proj_b = results
            .iter()
            .find(|r| r.project_path == format!("/tmp/proj_b_{}", pid))
            .expect("proj_b not found in results");

        assert_eq!(proj_a.commands, 2);
        assert_eq!(proj_b.commands, 1);
        assert_eq!(proj_a.saved_tokens, 800);
        assert_eq!(proj_b.saved_tokens, 1000);

        let idx_a = results
            .iter()
            .position(|r| r.project_path == format!("/tmp/proj_a_{}", pid))
            .unwrap();
        let idx_b = results
            .iter()
            .position(|r| r.project_path == format!("/tmp/proj_b_{}", pid))
            .unwrap();
        assert!(
            idx_b < idx_a,
            "proj_b (1000 saved) should appear before proj_a (800 saved)"
        );
    });
}

// 15. record_summary round-trip test
#[test]
fn test_record_summary_roundtrip() {
    with_test_db("test_record_summary_roundtrip", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");

        tracker
            .record_summary("cargo test", "5 tests passed", 1000, 200, 50, Some(0))
            .expect("Failed to record summary");

        let (captured_at, command, summary, project_path, input_tokens, output_tokens, tokens_saved, savings_pct) = tracker
            .conn
            .query_row(
                "SELECT captured_at, command, summary, project_path, input_tokens, output_tokens, tokens_saved, savings_pct FROM summaries WHERE command = 'cargo test'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, f64>(7)?,
                    ))
                },
            )
            .expect("Failed to query summary");

        assert_eq!(command, "cargo test");
        assert_eq!(summary, "5 tests passed");
        assert_eq!(input_tokens, 1000);
        assert_eq!(output_tokens, 200);
        assert_eq!(tokens_saved, 800);
        assert_eq!(savings_pct, 80.0);
        assert!(!captured_at.is_empty());
        assert!(!project_path.is_empty());
    });
}

// 16. WeekStats serialization test -- verify `date` field is present and no extra fields
#[test]
fn test_week_stats_serialization() {
    let week_stats = WeekStats {
        date: "2026-04-20".to_string(),
        week_end: "2026-04-26".to_string(),
        commands: 42,
        input_tokens: 15420,
        output_tokens: 3842,
        saved_tokens: 11578,
        savings_pct: 75.08,
        total_time_ms: 8450,
        avg_time_ms: 201,
    };

    let json_str = serde_json::to_string(&week_stats).expect("Failed to serialize WeekStats");
    let json: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

    // Verify `date` field is present
    assert!(json.get("date").is_some(), "Missing 'date' field in serialized WeekStats");
    assert_eq!(json.get("date").unwrap().as_str(), Some("2026-04-20"));

    // Verify `week_end` is not serialized (skip_serializing)
    assert!(json.get("week_end").is_none(), "week_end should not be serialized");

    // Verify all required schema fields are present
    assert!(json.get("commands").is_some());
    assert!(json.get("saved_tokens").is_some());
    assert!(json.get("input_tokens").is_some());
    assert!(json.get("output_tokens").is_some());
    assert!(json.get("avg_time_ms").is_some());
    assert!(json.get("total_time_ms").is_some());
    assert!(json.get("savings_pct").is_some());

    // Verify no extra fields beyond schema
    let allowed_fields = [
        "date", "commands", "saved_tokens", "input_tokens", "output_tokens",
        "avg_time_ms", "total_time_ms", "savings_pct"
    ];
    for (key, _) in json.as_object().unwrap().iter() {
        assert!(allowed_fields.contains(&key.as_str()), "Unexpected field '{}' in serialized WeekStats", key);
    }
}

// 17. MonthStats serialization test -- verify `date` field is present and no extra fields
#[test]
fn test_month_stats_serialization() {
    let month_stats = MonthStats {
        date: "2026-04-01".to_string(),
        commands: 128,
        input_tokens: 61200,
        output_tokens: 15342,
        saved_tokens: 45858,
        savings_pct: 75.0,
        total_time_ms: 33800,
        avg_time_ms: 264,
    };

    let json_str = serde_json::to_string(&month_stats).expect("Failed to serialize MonthStats");
    let json: serde_json::Value = serde_json::from_str(&json_str).expect("Failed to parse JSON");

    // Verify `date` field is present
    assert!(json.get("date").is_some(), "Missing 'date' field in serialized MonthStats");
    assert_eq!(json.get("date").unwrap().as_str(), Some("2026-04-01"));

    // Verify all required schema fields are present
    assert!(json.get("commands").is_some());
    assert!(json.get("saved_tokens").is_some());
    assert!(json.get("input_tokens").is_some());
    assert!(json.get("output_tokens").is_some());
    assert!(json.get("avg_time_ms").is_some());
    assert!(json.get("total_time_ms").is_some());
    assert!(json.get("savings_pct").is_some());

    // Verify no extra fields beyond schema
    let allowed_fields = [
        "date", "commands", "saved_tokens", "input_tokens", "output_tokens",
        "avg_time_ms", "total_time_ms", "savings_pct"
    ];
    for (key, _) in json.as_object().unwrap().iter() {
        assert!(allowed_fields.contains(&key.as_str()), "Unexpected field '{}' in serialized MonthStats", key);
    }
}
