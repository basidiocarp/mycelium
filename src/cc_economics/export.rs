//! JSON and CSV export for Claude Code economics data.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::ccusage::{self, Granularity};
use crate::tracking::Tracker;

use super::merge::{compute_totals, merge_daily, merge_monthly, merge_weekly};
use super::models::{PeriodEconomics, Totals};

pub fn export_json(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
) -> Result<()> {
    #[derive(Serialize)]
    struct Export {
        daily: Option<Vec<PeriodEconomics>>,
        weekly: Option<Vec<PeriodEconomics>>,
        monthly: Option<Vec<PeriodEconomics>>,
        totals: Option<Totals>,
    }

    let mut export = Export {
        daily: None,
        weekly: None,
        monthly: None,
        totals: None,
    };

    if all || daily {
        let cc = ccusage::fetch(Granularity::Daily)
            .context("Failed to fetch ccusage daily data for JSON export")?;
        let tracking = tracker
            .get_all_days()
            .context("Failed to load daily token savings for JSON export")?;
        export.daily = Some(merge_daily(cc, tracking));
    }

    if all || weekly {
        let cc = ccusage::fetch(Granularity::Weekly)
            .context("Failed to fetch ccusage weekly data for export")?;
        let tracking = tracker
            .get_by_week()
            .context("Failed to load weekly token savings for export")?;
        export.weekly = Some(merge_weekly(cc, tracking));
    }

    if all || monthly {
        let cc = ccusage::fetch(Granularity::Monthly)
            .context("Failed to fetch ccusage monthly data for export")?;
        let tracking = tracker
            .get_by_month()
            .context("Failed to load monthly token savings for export")?;
        let periods = merge_monthly(cc, tracking);
        export.totals = Some(compute_totals(&periods));
        export.monthly = Some(periods);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&export)
            .context("Failed to serialize economics data to JSON")?
    );
    Ok(())
}

pub fn export_csv(
    tracker: &Tracker,
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
) -> Result<()> {
    // Header (new columns: input_tokens, output_tokens, cache_create, cache_read, weighted_savings)
    println!("period,spent,input_tokens,output_tokens,cache_create,cache_read,active_tokens,total_tokens,saved_tokens,weighted_savings,active_savings,blended_savings,mycelium_commands");

    if all || daily {
        let cc = ccusage::fetch(Granularity::Daily)
            .context("Failed to fetch ccusage daily data for JSON export")?;
        let tracking = tracker
            .get_all_days()
            .context("Failed to load daily token savings for JSON export")?;
        let periods = merge_daily(cc, tracking);
        for p in periods {
            print_csv_row(&p);
        }
    }

    if all || weekly {
        let cc = ccusage::fetch(Granularity::Weekly)
            .context("Failed to fetch ccusage weekly data for export")?;
        let tracking = tracker
            .get_by_week()
            .context("Failed to load weekly token savings for export")?;
        let periods = merge_weekly(cc, tracking);
        for p in periods {
            print_csv_row(&p);
        }
    }

    if all || monthly {
        let cc = ccusage::fetch(Granularity::Monthly)
            .context("Failed to fetch ccusage monthly data for export")?;
        let tracking = tracker
            .get_by_month()
            .context("Failed to load monthly token savings for export")?;
        let periods = merge_monthly(cc, tracking);
        for p in periods {
            print_csv_row(&p);
        }
    }

    Ok(())
}

fn print_csv_row(p: &PeriodEconomics) {
    let spent = p.cc_cost.map(|c| format!("{:.4}", c)).unwrap_or_default();
    let input_tokens = p.cc_input_tokens.map(|t| t.to_string()).unwrap_or_default();
    let output_tokens = p
        .cc_output_tokens
        .map(|t| t.to_string())
        .unwrap_or_default();
    let cache_create = p
        .cc_cache_create_tokens
        .map(|t| t.to_string())
        .unwrap_or_default();
    let cache_read = p
        .cc_cache_read_tokens
        .map(|t| t.to_string())
        .unwrap_or_default();
    let active_tokens = p
        .cc_active_tokens
        .map(|t| t.to_string())
        .unwrap_or_default();
    let total_tokens = p.cc_total_tokens.map(|t| t.to_string()).unwrap_or_default();
    let saved_tokens = p
        .mycelium_saved_tokens
        .map(|t| t.to_string())
        .unwrap_or_default();
    let weighted_savings = p
        .savings_weighted
        .map(|s| format!("{:.4}", s))
        .unwrap_or_default();
    let active_savings = p
        .savings_active
        .map(|s| format!("{:.4}", s))
        .unwrap_or_default();
    let blended_savings = p
        .savings_blended
        .map(|s| format!("{:.4}", s))
        .unwrap_or_default();
    let cmds = p
        .mycelium_commands
        .map(|c| c.to_string())
        .unwrap_or_default();

    println!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{}",
        p.label,
        spent,
        input_tokens,
        output_tokens,
        cache_create,
        cache_read,
        active_tokens,
        total_tokens,
        saved_tokens,
        weighted_savings,
        active_savings,
        blended_savings,
        cmds
    );
}
