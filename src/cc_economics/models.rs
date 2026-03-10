//! Types and constants for Claude Code economics analysis.

use serde::Serialize;

use crate::ccusage;
use crate::tracking::{DayStats, MonthStats, WeekStats};

// ── Constants ──

// API pricing ratios (verified Feb 2026, consistent across Claude models <=200K context)
// Source: https://docs.anthropic.com/en/docs/about-claude/models
pub const WEIGHT_OUTPUT: f64 = 5.0; // Output = 5x input
pub const WEIGHT_CACHE_CREATE: f64 = 1.25; // Cache write = 1.25x input
pub const WEIGHT_CACHE_READ: f64 = 0.1; // Cache read = 0.1x input

// ── Types ──

/// Combined spending and savings metrics for a single time period.
#[derive(Debug, Serialize)]
pub struct PeriodEconomics {
    pub label: String,
    // ccusage metrics (Option for graceful degradation)
    pub cc_cost: Option<f64>,
    pub cc_total_tokens: Option<u64>,
    pub cc_active_tokens: Option<u64>, // input + output only (excluding cache)
    // Per-type token breakdown
    pub cc_input_tokens: Option<u64>,
    pub cc_output_tokens: Option<u64>,
    pub cc_cache_create_tokens: Option<u64>,
    pub cc_cache_read_tokens: Option<u64>,
    // tracking metrics
    pub mycelium_commands: Option<usize>,
    pub mycelium_saved_tokens: Option<usize>,
    pub mycelium_savings_pct: Option<f64>,
    // Primary metric (weighted input CPT)
    pub weighted_input_cpt: Option<f64>, // Derived input CPT using API ratios
    pub savings_weighted: Option<f64>,   // saved * weighted_input_cpt (PRIMARY)
    // Legacy metrics (verbose mode only)
    pub blended_cpt: Option<f64>, // cost / total_tokens (diluted by cache)
    pub active_cpt: Option<f64>,  // cost / active_tokens (OVERESTIMATES)
    pub savings_blended: Option<f64>, // saved * blended_cpt (UNDERESTIMATES)
    pub savings_active: Option<f64>, // saved * active_cpt (OVERESTIMATES)
}

