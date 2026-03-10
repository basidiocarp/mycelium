//! End-to-end integration tests for the learn pipeline.
//!
//! Uses a fixture JSONL file that simulates a Claude Code session with
//! error-then-correction patterns, and verifies the full pipeline:
//! provider extraction → find_corrections → deduplicate_corrections.

use mycelium::discover::provider::{ClaudeProvider, SessionProvider};
use mycelium::learn::corrections_store::{apply_correction, load_corrections, write_corrections_json, UserCorrection};
use mycelium::learn::detector::{
    CommandExecution, deduplicate_corrections, extract_base_command, find_corrections,
};
use std::path::Path;
use tempfile::tempdir;

fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Load the fixture and run through the full learn pipeline.
fn pipeline_from_fixture(name: &str) -> Vec<mycelium::learn::detector::CorrectionRule> {
    let path = fixture_path(name);
    let provider = ClaudeProvider;

    let extracted = provider
        .extract_commands(&path)
        .expect("fixture should parse cleanly");

    let commands: Vec<CommandExecution> = extracted
        .into_iter()
        .filter_map(|e| {
            e.output_content.map(|output| CommandExecution {
                command: e.command,
                is_error: e.is_error,
                output,
            })
        })
        .collect();

    let corrections = find_corrections(&commands);
    deduplicate_corrections(corrections)
}

// =========================================================
//  Provider extraction
// =========================================================

#[test]
fn test_fixture_extracts_correct_command_count() {
    let path = fixture_path("learn_session.jsonl");
    let provider = ClaudeProvider;
    let extracted = provider.extract_commands(&path).unwrap();

    // Fixture has 7 tool_use/tool_result pairs
    assert_eq!(extracted.len(), 7, "expected 7 commands from fixture");
}

#[test]
fn test_fixture_captures_error_flags() {
    let path = fixture_path("learn_session.jsonl");
    let provider = ClaudeProvider;
    let extracted = provider.extract_commands(&path).unwrap();

    let errors: Vec<_> = extracted.iter().filter(|e| e.is_error).collect();
    let successes: Vec<_> = extracted.iter().filter(|e| !e.is_error).collect();

    assert_eq!(errors.len(), 3, "expected 3 error results");
    assert_eq!(successes.len(), 4, "expected 4 success results");
}

// =========================================================
//  Correction detection
// =========================================================

#[test]
fn test_detects_git_commit_typo_correction() {
    let rules = pipeline_from_fixture("learn_session.jsonl");

    let git_rule = rules
        .iter()
        .find(|r| r.wrong_pattern.contains("--ammend"))
        .expect("should detect --ammend → --amend correction");

    assert_eq!(git_rule.wrong_pattern, "git commit --ammend");
    assert_eq!(git_rule.right_pattern, "git commit --amend");
    assert!(git_rule.occurrences >= 1);
}

#[test]
fn test_detects_gh_flag_correction() {
    let rules = pipeline_from_fixture("learn_session.jsonl");

    let gh_rule = rules
        .iter()
        .find(|r| r.base_command.starts_with("gh pr"))
        .expect("should detect gh pr edit -t → --title correction");

    assert!(gh_rule.wrong_pattern.contains("-t"));
    assert!(gh_rule.right_pattern.contains("--title"));
}

#[test]
fn test_tdd_cycle_not_detected_as_correction() {
    let rules = pipeline_from_fixture("learn_session.jsonl");

    // cargo test → cargo test (TDD cycle) must NOT appear in corrections
    let cargo_repeat = rules
        .iter()
        .find(|r| r.wrong_pattern == "cargo test" && r.right_pattern == "cargo test");

    assert!(
        cargo_repeat.is_none(),
        "TDD cycle (same command, compile error) should be filtered"
    );
}

#[test]
fn test_total_correction_count() {
    let rules = pipeline_from_fixture("learn_session.jsonl");

    // Fixture has 2 genuine corrections: git commit typo + gh pr flag
    assert_eq!(
        rules.len(),
        2,
        "expected exactly 2 correction rules: got {:?}",
        rules.iter().map(|r| &r.wrong_pattern).collect::<Vec<_>>()
    );
}

// =========================================================
//  Env prefix stripping
// =========================================================

#[test]
fn test_env_prefix_stripped_before_base_command() {
    // GIT_PAGER=cat git log ... → base = "git log"
    assert_eq!(
        extract_base_command("GIT_PAGER=cat git log --oneline -5"),
        "git log"
    );
    assert_eq!(
        extract_base_command("NODE_ENV=test CI=1 npx vitest run"),
        "npx vitest"
    );
    assert_eq!(
        extract_base_command("sudo cargo build --release"),
        "cargo build"
    );
}

// =========================================================
//  Corrections store
// =========================================================

#[test]
fn test_corrections_store_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("cli-corrections.json").to_string_lossy().into_owned();

    let corrections = vec![
        UserCorrection {
            wrong: "git commit --ammend".to_string(),
            right: "git commit --amend".to_string(),
        },
        UserCorrection {
            wrong: "gh pr edit -t".to_string(),
            right: "gh pr edit --title".to_string(),
        },
    ];

    write_corrections_json(&corrections, &path).unwrap();
    let loaded = load_corrections(&path);

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].wrong, "git commit --ammend");
    assert_eq!(loaded[0].right, "git commit --amend");
}

#[test]
fn test_apply_correction_exact_match() {
    let corrections = vec![UserCorrection {
        wrong: "git commit --ammend".to_string(),
        right: "git commit --amend".to_string(),
    }];

    assert_eq!(
        apply_correction("git commit --ammend", &corrections),
        Some("git commit --amend".to_string())
    );
    assert_eq!(apply_correction("git commit --amend", &corrections), None);
    assert_eq!(apply_correction("ls", &corrections), None);
}
