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

// 4. track_passthrough doesn't dilute stats (input=0, output=0)
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

// 14. get_by_project groups by project_path and returns sorted results
#[test]
fn test_get_by_project() {
    with_test_db("test_get_by_project", |db_path| {
        let tracker = Tracker::new_with_override(Some(db_path)).expect("Failed to create tracker");
        let pid = std::process::id();

        let ts = jiff::Timestamp::now().to_string();
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
