//! Merge logic for combining ccusage and mycelium tracking data.

use std::collections::HashMap;

use jiff::civil::Date;

use crate::ccusage::CcusagePeriod;
use crate::tracking::{DayStats, MonthStats, WeekStats};

use super::models::{
    PeriodEconomics, Totals, WEIGHT_CACHE_CREATE, WEIGHT_CACHE_READ, WEIGHT_OUTPUT,
};

pub fn merge_daily(
    cc: Option<Vec<CcusagePeriod>>,
    tracking: Vec<DayStats>,
) -> Vec<PeriodEconomics> {
    let mut map: HashMap<String, PeriodEconomics> = HashMap::new();

    // Insert ccusage data
    if let Some(cc_data) = cc {
        for entry in cc_data {
            let CcusagePeriod { key, metrics } = entry;
            map.entry(key)
                .or_insert_with_key(|k| PeriodEconomics::new(k))
                .set_ccusage(&metrics);
        }
    }

    // Merge tracking data
    for entry in tracking {
        map.entry(entry.date.clone())
            .or_insert_with_key(|k| PeriodEconomics::new(k))
            .set_tracking_from_day(&entry);
    }

    // Compute dual metrics and sort
    let mut result: Vec<_> = map.into_values().collect();
    for period in &mut result {
        period.compute_weighted_metrics();
        period.compute_dual_metrics();
    }
    result.sort_by(|a, b| a.label.cmp(&b.label));
    result
}

pub fn merge_weekly(
    cc: Option<Vec<CcusagePeriod>>,
    tracking: Vec<WeekStats>,
) -> Vec<PeriodEconomics> {
    let mut map: HashMap<String, PeriodEconomics> = HashMap::new();

    // Insert ccusage data (key = ISO Monday "2026-01-20")
    if let Some(cc_data) = cc {
        for entry in cc_data {
            let CcusagePeriod { key, metrics } = entry;
            map.entry(key)
                .or_insert_with_key(|k| PeriodEconomics::new(k))
                .set_ccusage(&metrics);
        }
    }

    // Merge tracking data (week_start = legacy Saturday "2026-01-18")
    // Convert Saturday to Monday for alignment
    for entry in tracking {
        let monday_key = match convert_saturday_to_monday(&entry.week_start) {
            Some(m) => m,
            None => {
                eprintln!("[!] Invalid week_start format: {}", entry.week_start);
                continue;
            }
        };

        map.entry(monday_key)
            .or_insert_with_key(|key| PeriodEconomics::new(key))
            .set_tracking_from_week(&entry);
    }

    let mut result: Vec<_> = map.into_values().collect();
    for period in &mut result {
        period.compute_weighted_metrics();
        period.compute_dual_metrics();
    }
    result.sort_by(|a, b| a.label.cmp(&b.label));
    result
}

pub fn merge_monthly(
    cc: Option<Vec<CcusagePeriod>>,
    tracking: Vec<MonthStats>,
) -> Vec<PeriodEconomics> {
    let mut map: HashMap<String, PeriodEconomics> = HashMap::new();

    // Insert ccusage data
    if let Some(cc_data) = cc {
        for entry in cc_data {
            let CcusagePeriod { key, metrics } = entry;
            map.entry(key)
                .or_insert_with_key(|k| PeriodEconomics::new(k))
                .set_ccusage(&metrics);
        }
    }

    // Merge tracking data
    for entry in tracking {
        map.entry(entry.month.clone())
            .or_insert_with_key(|k| PeriodEconomics::new(k))
            .set_tracking_from_month(&entry);
    }

    let mut result: Vec<_> = map.into_values().collect();
    for period in &mut result {
        period.compute_weighted_metrics();
        period.compute_dual_metrics();
    }
    result.sort_by(|a, b| a.label.cmp(&b.label));
    result
}

/// Convert Saturday week_start (legacy tracking) to ISO Monday
/// Example: "2026-01-18" (Sat) -> "2026-01-20" (Mon)
pub fn convert_saturday_to_monday(saturday: &str) -> Option<String> {
    let sat_date: Date = saturday.parse().ok()?;

    // tracking uses Saturday as week start, ISO uses Monday
    // Saturday + 2 days = Monday
    let monday = sat_date.checked_add(jiff::Span::new().days(2)).ok()?;

    Some(monday.to_string())
}

