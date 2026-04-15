//! Git status, branch, and related output filters.

use crate::config::{CompactionProfile, current_compaction_tuning};

fn format_status_output_with_limit(porcelain: &str, max_status_files: usize) -> String {
    let lines: Vec<&str> = porcelain.lines().collect();

    if lines.is_empty() {
        return "Clean working tree".to_string();
    }

    let mut output = String::new();

    // Parse branch info
    if let Some(branch_line) = lines.first()
        && branch_line.starts_with("##")
    {
        let branch = branch_line.trim_start_matches("## ");
        output.push_str(&format!("Branch: {}\n", branch));
    }

    // Count changes by type
    let mut staged = 0;
    let mut modified = 0;
    let mut untracked = 0;
    let mut conflicts = 0;

    let mut staged_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut untracked_files = Vec::new();

    for line in lines.iter().skip(1) {
        if line.len() < 3 {
            continue;
        }
        let status = line.get(0..2).unwrap_or("  ");
        let file = line.get(3..).unwrap_or("");

        match status.chars().next().unwrap_or(' ') {
            'M' | 'A' | 'D' | 'R' | 'C' => {
                staged += 1;
                staged_files.push(file);
            }
            'U' => conflicts += 1,
            _ => {}
        }

        match status.chars().nth(1).unwrap_or(' ') {
            'M' | 'D' => {
                modified += 1;
                modified_files.push(file);
            }
            _ => {}
        }

        if status == "??" {
            untracked += 1;
            untracked_files.push(file);
        }
    }

    // Build summary with a modest total cap so large worktrees still fit in context.
    let total = staged_files.len() + modified_files.len() + untracked_files.len();
    let mut remaining_budget = max_status_files;

    if staged > 0 {
        output.push_str(&format!("Staged: {} files\n", staged));
        for f in staged_files.iter().take(remaining_budget) {
            output.push_str(&format!("   {}\n", f));
        }
        remaining_budget =
            remaining_budget.saturating_sub(staged_files.len().min(remaining_budget));
    }

    if modified > 0 {
        output.push_str(&format!("Modified: {} files\n", modified));
        for f in modified_files.iter().take(remaining_budget) {
            output.push_str(&format!("   {}\n", f));
        }
        remaining_budget =
            remaining_budget.saturating_sub(modified_files.len().min(remaining_budget));
    }

    if untracked > 0 {
        output.push_str(&format!("Untracked: {} files\n", untracked));
        for f in untracked_files.iter().take(remaining_budget) {
            output.push_str(&format!("   {}\n", f));
        }
    }

    if total > max_status_files {
        output.push_str(&format!(
            "   ... +{} more files\n",
            total - max_status_files
        ));
    }

    if conflicts > 0 {
        output.push_str(&format!("Conflicts: {} files\n", conflicts));
    }

    output.trim_end().to_string()
}

/// Format porcelain output into compact Mycelium status display using a named profile.
#[allow(
    dead_code,
    reason = "The profile-aware status surface is part of the public library API"
)]
pub fn format_status_output_with_profile(porcelain: &str, profile: CompactionProfile) -> String {
    format_status_output_with_limit(porcelain, profile.tuning().status_max_files)
}

/// Format porcelain output into compact Mycelium status display.
pub fn format_status_output(porcelain: &str) -> String {
    format_status_output_with_limit(porcelain, current_compaction_tuning().status_max_files)
}

/// Minimal filtering for git status with user-provided args
pub fn filter_status_with_args(output: &str) -> String {
    let mut result = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Skip git hints - can appear at start or within line
        if trimmed.starts_with("(use \"git")
            || trimmed.starts_with("(create/copy files")
            || trimmed.contains("(use \"git add")
            || trimmed.contains("(use \"git restore")
        {
            continue;
        }

        // Special case: clean working tree
        if trimmed.contains("nothing to commit") && trimmed.contains("working tree clean") {
            result.push(trimmed.to_string());
            break;
        }

        result.push(line.to_string());
    }

    if result.is_empty() {
        "ok ✓".to_string()
    } else {
        result.join("\n")
    }
}

