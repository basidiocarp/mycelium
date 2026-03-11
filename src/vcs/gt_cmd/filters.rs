//! Filter functions for Graphite (gt) CLI output token reduction.
use crate::utils::{ok_confirmation, truncate};
use regex::Regex;
use std::sync::OnceLock;

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap())
}

fn branch_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?:Created|Pushed|pushed|Deleted|deleted)\s+branch\s+[`"']?([a-zA-Z0-9/_.\-+@]+)"#,
        )
        .expect("regex: gt branch action")
    })
}

fn pr_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(Created|Updated)\s+pull\s+request\s+#(\d+)\s+for\s+([^\s:]+)(?::\s*(\S+))?")
            .expect("regex: gt pull request action")
    })
}

pub(super) const MAX_LOG_ENTRIES: usize = 15;

pub(super) fn filter_identity(input: &str) -> String {
    input.to_string()
}

pub(super) fn filter_gt_log_entries(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = trimmed.lines().collect();
    let mut result = Vec::new();
    let mut entry_count = 0;

    for (i, line) in lines.iter().enumerate() {
        if is_graph_node(line) {
            entry_count += 1;
        }

        let replaced = email_re().replace_all(line, "");
        let processed = truncate(replaced.trim_end(), 120);
        result.push(processed);

        if entry_count >= MAX_LOG_ENTRIES {
            let remaining = lines[i + 1..].iter().filter(|l| is_graph_node(l)).count();
            if remaining > 0 {
                result.push(format!("... +{} more entries", remaining));
            }
            break;
        }
    }

    result.join("\n")
}

pub(super) fn filter_gt_submit(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut pushed = Vec::new();
    let mut prs = Vec::new();

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.contains("pushed") || line.contains("Pushed") {
            pushed.push(extract_branch_name(line));
        } else if let Some(caps) = pr_line_re().captures(line) {
            let action = caps[1].to_lowercase();
            let num = &caps[2];
            let branch = &caps[3];
            if let Some(url) = caps.get(4) {
                prs.push(format!(
                    "{} PR #{} {} {}",
                    action,
                    num,
                    branch,
                    url.as_str()
                ));
            } else {
                prs.push(format!("{} PR #{} {}", action, num, branch));
            }
        }
    }

    let mut summary = Vec::new();

    if !pushed.is_empty() {
        let branch_names: Vec<&str> = pushed
            .iter()
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .collect();
        if !branch_names.is_empty() {
            summary.push(format!("pushed {}", branch_names.join(", ")));
        } else {
            summary.push(format!("pushed {} branches", pushed.len()));
        }
    }

    summary.extend(prs);

    if summary.is_empty() {
        return truncate(trimmed, 200);
    }

    summary.join("\n")
}

pub(super) fn filter_gt_sync(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut synced = 0;
    let mut deleted = 0;
    let mut deleted_names = Vec::new();

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if (line.contains("Synced") && line.contains("branch"))
            || line.starts_with("Synced with remote")
        {
            synced += 1;
        }
        if line.contains("deleted") || line.contains("Deleted") {
            deleted += 1;
            let name = extract_branch_name(line);
            if !name.is_empty() {
                deleted_names.push(name);
            }
        }
    }

    let mut parts = Vec::new();

    if synced > 0 {
        parts.push(format!("{} synced", synced));
    }

    if deleted > 0 {
        if deleted_names.is_empty() {
            parts.push(format!("{} deleted", deleted));
        } else {
            parts.push(format!(
                "{} deleted ({})",
                deleted,
                deleted_names.join(", ")
            ));
        }
    }

    if parts.is_empty() {
        return ok_confirmation("synced", "");
    }

    format!("ok sync: {}", parts.join(", "))
}

pub(super) fn filter_gt_restack(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut restacked = 0;
    for line in trimmed.lines() {
        let line = line.trim();
        if (line.contains("Restacked") || line.contains("Rebased")) && line.contains("branch") {
            restacked += 1;
        }
    }

    if restacked > 0 {
        ok_confirmation("restacked", &format!("{} branches", restacked))
    } else {
        ok_confirmation("restacked", "")
    }
}

pub(super) fn filter_gt_create(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let branch_name = trimmed
        .lines()
        .find_map(|line| {
            let line = line.trim();
            if line.contains("Created") || line.contains("created") {
                Some(extract_branch_name(line))
            } else {
                None
            }
        })
        .unwrap_or_default();

    if branch_name.is_empty() {
        let first_line = trimmed.lines().next().unwrap_or("");
        ok_confirmation("created", first_line.trim())
    } else {
        ok_confirmation("created", &branch_name)
    }
}

pub(super) fn is_graph_node(line: &str) -> bool {
    let stripped = line
        .trim_start_matches('│')
        .trim_start_matches('|')
        .trim_start();
    stripped.starts_with('◉')
        || stripped.starts_with('○')
        || stripped.starts_with('◯')
        || stripped.starts_with('◆')
        || stripped.starts_with('●')
        || stripped.starts_with('@')
        || stripped.starts_with('*')
}

pub(super) fn extract_branch_name(line: &str) -> String {
    branch_name_re()
        .captures(line)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "filters_tests.rs"]
mod tests;
