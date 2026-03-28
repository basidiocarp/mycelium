//! Display functions for Claude Code economics in text format.

use anyhow::{Context, Result};

use crate::ccusage::{self, Granularity};
use crate::tracking::Tracker;
use crate::utils::{format_cpt, format_tokens, format_usd};

use super::merge::{compute_totals, merge_daily, merge_monthly, merge_weekly};
use super::models::PeriodEconomics;

fn load_daily_tracking(
    tracker: &Tracker,
    project_scope: Option<&str>,
) -> Result<Vec<crate::tracking::DayStats>> {
    match project_scope {
        Some(scope) => tracker.get_all_days_filtered(Some(scope)),
        None => tracker.get_all_days(),
    }
}

fn load_weekly_tracking(
    tracker: &Tracker,
    project_scope: Option<&str>,
) -> Result<Vec<crate::tracking::WeekStats>> {
    match project_scope {
        Some(scope) => tracker.get_by_week_filtered(Some(scope)),
        None => tracker.get_by_week(),
    }
}

fn load_monthly_tracking(
    tracker: &Tracker,
    project_scope: Option<&str>,
) -> Result<Vec<crate::tracking::MonthStats>> {
    match project_scope {
        Some(scope) => tracker.get_by_month_filtered(Some(scope)),
        None => tracker.get_by_month(),
    }
}

pub fn display_text(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    verbose: u8,
    project_scope: Option<&str>,
) -> Result<()> {
    // Default: summary view
    if !daily && !weekly && !monthly && !all {
        display_summary(tracker, verbose, project_scope)?;
        return Ok(());
    }

    if all || daily {
        display_daily(tracker, verbose, project_scope)?;
    }
    if all || weekly {
        display_weekly(tracker, verbose, project_scope)?;
    }
    if all || monthly {
        display_monthly(tracker, verbose, project_scope)?;
    }

    Ok(())
}

pub fn display_summary(tracker: &Tracker, verbose: u8, project_scope: Option<&str>) -> Result<()> {
    let cc_monthly =
        ccusage::fetch(Granularity::Monthly).context("Failed to fetch ccusage monthly data")?;
    let tracking_monthly = load_monthly_tracking(tracker, project_scope)
        .context("Failed to load monthly token savings from database")?;
    let periods = merge_monthly(cc_monthly, tracking_monthly);

    if periods.is_empty() {
        println!("No data available. Run some mycelium commands to start tracking.");
        return Ok(());
    }

    let totals = compute_totals(&periods);

    println!("Claude Code Economics");
    println!("════════════════════════════════════════════════════");
    println!();

    print_scope_note(project_scope);

    println!(
        "  Spent (ccusage):              {}",
        format_usd(totals.cc_cost)
    );
    println!("  Token breakdown:");
    println!(
        "    Input:                      {}",
        format_tokens(totals.cc_input_tokens as usize)
    );
    println!(
        "    Output:                     {}",
        format_tokens(totals.cc_output_tokens as usize)
    );
    println!(
        "    Cache writes:               {}",
        format_tokens(totals.cc_cache_create_tokens as usize)
    );
    println!(
        "    Cache reads:                {}",
        format_tokens(totals.cc_cache_read_tokens as usize)
    );
    println!();

    println!(
        "  Mycelium commands:             {}",
        totals.mycelium_commands
    );
    println!(
        "  Tokens saved:                 {}",
        format_tokens(totals.mycelium_saved_tokens)
    );
    println!();

    println!("  Estimated Savings:");
    println!("  ┌─────────────────────────────────────────────────┐");

    if let Some(weighted_savings) = totals.savings_weighted {
        let weighted_pct = if totals.cc_cost > 0.0 {
            (weighted_savings / totals.cc_cost) * 100.0
        } else {
            0.0
        };
        println!(
            "  │ Input token pricing:   {}  ({:.1}%)           │",
            format_usd(weighted_savings).trim_end(),
            weighted_pct
        );
        if let Some(input_cpt) = totals.weighted_input_cpt {
            println!(
                "  │ Derived input CPT:     {}               │",
                format_cpt(input_cpt)
            );
        }
    } else {
        println!("  │ Input token pricing:   —                         │");
    }

    println!("  └─────────────────────────────────────────────────┘");
    println!();

    println!("  How it works:");
    println!("  Mycelium compresses CLI outputs before they enter Claude's context.");
    println!("  Savings derived using API price ratios (out=5x, cache_w=1.25x, cache_r=0.1x).");
    println!();

    // Verbose mode: legacy metrics
    if verbose > 0 {
        println!("  Legacy metrics (reference only):");
        if let Some(active_savings) = totals.savings_active {
            let active_pct = if totals.cc_cost > 0.0 {
                (active_savings / totals.cc_cost) * 100.0
            } else {
                0.0
            };
            println!(
                "    Active (OVERESTIMATES):  {}  ({:.1}%)",
                format_usd(active_savings),
                active_pct
            );
        }
        if let Some(blended_savings) = totals.savings_blended {
            let blended_pct = if totals.cc_cost > 0.0 {
                (blended_savings / totals.cc_cost) * 100.0
            } else {
                0.0
            };
            println!(
                "    Blended (UNDERESTIMATES): {}  ({:.2}%)",
                format_usd(blended_savings),
                blended_pct
            );
        }
        println!("  Note: Saved tokens estimated via chars/4 heuristic, not exact tokenizer.");
        println!();
    }

    Ok(())
}

