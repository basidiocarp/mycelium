//! Summary view, time-breakdown printers, ASCII graph, and failure reporting.
use super::helpers::{
    colorize_pct_cell, mini_bar, print_efficiency_meter, print_kpi, shorten_path,
    style_command_cell, styled, truncate_for_column,
};
use crate::display_helpers::{format_duration, print_period_table};
use crate::tracking::Tracker;
use crate::utils::format_tokens;
use anyhow::{Context, Result};

/// Render the default summary view (KPI block + by-command table + optional graph/history/quota).
pub(crate) fn show_summary(
    tracker: &Tracker,
    project_scope: Option<&str>,
    graph: bool,
    history: bool,
    quota: bool,
    tier: &str,
) -> Result<()> {
    let summary = tracker
        .get_summary_filtered(project_scope)
        .context("Failed to load token savings summary from database")?;

    if summary.total_commands == 0 {
        println!("No tracking data yet.");
        println!("Run some mycelium commands to start tracking savings.");
        return Ok(());
    }

    let title = if project_scope.is_some() {
        "Mycelium Token Savings (Project Scope)"
    } else {
        "Mycelium Token Savings (Global Scope)"
    };
    println!("{}", styled(title, true));
    println!("{}", "═".repeat(60));
    if let Some(scope) = project_scope {
        println!("Scope: {}", shorten_path(scope));
    }
    println!();

    print_kpi("Total commands", summary.total_commands.to_string());
    print_kpi("Input tokens", format_tokens(summary.total_input));
    print_kpi("Output tokens", format_tokens(summary.total_output));
    print_kpi(
        "Tokens saved",
        format!(
            "{} ({:.1}%)",
            format_tokens(summary.total_saved),
            summary.avg_savings_pct
        ),
    );
    print_kpi(
        "Total exec time",
        format!(
            "{} (avg {})",
            format_duration(summary.total_time_ms),
            format_duration(summary.avg_time_ms)
        ),
    );
    print_efficiency_meter(summary.avg_savings_pct);
    println!();

    if !summary.by_command.is_empty() {
        println!("{}", styled("By Command", true));

        let cmd_width = 24usize;
        let impact_width = 10usize;
        let count_width = summary
            .by_command
            .iter()
            .map(|(_, count, _, _, _)| count.to_string().len())
            .max()
            .unwrap_or(5)
            .max(5);
        let saved_width = summary
            .by_command
            .iter()
            .map(|(_, _, saved, _, _)| format_tokens(*saved).len())
            .max()
            .unwrap_or(5)
            .max(5);
        let time_width = summary
            .by_command
            .iter()
            .map(|(_, _, _, _, avg_time)| format_duration(*avg_time).len())
            .max()
            .unwrap_or(6)
            .max(6);

        let table_width = 3
            + 2
            + cmd_width
            + 2
            + count_width
            + 2
            + saved_width
            + 2
            + 6
            + 2
            + time_width
            + 2
            + impact_width;
        println!("{}", "─".repeat(table_width));
        println!(
            "{:>3}  {:<cmd_width$}  {:>count_width$}  {:>saved_width$}  {:>6}  {:>time_width$}  {:<impact_width$}",
            "#", "Command", "Count", "Saved", "Avg%", "Time", "Impact",
            cmd_width = cmd_width, count_width = count_width,
            saved_width = saved_width, time_width = time_width,
            impact_width = impact_width
        );
        println!("{}", "─".repeat(table_width));

        let max_saved = summary
            .by_command
            .iter()
            .map(|(_, _, saved, _, _)| *saved)
            .max()
            .unwrap_or(1);

        for (idx, (cmd, count, saved, pct, avg_time)) in summary.by_command.iter().enumerate() {
            let row_idx = format!("{:>2}.", idx + 1);
            let cmd_cell = style_command_cell(&truncate_for_column(cmd, cmd_width));
            let count_cell = format!("{:>count_width$}", count, count_width = count_width);
            let saved_cell = format!(
                "{:>saved_width$}",
                format_tokens(*saved),
                saved_width = saved_width
            );
            let pct_plain = format!("{:>6}", format!("{pct:.1}%"));
            let pct_cell = colorize_pct_cell(*pct, &pct_plain);
            let time_cell = format!(
                "{:>time_width$}",
                format_duration(*avg_time),
                time_width = time_width
            );
            let impact = mini_bar(*saved, max_saved, impact_width);
            println!(
                "{}  {}  {}  {}  {}  {}  {}",
                row_idx, cmd_cell, count_cell, saved_cell, pct_cell, time_cell, impact
            );
        }
        println!("{}", "─".repeat(table_width));
        println!();
    }

    if graph && !summary.by_day.is_empty() {
        println!("{}", styled("Daily Savings (last 30 days)", true));
        println!("──────────────────────────────────────────────────────────");
        print_ascii_graph(&summary.by_day);
        println!();
    }

    if history {
        let recent = tracker.get_recent_filtered(10, project_scope)?;
        if !recent.is_empty() {
            println!("{}", styled("Recent Commands", true));
            println!("──────────────────────────────────────────────────────────");
            for rec in recent {
                let time = rec.timestamp.strftime("%m-%d %H:%M");
                let cmd_short = if rec.mycelium_cmd.len() > 25 {
                    format!("{}...", &rec.mycelium_cmd[..22])
                } else {
                    rec.mycelium_cmd.clone()
                };
                let sign = if rec.savings_pct >= 70.0 {
                    "▲"
                } else if rec.savings_pct >= 30.0 {
                    "■"
                } else {
                    "•"
                };
                println!(
                    "{} {} {:<25} -{:.0}% ({})",
                    time,
                    sign,
                    cmd_short,
                    rec.savings_pct,
                    format_tokens(rec.saved_tokens)
                );
            }
            println!();
        }
    }

    if quota {
        const ESTIMATED_PRO_MONTHLY: usize = 6_000_000;

        let (quota_tokens, tier_name) = match tier {
            "pro" => (ESTIMATED_PRO_MONTHLY, "Pro ($20/mo)"),
            "5x" => (ESTIMATED_PRO_MONTHLY * 5, "Max 5x ($100/mo)"),
            "20x" => (ESTIMATED_PRO_MONTHLY * 20, "Max 20x ($200/mo)"),
            _ => (ESTIMATED_PRO_MONTHLY, "Pro ($20/mo)"),
        };

        let quota_pct = (summary.total_saved as f64 / quota_tokens as f64) * 100.0;

        println!("{}", styled("Monthly Quota Analysis", true));
        println!("──────────────────────────────────────────────────────────");
        print_kpi("Subscription tier", tier_name.to_string());
        print_kpi("Estimated monthly quota", format_tokens(quota_tokens));
        print_kpi(
            "Tokens saved (lifetime)",
            format_tokens(summary.total_saved),
        );
        print_kpi("Quota preserved", format!("{:.1}%", quota_pct));
        println!();
        println!("Note: Heuristic estimate based on ~44K tokens/5h (Pro baseline)");
        println!("      Actual limits use rolling 5-hour windows, not monthly caps.");
    }

    Ok(())
}

