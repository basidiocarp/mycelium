use super::*;

fn count_tokens(text: &str) -> usize {
    crate::tracking::estimate_tokens(text)
}

#[test]
fn test_filter_gt_log_exact_format() {
    let input = r#"◉  abc1234 feat/add-auth 2d ago
│  feat(auth): add login endpoint
│
◉  def5678 feat/add-db 3d ago user@example.com
│  feat(db): add migration system
│
◉  ghi9012 main 5d ago admin@corp.io
│  chore: update dependencies
~
"#;
    let output = filter_gt_log_entries(input);
    let expected = "\
◉  abc1234 feat/add-auth 2d ago
│  feat(auth): add login endpoint
│
◉  def5678 feat/add-db 3d ago
│  feat(db): add migration system
│
◉  ghi9012 main 5d ago
│  chore: update dependencies
~";
    assert_eq!(output, expected);
}

#[test]
fn test_filter_gt_submit_exact_format() {
    let input = r#"Pushed branch feat/add-auth
Created pull request #42 for feat/add-auth
Pushed branch feat/add-db
Updated pull request #40 for feat/add-db
"#;
    let output = filter_gt_submit(input);
    let expected = "\
pushed feat/add-auth, feat/add-db
created PR #42 feat/add-auth
updated PR #40 feat/add-db";
    assert_eq!(output, expected);
}

#[test]
fn test_filter_gt_sync_exact_format() {
    let input = r#"Synced with remote
Deleted branch feat/merged-feature
Deleted branch fix/old-hotfix
"#;
    let output = filter_gt_sync(input);
    assert_eq!(
        output,
        "ok sync: 1 synced, 2 deleted (feat/merged-feature, fix/old-hotfix)"
    );
}

#[test]
fn test_filter_gt_restack_exact_format() {
    let input = r#"Restacked branch feat/add-auth on main
Restacked branch feat/add-db on feat/add-auth
Restacked branch fix/parsing on feat/add-db
"#;
    let output = filter_gt_restack(input);
    assert_eq!(output, "ok restacked 3 branches");
}

#[test]
fn test_filter_gt_create_exact_format() {
    let input = "Created branch feat/new-feature\n";
    let output = filter_gt_create(input);
    assert_eq!(output, "ok created feat/new-feature");
}

#[test]
fn test_filter_gt_log_truncation() {
    let mut input = String::new();
    for i in 0..20 {
        input.push_str(&format!(
            "◉  hash{:02} branch-{} 1d ago dev@example.com\n│  commit message {}\n│\n",
            i, i, i
        ));
    }
    input.push_str("~\n");

    let output = filter_gt_log_entries(&input);
    assert!(output.contains("... +"));
}

#[test]
fn test_filter_gt_log_empty() {
    assert_eq!(filter_gt_log_entries(""), String::new());
    assert_eq!(filter_gt_log_entries("  "), String::new());
}

#[test]
fn test_filter_gt_log_token_savings() {
    let mut input = String::new();
    for i in 0..40 {
        input.push_str(&format!(
            "◉  hash{:02}abc feat/feature-{} {}d ago developer{}@longcompany.example.com\n\
             │  feat(module-{}): implement feature {} with detailed description of changes\n│\n",
            i,
            i,
            i + 1,
            i,
            i,
            i
        ));
    }
    input.push_str("~\n");

    let output = filter_gt_log_entries(&input);
    let input_tokens = count_tokens(&input);
    let output_tokens = count_tokens(&output);
    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
    assert!(
        savings >= 60.0,
        "gt log filter: expected >=60% savings, got {:.1}% ({} -> {} tokens)",
        savings,
        input_tokens,
        output_tokens
    );
}

#[test]
fn test_filter_gt_log_long() {
    let input = r#"◉  abc1234 feat/add-auth
│  Author: Dev User <dev@example.com>
│  Date: 2026-02-25 10:30:00 -0800
│
│  feat(auth): add login endpoint with OAuth2 support
│  and session management for web clients
│
◉  def5678 feat/add-db
│  Author: Other Dev <other@example.com>
│  Date: 2026-02-24 14:00:00 -0800
│
│  feat(db): add migration system
~
"#;

    let output = filter_gt_log_entries(input);
    assert!(output.contains("abc1234"));
    assert!(!output.contains("dev@example.com"));
    assert!(!output.contains("other@example.com"));
}

