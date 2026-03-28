//! Shared hook audit log parsing and summarization.
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Default log file location (aligned with the platform data directory).
pub(crate) fn default_log_path() -> PathBuf {
    if let Ok(dir) = std::env::var("MYCELIUM_AUDIT_DIR") {
        PathBuf::from(dir).join("hook-audit.log")
    } else {
        crate::platform::mycelium_data_dir()
            .unwrap_or_else(|| PathBuf::from(".").join("mycelium"))
            .join("hook-audit.log")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditEntry {
    pub timestamp: String,
    pub action: String,
    pub original_cmd: String,
    pub rewritten_cmd: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditBucket {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AuditSummary {
    pub path: PathBuf,
    pub total: usize,
    pub rewrites: usize,
    pub skips: usize,
    pub rewrite_pct: f64,
    pub actionable_total: usize,
    pub actionable_rewrites: usize,
    pub actionable_coverage_pct: f64,
    pub skip_breakdown: Vec<AuditBucket>,
    pub top_rewrites: Vec<AuditBucket>,
}

/// Parse a single log line: "timestamp | action | original_cmd | rewritten_cmd"
pub(crate) fn parse_line(line: &str) -> Option<AuditEntry> {
    let parts: Vec<&str> = line.splitn(4, " | ").collect();
    if parts.len() < 3 {
        return None;
    }
    Some(AuditEntry {
        timestamp: parts[0].to_string(),
        action: parts[1].to_string(),
        original_cmd: parts[2].to_string(),
        rewritten_cmd: parts.get(3).unwrap_or(&"-").to_string(),
    })
}

pub(crate) fn parse_entries(content: &str) -> Vec<AuditEntry> {
    content.lines().filter_map(parse_line).collect()
}

/// Extract the base command (first 1-2 words) for grouping.
pub(crate) fn base_command(cmd: &str) -> String {
    let stripped = cmd
        .split_whitespace()
        .skip_while(|word| word.contains('='))
        .collect::<Vec<_>>();

    match stripped.len() {
        0 => cmd.to_string(),
        1 => stripped[0].to_string(),
        _ => format!("{} {}", stripped[0], stripped[1]),
    }
}

pub(crate) fn filter_since_days(entries: &[AuditEntry], days: u64) -> Vec<&AuditEntry> {
    if days == 0 {
        return entries.iter().collect();
    }

    let cutoff = jiff::Timestamp::now()
        .checked_sub(jiff::SignedDuration::from_hours(days as i64 * 24))
        .expect("timestamp subtraction should not overflow");
    let cutoff_str = cutoff.to_string();

    entries
        .iter()
        .filter(|entry| entry.timestamp >= cutoff_str)
        .collect()
}

fn actionable_action(action: &str) -> bool {
    matches!(
        action,
        "rewrite"
            | "skip:no_match"
            | "skip:no_mycelium"
            | "skip:no_jq"
            | "skip:old_version"
            | "skip:jq_parse_error"
    )
}

pub(crate) fn summarize_entries(
    entries: &[AuditEntry],
    since_days: u64,
    path: PathBuf,
) -> Option<AuditSummary> {
    let filtered = filter_since_days(entries, since_days);
    if filtered.is_empty() {
        return None;
    }

    let mut action_counts: HashMap<&str, usize> = HashMap::new();
    let mut rewrite_counts: HashMap<String, usize> = HashMap::new();

    for entry in &filtered {
        *action_counts.entry(&entry.action).or_insert(0) += 1;
        if entry.action == "rewrite" {
            *rewrite_counts
                .entry(base_command(&entry.original_cmd))
                .or_insert(0) += 1;
        }
    }

    let total = filtered.len();
    let rewrites = action_counts.get("rewrite").copied().unwrap_or(0);
    let skips = total.saturating_sub(rewrites);
    let rewrite_pct = percentage(rewrites, total);

    let actionable_total = action_counts
        .iter()
        .filter(|(action, _)| actionable_action(action))
        .map(|(_, count)| *count)
        .sum::<usize>();
    let actionable_coverage_pct = if actionable_total == 0 {
        100.0
    } else {
        percentage(rewrites, actionable_total)
    };

    let mut skip_breakdown = action_counts
        .iter()
        .filter(|(action, _)| action.starts_with("skip:"))
        .map(|(action, count)| AuditBucket {
            name: action.strip_prefix("skip:").unwrap_or(action).to_string(),
            count: *count,
        })
        .collect::<Vec<_>>();
    skip_breakdown.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });

    let mut top_rewrites = rewrite_counts
        .into_iter()
        .map(|(name, count)| AuditBucket { name, count })
        .collect::<Vec<_>>();
    top_rewrites.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });

    Some(AuditSummary {
        path,
        total,
        rewrites,
        skips,
        rewrite_pct,
        actionable_total,
        actionable_rewrites: rewrites,
        actionable_coverage_pct,
        skip_breakdown,
        top_rewrites,
    })
}

pub(crate) fn load_summary(since_days: u64) -> Result<Option<AuditSummary>> {
    let path = default_log_path();
    if !path.exists() {
        return Ok(None);
    }

    let content =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let entries = parse_entries(&content);
    Ok(summarize_entries(&entries, since_days, path))
}

fn percentage(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64 * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(action: &str, cmd: &str) -> AuditEntry {
        AuditEntry {
            timestamp: "2026-03-28T12:00:00Z".to_string(),
            action: action.to_string(),
            original_cmd: cmd.to_string(),
            rewritten_cmd: "-".to_string(),
        }
    }

    #[test]
    fn test_parse_line_rewrite() {
        let line = "2026-02-16T14:30:01Z | rewrite | git status | mycelium git status";
        let entry = parse_line(line).unwrap();
        assert_eq!(entry.action, "rewrite");
        assert_eq!(entry.original_cmd, "git status");
        assert_eq!(entry.rewritten_cmd, "mycelium git status");
    }

    #[test]
    fn test_base_command_with_env() {
        assert_eq!(base_command("GIT_PAGER=cat git status"), "git status");
        assert_eq!(base_command("NODE_ENV=test CI=1 npx vitest"), "npx vitest");
    }

    #[test]
    fn test_summarize_entries_computes_actionable_coverage() {
        let entries = vec![
            make_entry("rewrite", "git status"),
            make_entry("rewrite", "cargo test"),
            make_entry("skip:no_match", "echo hello"),
            make_entry("skip:already_mycelium", "mycelium git status"),
            make_entry("skip:heredoc", "cat <<'EOF'"),
        ];

        let summary = summarize_entries(&entries, 0, PathBuf::from("/tmp/hook-audit.log"))
            .expect("expected summary");

        assert_eq!(summary.total, 5);
        assert_eq!(summary.rewrites, 2);
        assert_eq!(summary.actionable_total, 3);
        assert!((summary.actionable_coverage_pct - 66.666).abs() < 0.1);
        assert_eq!(summary.skip_breakdown[0].name, "already_mycelium");
        assert_eq!(summary.top_rewrites[0].count, 1);
    }
}