fn print_ascii_graph(data: &[(String, usize)]) {
    if data.is_empty() {
        return;
    }

    let max_val = data.iter().map(|(_, v)| *v).max().unwrap_or(1);
    let width = 40;

    for (date, value) in data {
        let date_short = if date.len() >= 10 { &date[5..10] } else { date };

        let bar_len = if max_val > 0 {
            ((*value as f64 / max_val as f64) * width as f64) as usize
        } else {
            0
        };

        let bar: String = "█".repeat(bar_len);
        let spaces: String = " ".repeat(width - bar_len);

        println!(
            "{} │{}{} {}",
            date_short,
            bar,
            spaces,
            format_tokens(*value)
        );
    }
}

pub(crate) fn print_daily_full(tracker: &Tracker, project_scope: Option<&str>) -> Result<()> {
    let days = tracker.get_all_days_filtered(project_scope)?;
    print_period_table(&days);
    Ok(())
}

pub(crate) fn print_weekly(tracker: &Tracker, project_scope: Option<&str>) -> Result<()> {
    let weeks = tracker.get_by_week_filtered(project_scope)?;
    print_period_table(&weeks);
    Ok(())
}

pub(crate) fn print_monthly(tracker: &Tracker, project_scope: Option<&str>) -> Result<()> {
    let months = tracker.get_by_month_filtered(project_scope)?;
    print_period_table(&months);
    Ok(())
}

pub(crate) fn show_failures(tracker: &Tracker) -> Result<()> {
    let summary = tracker
        .get_parse_failure_summary()
        .context("Failed to load parse failure data")?;

    if summary.total == 0 {
        println!("No parse failures recorded.");
        println!("This means all commands parsed successfully (or fallback hasn't triggered yet).");
        return Ok(());
    }

    println!("{}", styled("Mycelium Parse Failures", true));
    println!("{}", "═".repeat(60));
    println!();

    print_kpi("Total failures", summary.total.to_string());
    print_kpi("Recovery rate", format!("{:.1}%", summary.recovery_rate));
    println!();

    if !summary.top_commands.is_empty() {
        println!("{}", styled("Top Commands (by frequency)", true));
        println!("{}", "─".repeat(60));
        for (cmd, count) in &summary.top_commands {
            let cmd_display = if cmd.len() > 50 {
                format!("{}...", &cmd[..47])
            } else {
                cmd.clone()
            };
            println!("  {:>4}x  {}", count, cmd_display);
        }
        println!();
    }

    if !summary.recent.is_empty() {
        println!("{}", styled("Recent Failures (last 10)", true));
        println!("{}", "─".repeat(60));
        for rec in &summary.recent {
            let ts_short = if rec.timestamp.len() >= 16 {
                &rec.timestamp[..16]
            } else {
                &rec.timestamp
            };
            let status = if rec.fallback_succeeded { "ok" } else { "FAIL" };
            let cmd_display = if rec.raw_command.len() > 40 {
                format!("{}...", &rec.raw_command[..37])
            } else {
                rec.raw_command.clone()
            };
            println!("  {} [{}] {}", ts_short, status, cmd_display);
        }
        println!();
    }

    Ok(())
}