#[test]
fn test_filter_gt_submit_empty() {
    assert_eq!(filter_gt_submit(""), String::new());
}

#[test]
fn test_filter_gt_submit_with_urls() {
    let input = "Created pull request #42 for feat/add-auth: https://github.com/org/repo/pull/42\n";
    let output = filter_gt_submit(input);
    assert!(output.contains("PR #42"));
    assert!(output.contains("feat/add-auth"));
    assert!(output.contains("https://github.com/org/repo/pull/42"));
}

#[test]
fn test_filter_gt_submit_token_savings() {
    let input = r#"
  ✅  Pushing to remote...
  Enumerating objects: 15, done.
  Counting objects: 100% (15/15), done.
  Delta compression using up to 10 threads
  Compressing objects: 100% (8/8), done.
  Writing objects: 100% (10/10), 2.50 KiB | 2.50 MiB/s, done.
  Total 10 (delta 5), reused 0 (delta 0), pack-reused 0
  Pushed branch feat/add-auth to origin
  Creating pull request for feat/add-auth...
  Created pull request #42 for feat/add-auth: https://github.com/org/repo/pull/42
  ✅  Pushing to remote...
  Enumerating objects: 8, done.
  Counting objects: 100% (8/8), done.
  Delta compression using up to 10 threads
  Compressing objects: 100% (4/4), done.
  Writing objects: 100% (5/5), 1.20 KiB | 1.20 MiB/s, done.
  Total 5 (delta 3), reused 0 (delta 0), pack-reused 0
  Pushed branch feat/add-db to origin
  Updating pull request for feat/add-db...
  Updated pull request #40 for feat/add-db: https://github.com/org/repo/pull/40
  ✅  Pushing to remote...
  Enumerating objects: 5, done.
  Counting objects: 100% (5/5), done.
  Delta compression using up to 10 threads
  Compressing objects: 100% (3/3), done.
  Writing objects: 100% (3/3), 890 bytes | 890.00 KiB/s, done.
  Total 3 (delta 2), reused 0 (delta 0), pack-reused 0
  Pushed branch fix/parsing to origin
  All branches submitted successfully!
"#;

    let output = filter_gt_submit(input);
    let input_tokens = count_tokens(input);
    let output_tokens = count_tokens(&output);
    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
    assert!(
        savings >= 60.0,
        "gt submit filter: expected >=60% savings, got {:.1}% ({} -> {} tokens)",
        savings,
        input_tokens,
        output_tokens
    );
}

#[test]
fn test_filter_gt_sync() {
    let input = r#"Synced with remote
Deleted branch feat/merged-feature
Deleted branch fix/old-hotfix
"#;

    let output = filter_gt_sync(input);
    assert!(output.contains("ok sync"));
    assert!(output.contains("synced"));
    assert!(output.contains("deleted"));
}

#[test]
fn test_filter_gt_sync_empty() {
    assert_eq!(filter_gt_sync(""), String::new());
}

#[test]
fn test_filter_gt_sync_no_deletes() {
    let input = "Synced with remote\n";
    let output = filter_gt_sync(input);
    assert!(output.contains("ok sync"));
    assert!(output.contains("synced"));
    assert!(!output.contains("deleted"));
}

#[test]
fn test_filter_gt_restack() {
    let input = r#"Restacked branch feat/add-auth on main
Restacked branch feat/add-db on feat/add-auth
Restacked branch fix/parsing on feat/add-db
"#;

    let output = filter_gt_restack(input);
    assert!(output.contains("ok restacked"));
    assert!(output.contains("3 branches"));
}

#[test]
fn test_filter_gt_restack_empty() {
    assert_eq!(filter_gt_restack(""), String::new());
}