impl PeriodEconomics {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            cc_cost: None,
            cc_total_tokens: None,
            cc_active_tokens: None,
            cc_input_tokens: None,
            cc_output_tokens: None,
            cc_cache_create_tokens: None,
            cc_cache_read_tokens: None,
            mycelium_commands: None,
            mycelium_saved_tokens: None,
            mycelium_savings_pct: None,
            weighted_input_cpt: None,
            savings_weighted: None,
            blended_cpt: None,
            active_cpt: None,
            savings_blended: None,
            savings_active: None,
        }
    }

    pub fn set_ccusage(&mut self, metrics: &ccusage::CcusageMetrics) {
        self.cc_cost = Some(metrics.total_cost);
        self.cc_total_tokens = Some(metrics.total_tokens);

        // Store per-type tokens
        self.cc_input_tokens = Some(metrics.input_tokens);
        self.cc_output_tokens = Some(metrics.output_tokens);
        self.cc_cache_create_tokens = Some(metrics.cache_creation_tokens);
        self.cc_cache_read_tokens = Some(metrics.cache_read_tokens);

        // Active tokens (legacy)
        let active = metrics.input_tokens + metrics.output_tokens;
        self.cc_active_tokens = Some(active);
    }

    pub fn set_tracking_from_day(&mut self, stats: &DayStats) {
        self.mycelium_commands = Some(stats.commands);
        self.mycelium_saved_tokens = Some(stats.saved_tokens);
        self.mycelium_savings_pct = Some(stats.savings_pct);
    }

    pub fn set_tracking_from_week(&mut self, stats: &WeekStats) {
        self.mycelium_commands = Some(stats.commands);
        self.mycelium_saved_tokens = Some(stats.saved_tokens);
        self.mycelium_savings_pct = Some(stats.savings_pct);
    }

    pub fn set_tracking_from_month(&mut self, stats: &MonthStats) {
        self.mycelium_commands = Some(stats.commands);
        self.mycelium_saved_tokens = Some(stats.saved_tokens);
        self.mycelium_savings_pct = Some(if stats.input_tokens + stats.output_tokens > 0 {
            stats.saved_tokens as f64
                / (stats.saved_tokens + stats.input_tokens + stats.output_tokens) as f64
                * 100.0
        } else {
            0.0
        });
    }

    pub fn compute_weighted_metrics(&mut self) {
        // Weighted input CPT derivation using API price ratios
        if let (Some(cost), Some(saved)) = (self.cc_cost, self.mycelium_saved_tokens)
            && let (Some(input), Some(output), Some(cache_create), Some(cache_read)) = (
                self.cc_input_tokens,
                self.cc_output_tokens,
                self.cc_cache_create_tokens,
                self.cc_cache_read_tokens,
            ) {
                // Weighted units = input + 5*output + 1.25*cache_create + 0.1*cache_read
                let weighted_units = input as f64
                    + WEIGHT_OUTPUT * output as f64
                    + WEIGHT_CACHE_CREATE * cache_create as f64
                    + WEIGHT_CACHE_READ * cache_read as f64;

                if weighted_units > 0.0 {
                    let input_cpt = cost / weighted_units;
                    let savings = saved as f64 * input_cpt;

                    self.weighted_input_cpt = Some(input_cpt);
                    self.savings_weighted = Some(savings);
                }
            }
    }

    pub fn compute_dual_metrics(&mut self) {
        if let (Some(cost), Some(saved)) = (self.cc_cost, self.mycelium_saved_tokens) {
            // Blended CPT (cost / total_tokens including cache)
            if let Some(total) = self.cc_total_tokens
                && total > 0 {
                    self.blended_cpt = Some(cost / total as f64);
                    self.savings_blended = Some(saved as f64 * (cost / total as f64));
                }

            // Active CPT (cost / active_tokens = input+output only)
            if let Some(active) = self.cc_active_tokens
                && active > 0 {
                    self.active_cpt = Some(cost / active as f64);
                    self.savings_active = Some(saved as f64 * (cost / active as f64));
                }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Totals {
    pub cc_cost: f64,
    pub cc_total_tokens: u64,
    pub cc_active_tokens: u64,
    pub cc_input_tokens: u64,
    pub cc_output_tokens: u64,
    pub cc_cache_create_tokens: u64,
    pub cc_cache_read_tokens: u64,
    pub mycelium_commands: usize,
    pub mycelium_saved_tokens: usize,
    pub mycelium_avg_savings_pct: f64,
    pub weighted_input_cpt: Option<f64>,
    pub savings_weighted: Option<f64>,
    pub blended_cpt: Option<f64>,
    pub active_cpt: Option<f64>,
    pub savings_blended: Option<f64>,
    pub savings_active: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_period_economics_new() {
        let p = PeriodEconomics::new("2026-01");
        assert_eq!(p.label, "2026-01");
        assert!(p.cc_cost.is_none());
        assert!(p.mycelium_commands.is_none());
    }

    #[test]
    fn test_compute_dual_metrics_with_data() {
        let mut p = PeriodEconomics {
            label: "2026-01".to_string(),
            cc_cost: Some(100.0),
            cc_total_tokens: Some(1_000_000),
            cc_active_tokens: Some(10_000),
            mycelium_saved_tokens: Some(5_000),
            ..PeriodEconomics::new("2026-01")
        };

        p.compute_dual_metrics();

        assert!(p.blended_cpt.is_some());
        assert_eq!(p.blended_cpt.unwrap(), 100.0 / 1_000_000.0);

        assert!(p.active_cpt.is_some());
        assert_eq!(p.active_cpt.unwrap(), 100.0 / 10_000.0);

        assert!(p.savings_blended.is_some());
        assert!(p.savings_active.is_some());
    }

    #[test]
    fn test_compute_dual_metrics_zero_tokens() {
        let mut p = PeriodEconomics {
            label: "2026-01".to_string(),
            cc_cost: Some(100.0),
            cc_total_tokens: Some(0),
            cc_active_tokens: Some(0),
            mycelium_saved_tokens: Some(5_000),
            ..PeriodEconomics::new("2026-01")
        };

        p.compute_dual_metrics();

        assert!(p.blended_cpt.is_none());
        assert!(p.active_cpt.is_none());
        assert!(p.savings_blended.is_none());
        assert!(p.savings_active.is_none());
    }

    #[test]
    fn test_compute_dual_metrics_no_ccusage_data() {
        let mut p = PeriodEconomics {
            label: "2026-01".to_string(),
            mycelium_saved_tokens: Some(5_000),
            ..PeriodEconomics::new("2026-01")
        };

        p.compute_dual_metrics();

        assert!(p.blended_cpt.is_none());
        assert!(p.active_cpt.is_none());
    }

    #[test]
    fn test_compute_weighted_input_cpt() {
        let mut p = PeriodEconomics::new("2026-01");
        p.cc_cost = Some(100.0);
        p.cc_input_tokens = Some(1000);
        p.cc_output_tokens = Some(500);
        p.cc_cache_create_tokens = Some(200);
        p.cc_cache_read_tokens = Some(5000);
        p.mycelium_saved_tokens = Some(10_000);

        p.compute_weighted_metrics();

        // weighted_units = 1000 + 5*500 + 1.25*200 + 0.1*5000 = 1000 + 2500 + 250 + 500 = 4250
        // input_cpt = 100 / 4250 = 0.0235294...
        // savings = 10000 * 0.0235294... = 235.29...

        assert!(p.weighted_input_cpt.is_some());
        let cpt = p.weighted_input_cpt.unwrap();
        assert!((cpt - (100.0 / 4250.0)).abs() < 1e-6);

        assert!(p.savings_weighted.is_some());
        let savings = p.savings_weighted.unwrap();
        assert!((savings - 235.294).abs() < 0.01);
    }

    #[test]
    fn test_compute_weighted_metrics_zero_tokens() {
        let mut p = PeriodEconomics::new("2026-01");
        p.cc_cost = Some(100.0);
        p.cc_input_tokens = Some(0);
        p.cc_output_tokens = Some(0);
        p.cc_cache_create_tokens = Some(0);
        p.cc_cache_read_tokens = Some(0);
        p.mycelium_saved_tokens = Some(5000);

        p.compute_weighted_metrics();

        assert!(p.weighted_input_cpt.is_none());
        assert!(p.savings_weighted.is_none());
    }

    #[test]
    fn test_compute_weighted_metrics_no_cache() {
        let mut p = PeriodEconomics::new("2026-01");
        p.cc_cost = Some(60.0);
        p.cc_input_tokens = Some(1000);
        p.cc_output_tokens = Some(1000);
        p.cc_cache_create_tokens = Some(0);
        p.cc_cache_read_tokens = Some(0);
        p.mycelium_saved_tokens = Some(3000);

        p.compute_weighted_metrics();

        // weighted_units = 1000 + 5*1000 = 6000
        // input_cpt = 60 / 6000 = 0.01
        // savings = 3000 * 0.01 = 30

        assert!(p.weighted_input_cpt.is_some());
        let cpt = p.weighted_input_cpt.unwrap();
        assert!((cpt - 0.01).abs() < 1e-6);

        assert!(p.savings_weighted.is_some());
        let savings = p.savings_weighted.unwrap();
        assert!((savings - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_set_ccusage_stores_per_type_tokens() {
        let mut p = PeriodEconomics::new("2026-01");
        let metrics = ccusage::CcusageMetrics {
            input_tokens: 1000,
            output_tokens: 500,
            cache_creation_tokens: 200,
            cache_read_tokens: 3000,
            total_tokens: 4700,
            total_cost: 50.0,
        };

        p.set_ccusage(&metrics);

        assert_eq!(p.cc_input_tokens, Some(1000));
        assert_eq!(p.cc_output_tokens, Some(500));
        assert_eq!(p.cc_cache_create_tokens, Some(200));
        assert_eq!(p.cc_cache_read_tokens, Some(3000));
        assert_eq!(p.cc_total_tokens, Some(4700));
        assert_eq!(p.cc_cost, Some(50.0));
    }
}