pub fn compute_totals(periods: &[PeriodEconomics]) -> Totals {
    let mut totals = Totals {
        cc_cost: 0.0,
        cc_total_tokens: 0,
        cc_active_tokens: 0,
        cc_input_tokens: 0,
        cc_output_tokens: 0,
        cc_cache_create_tokens: 0,
        cc_cache_read_tokens: 0,
        mycelium_commands: 0,
        mycelium_saved_tokens: 0,
        mycelium_avg_savings_pct: 0.0,
        weighted_input_cpt: None,
        savings_weighted: None,
        blended_cpt: None,
        active_cpt: None,
        savings_blended: None,
        savings_active: None,
    };

    let mut pct_sum = 0.0;
    let mut pct_count = 0;

    for p in periods {
        if let Some(cost) = p.cc_cost {
            totals.cc_cost += cost;
        }
        if let Some(total) = p.cc_total_tokens {
            totals.cc_total_tokens += total;
        }
        if let Some(active) = p.cc_active_tokens {
            totals.cc_active_tokens += active;
        }
        if let Some(input) = p.cc_input_tokens {
            totals.cc_input_tokens += input;
        }
        if let Some(output) = p.cc_output_tokens {
            totals.cc_output_tokens += output;
        }
        if let Some(cache_create) = p.cc_cache_create_tokens {
            totals.cc_cache_create_tokens += cache_create;
        }
        if let Some(cache_read) = p.cc_cache_read_tokens {
            totals.cc_cache_read_tokens += cache_read;
        }
        if let Some(cmds) = p.mycelium_commands {
            totals.mycelium_commands += cmds;
        }
        if let Some(saved) = p.mycelium_saved_tokens {
            totals.mycelium_saved_tokens += saved;
        }
        if let Some(pct) = p.mycelium_savings_pct {
            pct_sum += pct;
            pct_count += 1;
        }
    }

    if pct_count > 0 {
        totals.mycelium_avg_savings_pct = pct_sum / pct_count as f64;
    }

    // Compute global weighted metrics
    let weighted_units = totals.cc_input_tokens as f64
        + WEIGHT_OUTPUT * totals.cc_output_tokens as f64
        + WEIGHT_CACHE_CREATE * totals.cc_cache_create_tokens as f64
        + WEIGHT_CACHE_READ * totals.cc_cache_read_tokens as f64;

    if weighted_units > 0.0 {
        let input_cpt = totals.cc_cost / weighted_units;
        totals.weighted_input_cpt = Some(input_cpt);
        totals.savings_weighted = Some(totals.mycelium_saved_tokens as f64 * input_cpt);
    }

    // Compute global dual metrics (legacy)
    if totals.cc_total_tokens > 0 {
        let blended = totals.cc_cost / totals.cc_total_tokens as f64;
        totals.blended_cpt = Some(blended);
        totals.savings_blended = Some(totals.mycelium_saved_tokens as f64 * blended);
    }
    if totals.cc_active_tokens > 0 {
        let active = totals.cc_cost / totals.cc_active_tokens as f64;
        totals.active_cpt = Some(active);
        totals.savings_active = Some(totals.mycelium_saved_tokens as f64 * active);
    }

    totals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccusage::{self, CcusagePeriod};
    use crate::tracking::MonthStats;

    #[test]
    fn test_convert_saturday_to_monday() {
        // Saturday Jan 18 -> Monday Jan 20
        assert_eq!(
            convert_saturday_to_monday("2026-01-18"),
            Some("2026-01-20".to_string())
        );

        // Invalid format
        assert_eq!(convert_saturday_to_monday("invalid"), None);
    }

    #[test]
    fn test_merge_monthly_both_present() {
        let cc = vec![CcusagePeriod {
            key: "2026-01".to_string(),
            metrics: ccusage::CcusageMetrics {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_tokens: 100,
                cache_read_tokens: 200,
                total_tokens: 1800,
                total_cost: 12.34,
            },
        }];

        let out = vec![MonthStats {
            month: "2026-01".to_string(),
            commands: 10,
            input_tokens: 800,
            output_tokens: 400,
            saved_tokens: 5000,
            savings_pct: 50.0,
            total_time_ms: 0,
            avg_time_ms: 0,
        }];

        let merged = merge_monthly(Some(cc), out);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].label, "2026-01");
        assert_eq!(merged[0].cc_cost, Some(12.34));
        assert_eq!(merged[0].mycelium_commands, Some(10));
    }

    #[test]
    fn test_merge_monthly_only_ccusage() {
        let cc = vec![CcusagePeriod {
            key: "2026-01".to_string(),
            metrics: ccusage::CcusageMetrics {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_tokens: 100,
                cache_read_tokens: 200,
                total_tokens: 1800,
                total_cost: 12.34,
            },
        }];

        let merged = merge_monthly(Some(cc), vec![]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].cc_cost, Some(12.34));
        assert!(merged[0].mycelium_commands.is_none());
    }

    #[test]
    fn test_merge_monthly_only_mycelium() {
        let out = vec![MonthStats {
            month: "2026-01".to_string(),
            commands: 10,
            input_tokens: 800,
            output_tokens: 400,
            saved_tokens: 5000,
            savings_pct: 50.0,
            total_time_ms: 0,
            avg_time_ms: 0,
        }];

        let merged = merge_monthly(None, out);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].cc_cost.is_none());
        assert_eq!(merged[0].mycelium_commands, Some(10));
    }

    #[test]
    fn test_merge_monthly_sorted() {
        let out = vec![
            MonthStats {
                month: "2026-03".to_string(),
                commands: 5,
                input_tokens: 100,
                output_tokens: 50,
                saved_tokens: 1000,
                savings_pct: 40.0,
                total_time_ms: 0,
                avg_time_ms: 0,
            },
            MonthStats {
                month: "2026-01".to_string(),
                commands: 10,
                input_tokens: 200,
                output_tokens: 100,
                saved_tokens: 2000,
                savings_pct: 60.0,
                total_time_ms: 0,
                avg_time_ms: 0,
            },
        ];

        let merged = merge_monthly(None, out);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].label, "2026-01");
        assert_eq!(merged[1].label, "2026-03");
    }

    #[test]
    fn test_compute_totals() {
        let periods = vec![
            PeriodEconomics {
                label: "2026-01".to_string(),
                cc_cost: Some(100.0),
                cc_total_tokens: Some(1_000_000),
                cc_active_tokens: Some(10_000),
                cc_input_tokens: Some(5000),
                cc_output_tokens: Some(5000),
                cc_cache_create_tokens: Some(100),
                cc_cache_read_tokens: Some(984_900),
                mycelium_commands: Some(5),
                mycelium_saved_tokens: Some(2000),
                mycelium_savings_pct: Some(50.0),
                weighted_input_cpt: None,
                savings_weighted: None,
                blended_cpt: None,
                active_cpt: None,
                savings_blended: None,
                savings_active: None,
            },
            PeriodEconomics {
                label: "2026-02".to_string(),
                cc_cost: Some(200.0),
                cc_total_tokens: Some(2_000_000),
                cc_active_tokens: Some(20_000),
                cc_input_tokens: Some(10_000),
                cc_output_tokens: Some(10_000),
                cc_cache_create_tokens: Some(200),
                cc_cache_read_tokens: Some(1_979_800),
                mycelium_commands: Some(10),
                mycelium_saved_tokens: Some(3000),
                mycelium_savings_pct: Some(60.0),
                weighted_input_cpt: None,
                savings_weighted: None,
                blended_cpt: None,
                active_cpt: None,
                savings_blended: None,
                savings_active: None,
            },
        ];

        let totals = compute_totals(&periods);
        assert_eq!(totals.cc_cost, 300.0);
        assert_eq!(totals.cc_total_tokens, 3_000_000);
        assert_eq!(totals.cc_active_tokens, 30_000);
        assert_eq!(totals.cc_input_tokens, 15_000);
        assert_eq!(totals.cc_output_tokens, 15_000);
        assert_eq!(totals.mycelium_commands, 15);
        assert_eq!(totals.mycelium_saved_tokens, 5000);
        assert_eq!(totals.mycelium_avg_savings_pct, 55.0);

        assert!(totals.weighted_input_cpt.is_some());
        assert!(totals.savings_weighted.is_some());
        assert!(totals.blended_cpt.is_some());
        assert!(totals.active_cpt.is_some());
    }
}
