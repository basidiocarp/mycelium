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

    let (saved, savings_pct) = if raw_tokens > 0 && raw_tokens >= mycelium_tokens {
        let s = raw_tokens - mycelium_tokens;
        let pct = (s as f64 / raw_tokens as f64) * 100.0;
        (s, pct)
    } else {
        (0, 0.0)
    };

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
