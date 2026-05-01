//! Parser health diagnostic command.
//!
//! Reports parse tier distribution from the tracking database, highlighting
//! commands with frequent degradation or passthrough results.
use crate::tracking::Tracker;
use anyhow::Result;
use colored::Colorize;

/// Run the parse-health diagnostic command.
pub fn run(days: u32) -> Result<()> {
    let tracker = match Tracker::new() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to open tracking database: {}", e);
            return Ok(());
        }
    };

    let rows = tracker.get_parse_health(days)?;

    if rows.is_empty() {
        println!(
            "No parse tier data yet. Parser health tracking starts after commands use the new parser framework."
        );
        return Ok(());
    }

    // Aggregate by command: collect tier counts
    use std::collections::HashMap;
    let mut by_cmd: HashMap<String, [usize; 3]> = HashMap::new(); // [tier1, tier2, tier3]
    for row in &rows {
        let entry = by_cmd.entry(row.command.clone()).or_insert([0, 0, 0]);
        match row.tier {
            1 => entry[0] += row.count,
            2 => entry[1] += row.count,
            3 => entry[2] += row.count,
            _ => {}
        }
    }

    println!("Parser Health (last {} days)\n{}", days, "═".repeat(50));
    println!(
        "{:<25} {:>6} {:>9} {:>11}  Health",
        "Command", "Full", "Degraded", "Passthrough"
    );
    println!("{}", "─".repeat(60));

    let mut cmd_list: Vec<_> = by_cmd.iter().collect();
    cmd_list.sort_by_key(|(cmd, _)| cmd.as_str());

    let mut total_full = 0usize;
    let mut total_all = 0usize;

    for (cmd, counts) in &cmd_list {
        let (full, degraded, passthrough) = (counts[0], counts[1], counts[2]);
        let total = full + degraded + passthrough;
        let health_pct = full.saturating_mul(100).checked_div(total).unwrap_or(100);
        let degradation_pct = (degraded + passthrough).saturating_mul(100).checked_div(total).unwrap_or(0);

        let health_str = format!("({}% healthy)", health_pct);
        let health_colored = if degradation_pct > 25 {
            health_str.red().to_string()
        } else if degradation_pct > 10 {
            health_str.yellow().to_string()
        } else {
            health_str.green().to_string()
        };

        let warn = if degradation_pct > 10 { " [!]" } else { "" };

        println!(
            "{:<25} {:>6} {:>9} {:>11}  {}{}",
            cmd, full, degraded, passthrough, health_colored, warn
        );

        total_full += full;
        total_all += total;
    }

    println!("{}", "─".repeat(60));
    let overall_pct = total_full.saturating_mul(100).checked_div(total_all).unwrap_or(100);
    println!(
        "\nOverall: {}/{} commands parsed at Tier 1 ({}% healthy)",
        total_full, total_all, overall_pct
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_health_runs() {
        // Just verify it doesn't panic with a real (possibly empty) DB
        let result = run(30);
        assert!(result.is_ok());
    }
}