/// Filter branch listing into compact format with current, local, and remote-only sections
///
/// Legacy text-parsing fallback. The binary now uses `git branch --format` for
/// structured output, but this remains in the library API for external callers.
#[allow(
    dead_code,
    reason = "Kept in library API as fallback for callers that receive pre-formatted branch output"
)]
pub fn filter_branch_output(output: &str) -> String {
    let mut current = String::new();
    let mut local: Vec<String> = Vec::new();
    let mut remote: Vec<String> = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(branch) = line.strip_prefix("* ") {
            current = branch.to_string();
        } else if line.starts_with("remotes/origin/") {
            let branch = line.strip_prefix("remotes/origin/").unwrap_or(line);
            // Skip HEAD pointer
            if branch.starts_with("HEAD ") {
                continue;
            }
            remote.push(branch.to_string());
        } else {
            local.push(line.to_string());
        }
    }

    let mut result = Vec::new();
    result.push(format!("* {}", current));

    if !local.is_empty() {
        for b in &local {
            result.push(format!("  {}", b));
        }
    }

    if !remote.is_empty() {
        // Filter out remotes that already exist locally
        let remote_only: Vec<&String> = remote
            .iter()
            .filter(|r| *r != &current && !local.contains(r))
            .collect();
        if !remote_only.is_empty() {
            result.push(format!("  remote-only ({}):", remote_only.len()));
            for b in remote_only.iter().take(10) {
                result.push(format!("    {}", b));
            }
            if remote_only.len() > 10 {
                result.push(format!("    ... +{} more", remote_only.len() - 10));
            }
        }
    }

    result.join("\n")
}

/// Format branch listing from pre-parsed structured data (from `git branch --format`).
///
/// Takes pre-separated current, local, and remote branch names instead of parsing
/// human-readable `git branch -a` output. This avoids regex fragility when git's
/// display format changes.
pub fn format_branch_structured(current: &str, local: &[String], remote: &[String]) -> String {
    let mut result = Vec::new();
    result.push(format!("* {}", current));

    for b in local {
        result.push(format!("  {}", b));
    }

    if !remote.is_empty() {
        // Filter out remotes that already exist locally
        let remote_only: Vec<&String> = remote
            .iter()
            .filter(|r| r.as_str() != current && !local.contains(r))
            .collect();
        if !remote_only.is_empty() {
            result.push(format!("  remote-only ({}):", remote_only.len()));
            for b in remote_only.iter().take(10) {
                result.push(format!("    {}", b));
            }
            if remote_only.len() > 10 {
                result.push(format!("    ... +{} more", remote_only.len() - 10));
            }
        }
    }

    result.join("\n")
}

