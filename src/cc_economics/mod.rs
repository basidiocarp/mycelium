//! Claude Code Economics: Spending vs Savings Analysis
//!
//! Combines ccusage (tokens spent) with mycelium tracking (tokens saved) to provide
//! dual-metric economic impact reporting with blended and active cost-per-token.

mod display;
mod export;
pub mod merge;
pub mod models;

use anyhow::{Context, Result};

use crate::tracking::Tracker;

use display::display_text;
use export::{export_csv, export_json};

/// Display or export Claude Code economics (spending vs savings) in text, JSON, or CSV.
pub fn run(
    daily: bool,
    weekly: bool,
    monthly: bool,
    all: bool,
    format: &str,
    verbose: u8,
) -> Result<()> {
    let tracker = Tracker::new().context("Failed to initialize tracking database")?;

    match format {
        "json" => export_json(&tracker, daily, weekly, monthly, all),
        "csv" => export_csv(&tracker, daily, weekly, monthly, all),
        _ => display_text(&tracker, daily, weekly, monthly, all, verbose),
    }
}
