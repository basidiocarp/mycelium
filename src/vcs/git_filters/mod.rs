//! Pure text filters for git output (status, log, diff, branch, stash, worktree).

pub(crate) mod diff;
pub(crate) mod status;

pub(crate) use diff::compact_diff;
pub(crate) use status::{filter_branch_output, filter_status_with_args, format_status_output};

/// Detect `rev:path` style arguments (blob show) while ignoring flags like
/// `--pretty=format:...`.
pub(crate) fn is_blob_show_arg(arg: &str) -> bool {
    !arg.starts_with('-') && arg.contains(':')
}

/// Filter git log output: truncate long messages, cap lines
pub(crate) fn filter_log_output(output: &str, limit: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    let capped: Vec<String> = lines
        .iter()
        .take(limit)
        .map(|line| {
            if line.len() > 80 {
                let truncated: String = line.chars().take(77).collect();
                format!("{}...", truncated)
            } else {
                line.to_string()
            }
        })
        .collect();

    capped.join("\n").trim().to_string()
}

/// Compact stash list: strip "WIP on branch:" prefix
pub(crate) fn filter_stash_list(output: &str) -> String {
    // Format: "stash@{0}: WIP on main: abc1234 commit message"
    let mut result = Vec::new();
    for line in output.lines() {
        if let Some(colon_pos) = line.find(": ") {
            let index = &line[..colon_pos];
            let rest = &line[colon_pos + 2..];
            // Compact: strip "WIP on branch:" prefix if present
            let message = if let Some(second_colon) = rest.find(": ") {
                rest[second_colon + 2..].trim()
            } else {
                rest.trim()
            };
            result.push(format!("{}: {}", index, message));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

/// Compact worktree list: shorten home directory paths
pub(crate) fn filter_worktree_list(output: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut result = Vec::new();
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // Format: "/path/to/worktree  abc1234 [branch]"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let mut path = parts[0].to_string();
            if !home.is_empty() && path.starts_with(&home) {
                path = format!("~{}", &path[home.len()..]);
            }
            let hash = parts[1];
            let branch = parts[2..].join(" ");
            result.push(format!("{} {} {}", path, hash, branch));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_blob_show_arg() {
        assert!(is_blob_show_arg("develop:modules/pairs_backtest.py"));
        assert!(is_blob_show_arg("HEAD:src/main.rs"));
        assert!(!is_blob_show_arg("--pretty=format:%h"));
        assert!(!is_blob_show_arg("--format=short"));
        assert!(!is_blob_show_arg("HEAD"));
    }

    #[test]
    fn test_filter_stash_list() {
        let output =
            "stash@{0}: WIP on main: abc1234 fix login\nstash@{1}: On feature: def5678 wip\n";
        let result = filter_stash_list(output);
        assert!(result.contains("stash@{0}: abc1234 fix login"));
        assert!(result.contains("stash@{1}: def5678 wip"));
    }

    #[test]
    fn test_filter_worktree_list() {
        let output =
            "/home/user/project  abc1234 [main]\n/home/user/worktrees/feat  def5678 [feature]\n";
        let result = filter_worktree_list(output);
        assert!(result.contains("abc1234"));
        assert!(result.contains("[main]"));
        assert!(result.contains("[feature]"));
    }

    #[test]
    fn test_filter_log_output() {
        let output = "abc1234 This is a commit message (2 days ago) <author>\ndef5678 Another commit (1 week ago) <other>\n";
        let result = filter_log_output(output, 10);
        assert!(result.contains("abc1234"));
        assert!(result.contains("def5678"));
        assert_eq!(result.lines().count(), 2);
    }

    #[test]
    fn test_filter_log_output_truncate_long() {
        let long_line = "abc1234 ".to_string() + &"x".repeat(100) + " (2 days ago) <author>";
        let result = filter_log_output(&long_line, 10);
        assert!(result.len() < long_line.len());
        assert!(result.contains("..."));
        assert!(result.len() <= 80);
    }

    #[test]
    fn test_filter_log_output_cap_lines() {
        let output = (0..20)
            .map(|i| format!("hash{} message {} (1 day ago) <author>", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = filter_log_output(&output, 5);
        assert_eq!(result.lines().count(), 5);
    }

    #[test]
    fn test_filter_log_output_multibyte() {
        // Thai characters: each is 3 bytes. A line with >80 bytes but few chars
        let thai_msg = format!("abc1234 {} (2 days ago) <author>", "ก".repeat(30));
        let result = filter_log_output(&thai_msg, 10);
        // Should not panic
        assert!(result.contains("abc1234"));
        // The line has 30 Thai chars (90 bytes) + other text, so > 80 bytes
        // It should be truncated with "..."
        assert!(result.contains("..."));
    }

    #[test]
    fn test_filter_log_output_emoji() {
        let emoji_msg = "abc1234 🎉🎊🎈🎁🎂🎄🎃🎆🎇✨🎉🎊🎈🎁🎂🎄🎃🎆🎇✨ (1 day ago) <user>";
        let result = filter_log_output(emoji_msg, 10);
        // Should not panic, should have "..."
        assert!(result.contains("..."));
    }
}
