//! Side-by-side token comparison: run a command raw and via mycelium.
use super::helpers::styled;
use crate::tracking::estimate_tokens;
use crate::utils::format_tokens;
use anyhow::{Context, Result};
use colored::Colorize;
use std::io::IsTerminal;

/// Run `--compare`: execute a command raw and via `mycelium`, compare token counts side-by-side.
pub(crate) fn run_compare(cmd_str: &str) -> Result<()> {
    let args: Vec<&str> = cmd_str.split_whitespace().collect();
    if args.is_empty() {
        anyhow::bail!("--compare requires a non-empty command string");
    }

    // Detect current mycelium binary so we can re-invoke it.
    let mycelium_bin =
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("mycelium"));

    // Run the command raw.
    let raw_output = std::process::Command::new(args[0])
        .args(&args[1..])
        .output()
        .with_context(|| format!("Failed to execute raw command: {}", args[0]))?;
    let raw_text = format!(
        "{}{}",
        String::from_utf8_lossy(&raw_output.stdout),
        String::from_utf8_lossy(&raw_output.stderr)
    );

    // Run the same command through mycelium.
    let mycelium_output = std::process::Command::new(&mycelium_bin)
        .args(&args)
        .output()
        .with_context(|| format!("Failed to execute mycelium {}", args[0]))?;
    let mycelium_text = format!(
        "{}{}",
        String::from_utf8_lossy(&mycelium_output.stdout),
        String::from_utf8_lossy(&mycelium_output.stderr)
    );

    let raw_tokens = estimate_tokens(&raw_text);
    let mycelium_tokens = estimate_tokens(&mycelium_text);

    let (saved, savings_pct) = calculate_savings(raw_tokens, mycelium_tokens);

    let bar = compare_bar(savings_pct, 30);

    println!("{}", styled("Mycelium Token Comparison", true));
    println!("{}", "═".repeat(60));
    println!("  Command : {}", cmd_str);
    println!();
    println!("  Raw tokens : {:>8}", format_tokens(raw_tokens));
    println!("  Mycelium tokens : {:>8}", format_tokens(mycelium_tokens));
    println!(
        "  Saved      : {:>8}  ({:.1}%)",
        format_tokens(saved),
        savings_pct
    );
    println!();
    println!("  Savings  {}", bar);
    println!();

    if savings_pct < 1.0 && raw_tokens > 0 {
        println!(
            "  {} Mycelium may not have a dedicated filter for '{}'. Try: mycelium proxy {}",
            "hint:".yellow().bold(),
            args[0],
            cmd_str
        );
    }

    Ok(())
}

/// Calculate token savings between raw and mycelium-filtered output.
///
/// Returns `(tokens_saved, savings_percentage)`. Savings percentage is clamped
/// to `[0.0, 100.0]` — if mycelium output is larger than raw, savings are 0.
pub fn calculate_savings(raw_tokens: usize, mycelium_tokens: usize) -> (usize, f64) {
    if raw_tokens > 0 && raw_tokens >= mycelium_tokens {
        let saved = raw_tokens - mycelium_tokens;
        let pct = (saved as f64 / raw_tokens as f64) * 100.0;
        (saved, pct)
    } else {
        (0, 0.0)
    }
}

/// Build a colored savings bar for the compare view (TTY-aware).
pub(crate) fn compare_bar(pct: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let pct_clamped = pct.clamp(0.0, 100.0);
    let filled = ((pct_clamped / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let bar_str = format!("{}{}", "█".repeat(filled), "░".repeat(width - filled));
    let pct_label = format!(" {:.1}%", pct_clamped);
    let bar_with_label = format!("{}{}", bar_str, pct_label);
    if std::io::stdout().is_terminal() {
        if pct_clamped >= 60.0 {
            bar_with_label.green().to_string()
        } else if pct_clamped >= 30.0 {
            bar_with_label.yellow().to_string()
        } else {
            bar_with_label.red().to_string()
        }
    } else {
        bar_with_label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::estimate_tokens;

    // ── calculate_savings tests ──────────────────────────────────────

    #[test]
    fn test_savings_normal_case() {
        // 1000 raw tokens, 300 mycelium tokens → 70% savings
        let (saved, pct) = calculate_savings(1000, 300);
        assert_eq!(saved, 700);
        assert!((pct - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_no_regression() {
        // mycelium output larger than raw → 0% savings, not negative
        let (saved, pct) = calculate_savings(100, 150);
        assert_eq!(saved, 0);
        assert!((pct - 0.0).abs() < f64::EPSILON);

        // equal sizes → 0 saved, 0%
        let (saved, pct) = calculate_savings(100, 100);
        assert_eq!(saved, 0);
        assert!((pct - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_empty_raw() {
        // Both empty → 0% savings, no panic
        let (saved, pct) = calculate_savings(0, 0);
        assert_eq!(saved, 0);
        assert!((pct - 0.0).abs() < f64::EPSILON);

        // Raw empty but mycelium non-empty → 0%
        let (saved, pct) = calculate_savings(0, 50);
        assert_eq!(saved, 0);
        assert!((pct - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_complete_reduction() {
        // mycelium returns nothing → 100% savings
        let (saved, pct) = calculate_savings(500, 0);
        assert_eq!(saved, 500);
        assert!((pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_calculation_precision() {
        // 1/3 savings: 999 raw, 666 mycelium → 33.33..%
        let (saved, pct) = calculate_savings(999, 666);
        assert_eq!(saved, 333);
        assert!((pct - 33.333333333333336).abs() < 1e-10);

        // Very large values — no overflow
        let (saved, pct) = calculate_savings(usize::MAX / 2, usize::MAX / 4);
        assert_eq!(saved, usize::MAX / 2 - usize::MAX / 4);
        assert!(pct > 49.0 && pct < 51.0);

        // Single token
        let (saved, pct) = calculate_savings(1, 0);
        assert_eq!(saved, 1);
        assert!((pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_savings_with_estimate_tokens() {
        // Integration: use estimate_tokens to verify end-to-end
        let raw = "a]".repeat(200); // 400 chars → 100 tokens
        let filtered = "a]".repeat(60); // 120 chars → 30 tokens
        let raw_tokens = estimate_tokens(&raw);
        let filtered_tokens = estimate_tokens(&filtered);
        let (saved, pct) = calculate_savings(raw_tokens, filtered_tokens);
        assert_eq!(saved, raw_tokens - filtered_tokens);
        assert!((pct - 70.0).abs() < f64::EPSILON);
    }

    // ── compare_bar tests (existing + retained) ─────────────────────

    #[test]
    fn test_compare_bar_full_savings() {
        let result = compare_bar(100.0, 10);
        // 100% savings → all filled blocks
        let filled = result.matches('█').count();
        assert_eq!(filled, 10);
        assert!(result.contains("100.0%"));
    }

    #[test]
    fn test_compare_bar_zero_savings() {
        let result = compare_bar(0.0, 10);
        let filled = result.matches('█').count();
        assert_eq!(filled, 0);
        assert!(result.contains("0.0%"));
    }

    #[test]
    fn test_compare_bar_partial_savings() {
        let result = compare_bar(70.0, 10);
        let filled = result.matches('█').count();
        // 70% of 10 = 7 filled
        assert_eq!(filled, 7);
        assert!(result.contains("70.0%"));
    }

    #[test]
    fn test_compare_bar_zero_width() {
        let result = compare_bar(90.0, 0);
        assert_eq!(result, "");
    }
}
