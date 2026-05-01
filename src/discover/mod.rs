//! Analyzes Claude Code and Codex session history to find commands that could benefit from Mycelium.
pub mod provider;
pub mod registry;
mod report;
pub mod rules;

use anyhow::Result;
use std::collections::HashMap;

use provider::{
    SessionSource, available_sources, discover_sessions, extract_commands,
    project_filter_for_source,
};
use registry::{
    Classification, category_avg_tokens, classify_command, display_command_for_discover,
    split_command_chain,
};
use report::{DiscoverReport, SupportedEntry, UnsupportedEntry};

/// Aggregation bucket for supported commands.
struct SupportedBucket {
    mycelium_equivalent: &'static str,
    category: &'static str,
    count: usize,
    total_output_tokens: usize,
    savings_pct: f64,
    // For display: the most common raw command
    command_counts: HashMap<String, usize>,
}

/// Aggregation bucket for unsupported commands.
struct UnsupportedBucket {
    count: usize,
    example: String,
}

/// Analyze Claude Code and Codex session history and report missed Mycelium savings opportunities.
pub fn run(
    project: Option<&str>,
    all: bool,
    since_days: u64,
    limit: usize,
    format: &str,
    verbose: u8,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let cwd_str = cwd.to_string_lossy().to_string();

    let mut sessions: Vec<(SessionSource, std::path::PathBuf)> = Vec::new();
    for source in available_sources() {
        let project_filter = project_filter_for_source(source, project, all, &cwd_str);
        let discovered = discover_sessions(source, project_filter.as_deref(), Some(since_days))?;
        sessions.extend(discovered.into_iter().map(|path| (source, path)));
    }

    if verbose > 0 {
        eprintln!("Scanning {} session files...", sessions.len());
        for (source, s) in &sessions {
            eprintln!("  [{}] {}", source.label(), s.display());
        }
    }

    let mut total_commands: usize = 0;
    let mut already_mycelium: usize = 0;
    let mut parse_errors: usize = 0;
    let mut supported_map: HashMap<&'static str, SupportedBucket> = HashMap::new();
    let mut unsupported_map: HashMap<String, UnsupportedBucket> = HashMap::new();

    for (source, session_path) in &sessions {
        let extracted = match extract_commands(*source, session_path) {
            Ok(cmds) => cmds,
            Err(e) => {
                if verbose > 0 {
                    eprintln!(
                        "Warning: skipping [{}] {}: {}",
                        source.label(),
                        session_path.display(),
                        e
                    );
                }
                parse_errors += 1;
                continue;
            }
        };

        for ext_cmd in &extracted {
            let parts = split_command_chain(&ext_cmd.command);
            for part in parts {
                total_commands += 1;

                match classify_command(part) {
                    Classification::Supported {
                        mycelium_equivalent,
                        category,
                        estimated_savings_pct,
                        status,
                    } => {
                        let bucket =
                            supported_map.entry(mycelium_equivalent).or_insert_with(|| {
                                SupportedBucket {
                                    mycelium_equivalent,
                                    category,
                                    count: 0,
                                    total_output_tokens: 0,
                                    savings_pct: estimated_savings_pct,
                                    command_counts: HashMap::new(),
                                }
                            });

                        bucket.count += 1;

                        // Estimate tokens for this command
                        let output_tokens = if let Some(len) = ext_cmd.output_len {
                            // Real: from tool_result content length
                            len / 4
                        } else {
                            // Fallback: category average
                            let subcmd = extract_subcmd(part);
                            category_avg_tokens(category, subcmd)
                        };

                        let savings =
                            (output_tokens as f64 * estimated_savings_pct / 100.0) as usize;
                        bucket.total_output_tokens += savings;

                        // Track the display name with status
                        let display_name = display_command_for_discover(part);
                        let entry = bucket
                            .command_counts
                            .entry(format!("{}:{:?}", display_name, status))
                            .or_insert(0);
                        *entry += 1;
                    }
                    Classification::Unsupported { base_command } => {
                        let bucket = unsupported_map.entry(base_command).or_insert_with(|| {
                            UnsupportedBucket {
                                count: 0,
                                example: display_command_for_discover(part),
                            }
                        });
                        bucket.count += 1;
                    }
                    Classification::Ignored => {
                        // Check if it starts with "mycelium "
                        if part.trim().starts_with("mycelium ") {
                            already_mycelium += 1;
                        }
                        // Otherwise just skip
                    }
                }
            }
        }
    }

    // Build report
    let mut supported: Vec<SupportedEntry> = supported_map
        .into_values()
        .map(|bucket| {
            // Pick the most common command as the display name
            let (command_with_status, status) = bucket
                .command_counts
                .into_iter()
                .max_by_key(|(_, c)| *c)
                .map(|(name, _)| {
                    // Extract status from "command:Status" format
                    if let Some(colon_pos) = name.rfind(':') {
                        let cmd = name[..colon_pos].to_string();
                        let status_str = &name[colon_pos + 1..];
                        let status = match status_str {
                            "Passthrough" => report::MyceliumStatus::Passthrough,
                            "NotSupported" => report::MyceliumStatus::NotSupported,
                            _ => report::MyceliumStatus::Existing,
                        };
                        (cmd, status)
                    } else {
                        (name, report::MyceliumStatus::Existing)
                    }
                })
                .unwrap_or_else(|| (String::new(), report::MyceliumStatus::Existing));

            SupportedEntry {
                command: command_with_status,
                count: bucket.count,
                mycelium_equivalent: bucket.mycelium_equivalent,
                category: bucket.category,
                estimated_savings_tokens: bucket.total_output_tokens,
                estimated_savings_pct: bucket.savings_pct,
                mycelium_status: status,
            }
        })
        .collect();

    // Sort by estimated savings descending
    supported.sort_by_key(|a| std::cmp::Reverse(a.estimated_savings_tokens));

    let mut unsupported: Vec<UnsupportedEntry> = unsupported_map
        .into_iter()
        .map(|(base, bucket)| UnsupportedEntry {
            base_command: base,
            count: bucket.count,
            example: bucket.example,
        })
        .collect();

    // Sort by count descending
    unsupported.sort_by_key(|a| std::cmp::Reverse(a.count));

    let report = DiscoverReport {
        sessions_scanned: sessions.len(),
        total_commands,
        already_mycelium,
        since_days,
        supported,
        unsupported,
        parse_errors,
    };

    match format {
        "json" => println!("{}", report::format_json(&report)),
        _ => print!("{}", report::format_text(&report, limit, verbose > 0)),
    }

    Ok(())
}

/// Extract the subcommand from a command string (second word).
fn extract_subcmd(cmd: &str) -> &str {
    let parts: Vec<&str> = cmd.trim().splitn(3, char::is_whitespace).collect();
    if parts.len() >= 2 { parts[1] } else { "" }
}
