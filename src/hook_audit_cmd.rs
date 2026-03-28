//! Analyzes the hook audit log to show rewrite statistics and skipped commands.
use anyhow::Result;

use crate::hook_audit;

/// Display hook rewrite statistics and skip reasons from the audit log.
pub fn run(since_days: u64, verbose: u8) -> Result<()> {
    let log_path = hook_audit::default_log_path();
    let Some(summary) = hook_audit::load_summary(since_days)? else {
        if !log_path.exists() {
            println!("No audit log found at {}", log_path.display());
            println!(
                "Enable audit mode: export MYCELIUM_HOOK_AUDIT=1 in your shell, then use Claude Code."
            );
        } else {
            println!("No entries in the last {} days.", since_days);
        }
        return Ok(());
    };

    let period = if since_days == 0 {
        "all time".to_string()
    } else {
        format!("last {} days", since_days)
    };

    println!("Hook Audit ({})", period);
    println!("{}", "─".repeat(30));
    println!("Total invocations: {}", summary.total);
    println!(
        "Rewrites:          {} ({:.1}%)",
        summary.rewrites, summary.rewrite_pct
    );
    println!(
        "Skips:             {} ({:.1}%)",
        summary.skips,
        100.0 - summary.rewrite_pct
    );
    println!(
        "Actionable cover:  {:.1}% ({}/{})",
        summary.actionable_coverage_pct, summary.actionable_rewrites, summary.actionable_total
    );

    if !summary.skip_breakdown.is_empty() {
        for bucket in &summary.skip_breakdown {
            println!(
                "  {}:{}{}",
                bucket.name,
                " ".repeat(14 - bucket.name.len().min(13)),
                bucket.count
            );
        }
    }

    if !summary.top_rewrites.is_empty() {
        let top = summary
            .top_rewrites
            .iter()
            .take(5)
            .map(|bucket| format!("{} ({})", bucket.name, bucket.count))
            .collect::<Vec<_>>();
        println!("Top commands: {}", top.join(", "));
    }

    if verbose > 0 {
        println!("\nLog: {}", summary.path.display());
    }

    Ok(())
}