/// Format worktree listing from `git worktree list --porcelain` output.
///
/// Porcelain format outputs one attribute per line with blank-line separators:
/// ```text
/// worktree /path/to/main
/// HEAD abc1234def5678
/// branch refs/heads/main
///
/// worktree /path/to/feature
/// HEAD def5678abc1234
/// branch refs/heads/feature
/// ```
///
/// This avoids parsing the space-aligned human-readable format.
pub fn format_worktree_porcelain(porcelain: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut result = Vec::new();
    let mut current_path = String::new();
    let mut current_head = String::new();
    let mut current_branch = String::new();
    let mut is_bare = false;

    for line in porcelain.lines() {
        if line.trim().is_empty() {
            // End of a worktree entry -- emit it
            if !current_path.is_empty() {
                let mut path = current_path.clone();
                if !home.is_empty() && path.starts_with(&home) {
                    path = format!("~{}", &path[home.len()..]);
                }
                let short_head = if current_head.len() > 7 {
                    &current_head[..7]
                } else {
                    &current_head
                };
                let branch_display = if is_bare {
                    "(bare)".to_string()
                } else if current_branch.is_empty() {
                    "(detached)".to_string()
                } else {
                    let short = current_branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(&current_branch);
                    format!("[{}]", short)
                };
                result.push(format!("{} {} {}", path, short_head, branch_display));
            }
            current_path.clear();
            current_head.clear();
            current_branch.clear();
            is_bare = false;
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            current_head = head.to_string();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = branch.to_string();
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Emit last entry if porcelain didn't end with blank line
    if !current_path.is_empty() {
        let mut path = current_path;
        if !home.is_empty() && path.starts_with(&home) {
            path = format!("~{}", &path[home.len()..]);
        }
        let short_head = if current_head.len() > 7 {
            &current_head[..7]
        } else {
            &current_head
        };
        let branch_display = if is_bare {
            "(bare)".to_string()
        } else if current_branch.is_empty() {
            "(detached)".to_string()
        } else {
            let short = current_branch
                .strip_prefix("refs/heads/")
                .unwrap_or(&current_branch);
            format!("[{}]", short)
        };
        result.push(format!("{} {} {}", path, short_head, branch_display));
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_branch_output() {
        let output = "* main\n  feature/auth\n  fix/bug-123\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/main\n  remotes/origin/feature/auth\n  remotes/origin/release/v2\n";
        let result = filter_branch_output(output);
        assert!(result.contains("* main"));
        assert!(result.contains("feature/auth"));
        assert!(result.contains("fix/bug-123"));
        // remote-only should show release/v2 but not main or feature/auth (already local)
        assert!(result.contains("remote-only"));
        assert!(result.contains("release/v2"));
    }

    #[test]
    fn test_filter_branch_no_remotes() {
        let output = "* main\n  develop\n";
        let result = filter_branch_output(output);
        assert!(result.contains("* main"));
        assert!(result.contains("develop"));
        assert!(!result.contains("remote-only"));
    }

    #[test]
    fn test_format_status_output_clean() {
        let porcelain = "";
        let result = format_status_output(porcelain);
        assert_eq!(result, "Clean working tree");
    }

    #[test]
    fn test_format_status_output_modified_files() {
        let porcelain = "## main...origin/main\n M src/main.rs\n M src/lib.rs\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("Branch: main...origin/main"));
        assert!(result.contains("Modified: 2 files"));
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("src/lib.rs"));
        assert!(!result.contains("Staged"));
        assert!(!result.contains("Untracked"));
    }

    #[test]
    fn test_format_status_output_untracked_files() {
        let porcelain = "## feature/new\n?? temp.txt\n?? debug.log\n?? test.sh\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("Branch: feature/new"));
        assert!(result.contains("Untracked: 3 files"));
        assert!(result.contains("temp.txt"));
        assert!(result.contains("debug.log"));
        assert!(result.contains("test.sh"));
        assert!(!result.contains("Modified"));
    }

    #[test]
    fn test_format_status_output_mixed_changes() {
        let porcelain = r#"## main
M  staged.rs
 M modified.rs
A  added.rs
?? untracked.txt
"#;
        let result = format_status_output(porcelain);
        assert!(result.contains("Branch: main"));
        assert!(result.contains("Staged: 2 files"));
        assert!(result.contains("staged.rs"));
        assert!(result.contains("added.rs"));
        assert!(result.contains("Modified: 1 files"));
        assert!(result.contains("modified.rs"));
        assert!(result.contains("Untracked: 1 files"));
        assert!(result.contains("untracked.txt"));
    }

    #[test]
    fn test_format_status_output_truncation() {
        // Test with 7 staged files - all shown since < 75 budget
        let porcelain = r#"## main
M  file1.rs
M  file2.rs
M  file3.rs
M  file4.rs
M  file5.rs
M  file6.rs
M  file7.rs
"#;
        let result = format_status_output(porcelain);
        assert!(result.contains("Staged: 7 files"));
        assert!(result.contains("file1.rs"));
        assert!(result.contains("file7.rs")); // All files show when < 50 total
        assert!(!result.contains("... +")); // No overflow when < 50
    }

    #[test]
    fn test_format_status_output_75_file_budget() {
        // Test that 30+30+30=90 files total shows "... +15 more files"
        let mut porcelain = "## main\n".to_string();

        // Add 30 staged files
        for i in 1..=30 {
            porcelain.push_str(&format!("M  staged{}.rs\n", i));
        }
        // Add 30 modified files
        for i in 1..=30 {
            porcelain.push_str(&format!(" M modified{}.rs\n", i));
        }
        // Add 30 untracked files
        for i in 1..=30 {
            porcelain.push_str(&format!("?? untracked{}.txt\n", i));
        }

        let result = format_status_output(&porcelain);

        // Should show summary lines
        assert!(result.contains("Staged: 30 files"));
        assert!(result.contains("Modified: 30 files"));
        assert!(result.contains("Untracked: 30 files"));

        // Should show first few from each category
        assert!(result.contains("staged1.rs"));
        assert!(result.contains("modified1.rs"));
        assert!(result.contains("untracked1.txt"));

        // Should show budget overflow message
        assert!(result.contains("... +15 more files"));

        // Staged and modified should all appear (first 60 of budget)
        assert!(result.contains("staged30.rs"));
        assert!(result.contains("modified30.rs"));

        // Only first 15 untracked should appear (budget exhausted)
        assert!(result.contains("untracked15.txt"));
        assert!(!result.contains("untracked16.txt"));
        assert!(!result.contains("untracked30.txt"));
    }

    #[test]
    fn test_filter_status_with_args() {
        let output = r#"On branch main
Your branch is up to date with 'origin/main'.

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
	modified:   src/main.rs

no changes added to commit (use "git add" and/or "git commit -a")
"#;
        let result = filter_status_with_args(output);
        eprintln!("Result:\n{}", result);
        assert!(result.contains("On branch main"));
        assert!(result.contains("modified:   src/main.rs"));
        assert!(
            !result.contains("(use \"git"),
            "Result should not contain git hints"
        );
    }

    #[test]
    fn test_filter_status_with_args_clean() {
        let output = "nothing to commit, working tree clean\n";
        let result = filter_status_with_args(output);
        assert!(result.contains("nothing to commit"));
    }

    #[test]
    fn test_format_status_output_thai_filename() {
        let porcelain = "## main\n M สวัสดี.txt\n?? ทดสอบ.rs\n";
        let result = format_status_output(porcelain);
        // Should not panic
        assert!(result.contains("Branch: main"));
        assert!(result.contains("สวัสดี.txt"));
        assert!(result.contains("ทดสอบ.rs"));
    }

    #[test]
    fn test_format_status_output_emoji_filename() {
        let porcelain = "## main\nA  🎉-party.txt\n M 日本語ファイル.rs\n";
        let result = format_status_output(porcelain);
        assert!(result.contains("Branch: main"));
    }

    #[test]
    fn test_format_status_output_debug_profile_preserves_more_files() {
        let mut porcelain = String::from("## debug\n");
        for i in 1..=100 {
            porcelain.push_str(&format!("?? file_{i}.txt\n"));
        }
        let result = format_status_output_with_profile(&porcelain, CompactionProfile::Debug);
        assert!(result.contains("file_100.txt"));
        assert!(!result.contains("more files"));
    }

    #[test]
    fn test_format_status_output_aggressive_profile_truncates_earlier() {
        let mut porcelain = String::from("## aggressive\n");
        for i in 1..=60 {
            porcelain.push_str(&format!("?? file_{i}.txt\n"));
        }
        let result = format_status_output_with_profile(&porcelain, CompactionProfile::Aggressive);
        assert!(result.contains("... +20 more files"));
    }

    // ── format_branch_structured tests ─────────────────────────────────

    #[test]
    fn test_format_branch_structured_basic() {
        let current = "main";
        let local = vec!["feature/auth".to_string(), "fix/bug-123".to_string()];
        let remote = vec![
            "main".to_string(),
            "feature/auth".to_string(),
            "release/v2".to_string(),
        ];
        let result = format_branch_structured(current, &local, &remote);
        assert!(result.contains("* main"));
        assert!(result.contains("  feature/auth"));
        assert!(result.contains("  fix/bug-123"));
        // remote-only should show release/v2 but not main or feature/auth (already local)
        assert!(result.contains("remote-only (1):"));
        assert!(result.contains("release/v2"));
    }

    #[test]
    fn test_format_branch_structured_no_remotes() {
        let current = "main";
        let local = vec!["develop".to_string()];
        let remote: Vec<String> = Vec::new();
        let result = format_branch_structured(current, &local, &remote);
        assert!(result.contains("* main"));
        assert!(result.contains("  develop"));
        assert!(!result.contains("remote-only"));
    }

    #[test]
    fn test_format_branch_structured_many_remotes() {
        let current = "main";
        let local: Vec<String> = Vec::new();
        let remote: Vec<String> = (1..=15).map(|i| format!("feature-{}", i)).collect();
        let result = format_branch_structured(current, &local, &remote);
        assert!(result.contains("remote-only (15):"));
        // First 10 shown
        assert!(result.contains("feature-10"));
        // 11+ truncated
        assert!(result.contains("... +5 more"));
    }

    #[test]
    fn test_format_branch_structured_matches_legacy_output() {
        // Verify the new function produces identical output to the legacy filter
        let current = "main";
        let local = vec!["feature/auth".to_string(), "fix/bug-123".to_string()];
        let remote = vec!["release/v2".to_string()];
        let structured = format_branch_structured(current, &local, &remote);

        // Build equivalent human-readable input for the legacy filter
        let human_readable = "* main\n  feature/auth\n  fix/bug-123\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/main\n  remotes/origin/feature/auth\n  remotes/origin/release/v2\n";
        let legacy = filter_branch_output(human_readable);

        assert_eq!(structured, legacy);
    }

    // ── format_worktree_porcelain tests ────────────────────────────────

    #[test]
    fn test_format_worktree_porcelain_basic() {
        let porcelain = "\
worktree /home/user/project
HEAD abc1234def5678901234567890abcdef12345678
branch refs/heads/main

worktree /home/user/worktrees/feat
HEAD def5678abc1234901234567890abcdef12345678
branch refs/heads/feature

";
        let result = format_worktree_porcelain(porcelain);
        assert!(result.contains("abc1234"));
        assert!(result.contains("[main]"));
        assert!(result.contains("[feature]"));
        assert_eq!(result.lines().count(), 2);
    }

    #[test]
    fn test_format_worktree_porcelain_detached_head() {
        let porcelain = "\
worktree /tmp/detached
HEAD abc1234def5678901234567890abcdef12345678
detached

";
        let result = format_worktree_porcelain(porcelain);
        assert!(result.contains("(detached)"));
    }

    #[test]
    fn test_format_worktree_porcelain_bare() {
        let porcelain = "\
worktree /tmp/bare-repo
HEAD abc1234def5678901234567890abcdef12345678
bare

";
        let result = format_worktree_porcelain(porcelain);
        assert!(result.contains("(bare)"));
    }

    #[test]
    fn test_format_worktree_porcelain_no_trailing_newline() {
        // Porcelain output without trailing blank line
        let porcelain = "\
worktree /tmp/project
HEAD abc1234def5678901234567890abcdef12345678
branch refs/heads/main";
        let result = format_worktree_porcelain(porcelain);
        assert!(result.contains("[main]"));
        assert!(result.contains("abc1234"));
    }

    #[test]
    fn test_format_worktree_porcelain_home_shortening() {
        let home = dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| "/home/user".to_string());

        let porcelain = format!(
            "worktree {home}/projects/myapp\nHEAD abc1234def5678\nbranch refs/heads/main\n\n"
        );
        let result = format_worktree_porcelain(&porcelain);
        assert!(
            result.contains("~/projects/myapp"),
            "Should shorten home directory to ~, got: {}",
            result
        );
    }
}
