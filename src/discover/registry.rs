//! Classifies shell commands against the rule set.
use regex::{Regex, RegexSet};
use std::sync::OnceLock;

use super::rules::{IGNORED_EXACT, IGNORED_PREFIXES, PATTERNS, RULES};

// Re-export the rewrite API from the rewriter module
// Note: rewrite_command is pub; others are pub(crate) for internal use
#[cfg(test)]
pub(crate) use super::rewriter::set_find_fd_rewrite_active_for_tests;
#[allow(unused_imports)]
pub(crate) use super::rewriter::{
    learned_correction_block_reason, rewrite_block_reason, rewrite_primary_command,
};
#[allow(unused_imports)]
pub use super::rewriter::{rewrite_command, rewrite_command_with_prefixes};

#[path = "registry_compound.rs"]
pub(super) mod compound;
#[path = "registry_parser.rs"]
pub(super) mod parser;
#[path = "registry_shell.rs"]
pub(super) mod shell;

/// Result of classifying a command.
#[derive(Debug, PartialEq)]
pub enum Classification {
    Supported {
        mycelium_equivalent: &'static str,
        category: &'static str,
        estimated_savings_pct: f64,
        status: super::report::MyceliumStatus,
    },
    Unsupported {
        base_command: String,
    },
    Ignored,
}

/// Average token counts per category for estimation when no `output_len` available.
#[must_use]
pub fn category_avg_tokens(category: &str, subcmd: &str) -> usize {
    match category {
        "Git" => match subcmd {
            "log" | "diff" | "show" => 200,
            _ => 40,
        },
        "Cargo" => match subcmd {
            "test" => 500,
            _ => 150,
        },
        "Tests" => 800,
        "Files" => 100,
        "Build" => 300,
        "Infra" => 120,
        "GitHub" => 200,
        _ => 150,
    }
}

fn regex_set() -> &'static RegexSet {
    static RE: OnceLock<RegexSet> = OnceLock::new();
    RE.get_or_init(|| RegexSet::new(PATTERNS).expect("invalid regex patterns"))
}

pub(crate) const DIAGNOSTIC_PASSTHROUGH: &[&str] = &[
    "which",
    "type",
    "file",
    "stat",
    "otool",
    "ldd",
    "readelf",
    "uname",
    "whoami",
    "hostname",
    "printenv",
    "echo",
    "printf",
    "id",
    "groups",
    "locale",
    "sw_vers",
    "xcode-select",
    "rustup",
    "nvm",
    "pyenv",
    // File operations — produce short confirmation output, should not be filtered
    "timeout",
    "mv",
    "cp",
    "chmod",
    "mkdir",
    "rm",
    "touch",
    "ln",
    "codesign",
    "xattr",
];

fn compiled() -> &'static Vec<Regex> {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        PATTERNS
            .iter()
            .map(|p| Regex::new(p).expect("invalid regex"))
            .collect()
    })
}

#[must_use]
pub fn split_command_chain(cmd: &str) -> Vec<&str> {
    compound::split_command_chain(cmd)
}

/// Normalize a command string for discover reports.
///
/// This trims wrappers like `mise exec --` and `just --`, then keeps the first
/// two words for display so reports surface the underlying command shape.
pub(crate) fn display_command_for_discover(cmd: &str) -> String {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let (_, cmd_clean) = shell::strip_env_prefix_segments(trimmed);
    let display_source = shell::unwrap_task_runner_command(&cmd_clean).unwrap_or(&cmd_clean);
    let parts: Vec<&str> = display_source.splitn(3, char::is_whitespace).collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => format!("{} {}", parts[0], parts[1]),
    }
}

/// Classify a single (already-split) command.
#[must_use]
pub fn classify_command(cmd: &str) -> Classification {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return Classification::Ignored;
    }

    // Check ignored
    for exact in IGNORED_EXACT {
        if trimmed == *exact {
            return Classification::Ignored;
        }
    }
    for prefix in IGNORED_PREFIXES {
        if trimmed.starts_with(prefix) {
            return Classification::Ignored;
        }
    }

    // Strip env prefixes (sudo, env VAR=val, VAR=val)
    let (_, cmd_clean) = shell::strip_env_prefix_segments(trimmed);
    if cmd_clean.is_empty() {
        return Classification::Ignored;
    }

    if let Some(inner) = shell::unwrap_task_runner_command(&cmd_clean) {
        return classify_command(inner);
    }

    // Fast check with RegexSet — take the last (most specific) match
    let matches: Vec<usize> = regex_set().matches(&cmd_clean).into_iter().collect();
    if let Some(&idx) = matches.last() {
        let rule = &RULES[idx];

        // Extract subcommand for savings override and status detection
        let (savings, status) = if let Some(caps) = compiled()[idx].captures(&cmd_clean) {
            if let Some(sub) = caps.get(1) {
                let subcmd = sub.as_str();
                // Check if this subcommand has a special status
                let status = rule
                    .subcmd_status
                    .iter()
                    .find(|(s, _)| *s == subcmd)
                    .map_or(super::report::MyceliumStatus::Existing, |(_, st)| *st);

                // Check if this subcommand has custom savings
                let savings = rule
                    .subcmd_savings
                    .iter()
                    .find(|(s, _)| *s == subcmd)
                    .map_or(rule.savings_pct, |(_, pct)| *pct);

                (savings, status)
            } else {
                (rule.savings_pct, super::report::MyceliumStatus::Existing)
            }
        } else {
            (rule.savings_pct, super::report::MyceliumStatus::Existing)
        };

        Classification::Supported {
            mycelium_equivalent: rule.mycelium_cmd,
            category: rule.category,
            estimated_savings_pct: savings,
            status,
        }
    } else {
        // Extract base command for unsupported
        let base = extract_base_command(&cmd_clean);
        if base.is_empty() {
            Classification::Ignored
        } else {
            Classification::Unsupported {
                base_command: base.to_string(),
            }
        }
    }
}

/// Extract the base command (first word, or first two if it looks like a subcommand pattern).
fn extract_base_command(cmd: &str) -> &str {
    let parts: Vec<&str> = cmd.splitn(3, char::is_whitespace).collect();
    match parts.len() {
        0 => "",
        1 => parts[0],
        _ => {
            let second = parts[1];
            // If the second token looks like a subcommand (no leading -)
            if !second.starts_with('-') && !second.contains('/') && !second.contains('.') {
                // Return "cmd subcmd"
                let end = cmd
                    .find(char::is_whitespace)
                    .and_then(|i| {
                        let rest = &cmd[i..];
                        let trimmed = rest.trim_start();
                        trimmed
                            .find(char::is_whitespace)
                            .map(|j| i + (rest.len() - trimmed.len()) + j)
                    })
                    .unwrap_or(cmd.len());
                &cmd[..end]
            } else {
                parts[0]
            }
        }
    }
}

pub(crate) fn is_diagnostic_passthrough_command(cmd: &str) -> bool {
    diagnostic_passthrough_base(cmd)
}

fn diagnostic_passthrough_base(cmd: &str) -> bool {
    let (_, cmd_clean) = shell::strip_env_prefix_segments(cmd);
    let effective = shell::unwrap_all_task_runner_commands(&cmd_clean);
    let Some(base) = effective.split_whitespace().next() else {
        return false;
    };

    if DIAGNOSTIC_PASSTHROUGH.contains(&base) {
        return true;
    }

    if base == "ls"
        && effective
            .split_whitespace()
            .skip(1)
            .any(|arg| arg.starts_with('-'))
    {
        return true;
    }

    false
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