#[test]
fn test_filter_gt_create() {
    let input = "Created branch feat/new-feature\n";
    let output = filter_gt_create(input);
    assert_eq!(output, "ok created feat/new-feature");
}

#[test]
fn test_filter_gt_create_empty() {
    assert_eq!(filter_gt_create(""), String::new());
}

#[test]
fn test_filter_gt_create_no_branch_name() {
    let input = "Some unexpected output\n";
    let output = filter_gt_create(input);
    assert!(output.starts_with("ok created"));
}

#[test]
fn test_is_graph_node() {
    assert!(is_graph_node("◉  abc1234 main"));
    assert!(is_graph_node("○  def5678 feat/x"));
    assert!(is_graph_node("@  ghi9012 (current)"));
    assert!(is_graph_node("*  jkl3456 branch"));
    assert!(is_graph_node("│ ◉  nested node"));
    assert!(!is_graph_node("│  just a message line"));
    assert!(!is_graph_node("~"));
}

#[test]
fn test_extract_branch_name() {
    assert_eq!(
        extract_branch_name("Created branch feat/new-feature"),
        "feat/new-feature"
    );
    assert_eq!(
        extract_branch_name("Pushed branch fix/bug-123"),
        "fix/bug-123"
    );
    assert_eq!(
        extract_branch_name("Pushed branch feat/auth+session"),
        "feat/auth+session"
    );
    assert_eq!(extract_branch_name("Created branch user@fix"), "user@fix");
    assert_eq!(extract_branch_name("no branch here"), "");
}

#[test]
fn test_filter_gt_log_pre_stripped_input() {
    let input = "◉  abc1234 feat/x 1d ago user@test.com\n│  message\n~\n";
    let output = filter_gt_log_entries(input);
    assert!(output.contains("abc1234"));
    assert!(!output.contains("user@test.com"));
}

#[test]
fn test_filter_gt_sync_token_savings() {
    let input = r#"
  ✅ Syncing with remote...
  Pulling latest changes from main...
  Successfully pulled 5 new commits
  Synced branch feat/add-auth with remote
  Synced branch feat/add-db with remote
  Branch feat/merged-feature has been merged
  Deleted branch feat/merged-feature
  Branch fix/old-hotfix has been merged
  Deleted branch fix/old-hotfix
  All branches synced!
"#;

    let output = filter_gt_sync(input);
    let input_tokens = count_tokens(input);
    let output_tokens = count_tokens(&output);
    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
    assert!(
        savings >= 60.0,
        "gt sync filter: expected >=60% savings, got {:.1}% ({} -> {} tokens)",
        savings,
        input_tokens,
        output_tokens
    );
}

#[test]
fn test_filter_gt_create_token_savings() {
    let input = r#"
  ✅ Creating new branch...
  Checking out from feat/add-auth...
  Created branch feat/new-feature from feat/add-auth
  Tracking branch set up to follow feat/add-auth
  Branch feat/new-feature is ready for development
"#;

    let output = filter_gt_create(input);
    let input_tokens = count_tokens(input);
    let output_tokens = count_tokens(&output);
    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
    assert!(
        savings >= 60.0,
        "gt create filter: expected >=60% savings, got {:.1}% ({} -> {} tokens)",
        savings,
        input_tokens,
        output_tokens
    );
}

#[test]
fn test_filter_gt_restack_token_savings() {
    let input = r#"
  ✅ Restacking branches...
  Restacked branch feat/add-auth on top of main
  Successfully rebased feat/add-auth (3 commits)
  Restacked branch feat/add-db on top of feat/add-auth
  Successfully rebased feat/add-db (2 commits)
  Restacked branch fix/parsing on top of feat/add-db
  Successfully rebased fix/parsing (1 commit)
  All branches restacked!
"#;

    let output = filter_gt_restack(input);
    let input_tokens = count_tokens(input);
    let output_tokens = count_tokens(&output);
    let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
    assert!(
        savings >= 60.0,
        "gt restack filter: expected >=60% savings, got {:.1}%",
        savings
    );
}