pub fn display_daily(tracker: &Tracker, verbose: u8, project_scope: Option<&str>) -> Result<()> {
    let cc_daily =
        ccusage::fetch(Granularity::Daily).context("Failed to fetch ccusage daily data")?;
    let tracking_daily = load_daily_tracking(tracker, project_scope)
        .context("Failed to load daily token savings from database")?;
    let periods = merge_daily(cc_daily, tracking_daily);

    println!("Daily Economics");
    println!("════════════════════════════════════════════════════");
    print_scope_note(project_scope);
    print_period_table(&periods, verbose);
    Ok(())
}

pub fn display_weekly(tracker: &Tracker, verbose: u8, project_scope: Option<&str>) -> Result<()> {
    let cc_weekly =
        ccusage::fetch(Granularity::Weekly).context("Failed to fetch ccusage weekly data")?;
    let tracking_weekly = load_weekly_tracking(tracker, project_scope)
        .context("Failed to load weekly token savings from database")?;
    let periods = merge_weekly(cc_weekly, tracking_weekly);

    println!("Weekly Economics");
    println!("════════════════════════════════════════════════════");
    print_scope_note(project_scope);
    print_period_table(&periods, verbose);
    Ok(())
}

pub fn display_monthly(tracker: &Tracker, verbose: u8, project_scope: Option<&str>) -> Result<()> {
    let cc_monthly =
        ccusage::fetch(Granularity::Monthly).context("Failed to fetch ccusage monthly data")?;
    let tracking_monthly = load_monthly_tracking(tracker, project_scope)
        .context("Failed to load monthly token savings from database")?;
    let periods = merge_monthly(cc_monthly, tracking_monthly);

    println!("Monthly Economics");
    println!("════════════════════════════════════════════════════");
    print_scope_note(project_scope);
    print_period_table(&periods, verbose);
    Ok(())
}

fn print_scope_note(project_scope: Option<&str>) {
    if let Some(scope) = project_scope {
        println!("  Scope: {}", scope);
        println!("  Note: Mycelium savings are project-scoped here; ccusage spend remains global.");
        println!();
    }
}

pub fn print_period_table(periods: &[PeriodEconomics], verbose: u8) {
    println!();

    if verbose > 0 {
        // Verbose: include legacy metrics
        println!(
            "{:<12} {:>10} {:>10} {:>10} {:>10} {:>12} {:>12}",
            "Period", "Spent", "Saved", "Savings", "Active$", "Blended$", "Mycelium Cmds"
        );
        println!(
            "{:-<12} {:-<10} {:-<10} {:-<10} {:-<10} {:-<12} {:-<12}",
            "", "", "", "", "", "", ""
        );

        for p in periods {
            let spent = p.cc_cost.map(format_usd).unwrap_or_else(|| "—".to_string());
            let saved = p
                .mycelium_saved_tokens
                .map(format_tokens)
                .unwrap_or_else(|| "—".to_string());
            let weighted = p
                .savings_weighted
                .map(format_usd)
                .unwrap_or_else(|| "—".to_string());
            let active = p
                .savings_active
                .map(format_usd)
                .unwrap_or_else(|| "—".to_string());
            let blended = p
                .savings_blended
                .map(format_usd)
                .unwrap_or_else(|| "—".to_string());
            let cmds = p
                .mycelium_commands
                .map(|c| c.to_string())
                .unwrap_or_else(|| "—".to_string());

            println!(
                "{:<12} {:>10} {:>10} {:>10} {:>10} {:>12} {:>12}",
                p.label, spent, saved, weighted, active, blended, cmds
            );
        }
    } else {
        // Default: single Savings column
        println!(
            "{:<12} {:>10} {:>10} {:>10} {:>12}",
            "Period", "Spent", "Saved", "Savings", "Mycelium Cmds"
        );
        println!(
            "{:-<12} {:-<10} {:-<10} {:-<10} {:-<12}",
            "", "", "", "", ""
        );

        for p in periods {
            let spent = p.cc_cost.map(format_usd).unwrap_or_else(|| "—".to_string());
            let saved = p
                .mycelium_saved_tokens
                .map(format_tokens)
                .unwrap_or_else(|| "—".to_string());
            let weighted = p
                .savings_weighted
                .map(format_usd)
                .unwrap_or_else(|| "—".to_string());
            let cmds = p
                .mycelium_commands
                .map(|c| c.to_string())
                .unwrap_or_else(|| "—".to_string());

            println!(
                "{:<12} {:>10} {:>10} {:>10} {:>12}",
                p.label, spent, saved, weighted, cmds
            );
        }
    }
    println!();
}
