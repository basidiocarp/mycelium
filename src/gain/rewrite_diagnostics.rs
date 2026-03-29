//! Rewrite coverage, quality, and passthrough diagnostics for `mycelium gain`.
use anyhow::Result;

use crate::hook_audit;
use crate::tracking::{PassthroughSummary, Tracker};

const DEFAULT_AUDIT_DAYS: u64 = 7;

pub(crate) fn show_rewrite_diagnostics(
    tracker: &Tracker,
    project_scope: Option<&str>,
    explain: bool,
) -> Result<()> {
    let audit_summary = hook_audit::load_summary(DEFAULT_AUDIT_DAYS)?;
    let passthrough = tracker.get_passthrough_summary_filtered(project_scope)?;
    let parse_failures = tracker.get_parse_failure_summary_filtered(project_scope)?;
    let quality_score = compute_quality_score(audit_summary.as_ref(), parse_failures.recovery_rate);

    println!("Rewrite Diagnostics");
    println!("{}", "═".repeat(60));
    for line in scope_banner_lines(project_scope) {
        println!("{line}");
    }
    println!("Quality score: {:.1}/100", quality_score);
    println!(
        "Parse recovery (global): {:.1}% ({} failure record{})",
        parse_failures.recovery_rate,
        parse_failures.total,
        if parse_failures.total == 1 { "" } else { "s" }
    );
    println!();

    println!("Hook coverage (global, last {} days):", DEFAULT_AUDIT_DAYS);
    if let Some(summary) = &audit_summary {
        println!("  Total audited invocations: {}", summary.total);
        println!(
            "  Rewrites:                {} ({:.1}%)",
            summary.rewrites, summary.rewrite_pct
        );
        println!(
            "  Actionable coverage:     {:.1}% ({}/{})",
            summary.actionable_coverage_pct, summary.actionable_rewrites, summary.actionable_total
        );
        if !summary.skip_breakdown.is_empty() {
            println!("  Top skip reasons:");
            for bucket in summary.skip_breakdown.iter().take(5) {
                println!("    - {} ({})", bucket.name, bucket.count);
            }
        }
    } else {
        println!(
            "  No hook audit log found. Enable `MYCELIUM_HOOK_AUDIT=1` to capture rewrite decisions."
        );
    }
    println!();

    print_passthrough_section(&passthrough, project_scope);

    if !parse_failures.top_commands.is_empty() {
        println!("Parse failures:");
        for (command, count) in parse_failures.top_commands.iter().take(5) {
            println!("  - {} ({})", command, count);
        }
    } else {
        println!("Parse failures: none recorded");
    }

    if explain {
        println!();
        println!("Diagnostics explanation:");
        for line in explanation_lines(project_scope) {
            println!("  {}", line);
        }
    }

    Ok(())
}

fn print_passthrough_section(summary: &PassthroughSummary, project_scope: Option<&str>) {
    println!("{}", passthrough_heading(project_scope));
    println!(
        "  Recorded passthroughs: {} ({} ms total)",
        summary.total_commands, summary.total_exec_time_ms
    );
    if summary.top_commands.is_empty() {
        println!("  No passthrough commands recorded.");
    } else {
        println!("  Top passthrough commands:");
        for stat in &summary.top_commands {
            println!(
                "    - {} ({} runs, {} ms)",
                stat.command, stat.count, stat.total_exec_time_ms
            );
        }
    }
    println!();
}

fn scope_banner_lines(project_scope: Option<&str>) -> Vec<String> {
    project_scope.map_or_else(Vec::new, |scope| {
        vec![
            format!("Project-scoped passthrough view: {}", scope),
            "Global metrics: quality score, hook coverage, and parse recovery".to_string(),
        ]
    })
}

fn passthrough_heading(project_scope: Option<&str>) -> &'static str {
    if project_scope.is_some() {
        "Passthrough commands (project-scoped):"
    } else {
        "Passthrough commands (global):"
    }
}

fn explanation_lines(project_scope: Option<&str>) -> Vec<String> {
    let mut lines = vec![
        "Quality score formula: 70% hook coverage + 30% parse recovery.".to_string(),
        "Hook coverage and parse recovery are global metrics derived from all recorded hook activity and parse-failure rows.".to_string(),
    ];

    if let Some(scope) = project_scope {
        lines.push(format!(
            "Passthrough summaries are filtered to project scope: {}.",
            scope
        ));
    } else {
        lines.push(
            "Passthrough summaries are global when no project scope is provided.".to_string(),
        );
    }

    lines
}

pub(crate) fn compute_quality_score(
    audit_summary: Option<&hook_audit::AuditSummary>,
    parse_recovery_pct: f64,
) -> f64 {
    let coverage_pct = audit_summary
        .map(|summary| summary.actionable_coverage_pct)
        .unwrap_or(0.0);
    ((coverage_pct * 0.7) + (parse_recovery_pct * 0.3)).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_audit::{AuditBucket, AuditSummary};
    use std::path::PathBuf;

    #[test]
    fn test_compute_quality_score_combines_coverage_and_parse_recovery() {
        let audit = AuditSummary {
            path: PathBuf::from("/tmp/hook-audit.log"),
            total: 10,
            rewrites: 6,
            skips: 4,
            rewrite_pct: 60.0,
            actionable_total: 8,
            actionable_rewrites: 6,
            actionable_coverage_pct: 75.0,
            skip_breakdown: vec![AuditBucket {
                name: "no_match".to_string(),
                count: 2,
            }],
            top_rewrites: vec![AuditBucket {
                name: "git status".to_string(),
                count: 3,
            }],
        };

        let score = compute_quality_score(Some(&audit), 50.0);
        assert!((score - 67.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_banner_lines_explain_mixed_scope() {
        let lines = scope_banner_lines(Some("/tmp/project"));
        assert_eq!(
            lines,
            vec![
                "Project-scoped passthrough view: /tmp/project".to_string(),
                "Global metrics: quality score, hook coverage, and parse recovery".to_string(),
            ]
        );
    }

    #[test]
    fn test_passthrough_heading_reflects_scope() {
        assert_eq!(
            passthrough_heading(Some("/tmp/project")),
            "Passthrough commands (project-scoped):"
        );
        assert_eq!(passthrough_heading(None), "Passthrough commands (global):");
    }

    #[test]
    fn test_explain_lines_describe_quality_formula() {
        let lines = explanation_lines(Some("/tmp/project"));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Quality score formula"))
        );
        assert!(lines.iter().any(|line| line.contains("70% hook coverage")));
    }
}
