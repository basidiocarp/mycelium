//! Hyphae integration — optional chunked storage for large command outputs.

use spore::{Tool, discover};
use std::sync::OnceLock;

/// Cached string representation of the hyphae binary path.
static HYPHAE_BINARY_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Check if the Hyphae binary is available in PATH.
/// Result is cached by spore for the lifetime of the process.
pub fn is_available() -> bool {
    discover(Tool::Hyphae).is_some()
}

/// Returns the cached path to the hyphae binary, if available.
pub fn hyphae_binary() -> Option<&'static str> {
    HYPHAE_BINARY_PATH
        .get_or_init(|| {
            discover(Tool::Hyphae)
                .map(|info| info.binary_path.to_str().unwrap_or("hyphae").to_string())
        })
        .as_deref()
}

/// Check config override, then auto-detection.
pub fn should_use_hyphae() -> bool {
    if let Ok(config) = crate::config::Config::load()
        && let Some(hyphae_config) = &config.filters.hyphae
        && let Some(enabled) = hyphae_config.enabled
    {
        return enabled && is_available();
    }
    is_available()
}

/// What to do with command output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputAction {
    /// Small output — return as-is.
    Passthrough,
    /// Medium output or Hyphae unavailable — apply local filter.
    Filter,
    /// Large output + Hyphae available — chunk via Hyphae.
    Chunk,
}

/// Decide how to handle command output based on size and Hyphae availability.
pub fn decide_action(output: &str) -> OutputAction {
    let level = mycelium::adaptive::classify(output);
    match level {
        mycelium::adaptive::AdaptiveLevel::Passthrough => OutputAction::Passthrough,
        mycelium::adaptive::AdaptiveLevel::Structured if should_use_hyphae() => OutputAction::Chunk,
        _ => OutputAction::Filter,
    }
}

/// Validate a filter's output against the raw input.
///
/// Three rules determine whether the filtered output is returned or the raw
/// input is used as a fallback:
///
/// 1. Never return empty from non-empty input.
/// 2. If savings < 20%, filtering isn't worth the information loss.
/// 3. If >95% reduction on output <200 lines, the result is suspiciously aggressive.
fn validate_filter_output(raw: &str, filtered: &str) -> String {
    // Rule 1: Never return empty from non-empty input.
    if filtered.trim().is_empty() && !raw.trim().is_empty() {
        return raw.to_string();
    }

    let raw_tokens = crate::tracking::utils::estimate_tokens(raw);
    if raw_tokens > 0 {
        let filtered_tokens = crate::tracking::utils::estimate_tokens(filtered);
        let savings = 1.0 - (filtered_tokens as f64 / raw_tokens as f64);

        // Rule 2: If savings < 20%, not worth the information loss.
        if savings < 0.20 {
            return raw.to_string();
        }

        // Rule 3: Suspiciously aggressive — >95% reduction on small output.
        let raw_lines = raw.lines().count();
        if raw_lines < 200 && savings > 0.95 {
            return raw.to_string();
        }
    }

    filtered.to_string()
}

/// Check if the filter header should be shown.
fn should_show_filter_header() -> bool {
    crate::config::Config::load()
        .map(|c| c.filters.show_filter_header)
        .unwrap_or(true)
}

/// Route command output through Hyphae or fall back to local filtering.
///
/// - Small outputs pass through unchanged.
/// - Large outputs are sent to Hyphae for chunked storage (if available).
/// - On Hyphae failure or medium outputs, `filter_fn` is applied.
/// - All filter results pass through `validate_filter_output` before returning.
pub fn route_or_filter(command: &str, raw: &str, filter_fn: impl FnOnce(&str) -> String) -> String {
    match decide_action(raw) {
        OutputAction::Passthrough => raw.to_string(),
        OutputAction::Filter => {
            let filtered = filter_fn(raw);
            let validated = validate_filter_output(raw, &filtered);
            // If validation kept the filtered output (not raw fallback), add header
            if validated != raw && should_show_filter_header() {
                add_filter_header(command, raw, &validated)
            } else {
                validated
            }
        }
        OutputAction::Chunk => match crate::hyphae_client::store_output(command, raw, None) {
            Ok(summary) => format_chunk_summary(command, &summary),
            Err(e) => {
                eprintln!(
                    "[mycelium] Hyphae chunking failed, falling back to filter: {}",
                    e
                );
                let filtered = filter_fn(raw);
                let validated = validate_filter_output(raw, &filtered);
                if validated != raw && should_show_filter_header() {
                    add_filter_header(command, raw, &validated)
                } else {
                    validated
                }
            }
        },
    }
}

fn format_chunk_summary(command: &str, summary: &crate::hyphae_client::ChunkSummary) -> String {
    format!(
        "[mycelium→hyphae] {}: {}. Use hyphae_get_command_chunks(document_id=\"{}\") for details.",
        command, summary.summary, summary.document_id
    )
}

/// Add a transparency header when output has been filtered.
///
/// Format: `[mycelium filtered 847→12 lines, 4230→156 tokens (96%) | `mycelium proxy <cmd>` for raw]`
///
/// The header shows:
/// - Line count reduction (raw → filtered)
/// - Token count reduction (raw → filtered)
/// - Compression percentage
/// - How to get raw output via `mycelium proxy`
fn add_filter_header(command: &str, raw: &str, filtered: &str) -> String {
    let raw_lines = raw.lines().count();
    let filtered_lines = filtered.lines().count();
    let raw_tokens = crate::tracking::utils::estimate_tokens(raw);
    let filtered_tokens = crate::tracking::utils::estimate_tokens(filtered);
    let savings_pct = if raw_tokens > 0 {
        ((1.0 - filtered_tokens as f64 / raw_tokens as f64) * 100.0) as usize
    } else {
        0
    };

    format!(
        "[mycelium filtered {}→{} lines, {}→{} tokens ({}%) | `mycelium proxy {}` for raw]\n{}",
        raw_lines, filtered_lines, raw_tokens, filtered_tokens, savings_pct, command, filtered
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available_does_not_panic() {
        // In CI/test environment, hyphae is likely not installed
        // This test just verifies the function doesn't panic
        let _available = is_available();
    }

    #[test]
    fn test_decide_action_small_output() {
        let small = "hello world\n";
        assert_eq!(decide_action(small), OutputAction::Passthrough);
    }

    #[test]
    fn test_decide_action_medium_output() {
        // ~600 tokens (2400 chars / 4) — between passthrough (500) and light (2000) thresholds,
        // with enough lines to avoid the ≤5-line passthrough override.
        let medium = format!("{}\n", "a".repeat(24)).repeat(100); // ~600 tokens, 100 lines
        assert_eq!(decide_action(&medium), OutputAction::Filter);
    }

    #[test]
    fn test_decide_action_large_output() {
        // ~3000 tokens (12000 chars / 4) — above the light (2000) threshold → Structured.
        // With Hyphae available: Chunk; without: Filter.
        let large = format!("{}\n", "a".repeat(100)).repeat(120); // ~3000 tokens, 120 lines
        if is_available() {
            assert_eq!(decide_action(&large), OutputAction::Chunk);
        } else {
            assert_eq!(decide_action(&large), OutputAction::Filter);
        }
    }

    #[test]
    fn test_route_or_filter_passthrough() {
        let small = "hello\n";
        let result = route_or_filter("test", small, |_| "FILTERED".to_string());
        assert_eq!(result, small);
    }

    #[test]
    fn test_route_or_filter_applies_filter() {
        // Medium input: ~600 tokens → filtered to ~300 tokens (50% savings).
        // 50% savings passes Rule 2, and the savings aren't suspiciously aggressive.
        // Using 100 lines so it routes through Filter action (token count > 500).
        let medium = format!("{}\n", "a".repeat(24)).repeat(100); // ~600 tokens, 100 lines
        let filtered_output = format!("{}\n", "a".repeat(24)).repeat(50); // ~300 tokens, 50% savings
        let result = route_or_filter("test", &medium, move |_| filtered_output);
        // Result should contain filtered output (possibly with header prepended)
        assert!(result.contains(&"a".repeat(24)), "Filter output should be present");
        assert_ne!(result, medium, "Should not be raw passthrough");
    }

    #[test]
    fn test_route_or_filter_large_output() {
        // Large output (>2000 tokens) — routes through Hyphae if available,
        // otherwise falls back to filter (which may itself be validated back to raw).
        let large = format!("{}\n", "a".repeat(100)).repeat(120); // ~3000 tokens, 120 lines
        let result = route_or_filter("test", &large, |_| "FILTERED".to_string());
        if is_available() {
            // Hyphae available: either a Hyphae summary, or filter output (or raw on
            // validation fallback if Hyphae fails internally)
            assert!(
                result.contains("[mycelium→hyphae]") || !result.is_empty(),
                "Expected non-empty result from Hyphae or filter fallback"
            );
        } else {
            // No Hyphae: filter runs, then validate_filter_output fires.
            // "FILTERED" has >95% reduction on <200 lines → raw fallback.
            assert_eq!(result, large, "Large output without Hyphae: validation returns raw");
        }
    }

    #[test]
    fn test_route_or_filter_empty_filter_falls_back_to_raw() {
        // A filter that returns empty should fall back to raw output.
        // Use content above the 500-token passthrough threshold so it routes through Filter.
        let medium = format!("{}\n", "a".repeat(24)).repeat(100); // ~600 tokens, 100 lines
        let result = route_or_filter("test", &medium, |_| String::new());
        assert_eq!(result, medium, "Empty filter output should fall back to raw");
    }

    #[test]
    fn test_route_or_filter_whitespace_filter_falls_back_to_raw() {
        // Use content above the 500-token passthrough threshold.
        let medium = format!("{}\n", "a".repeat(24)).repeat(100); // ~600 tokens, 100 lines
        let result = route_or_filter("test", &medium, |_| "   \n  ".to_string());
        assert_eq!(result, medium, "Whitespace-only filter output should fall back to raw");
    }

    // ── validate_filter_output: rule-by-rule tests ────────────────────────────

    #[test]
    fn test_validate_rule1_empty_filtered_returns_raw() {
        let raw = "some output\nwith content\n";
        let result = validate_filter_output(raw, "");
        assert_eq!(result, raw, "Rule 1: empty filtered should return raw");
    }

    #[test]
    fn test_validate_rule1_whitespace_filtered_returns_raw() {
        let raw = "some output\nwith content\n";
        let result = validate_filter_output(raw, "   \n  ");
        assert_eq!(result, raw, "Rule 1: whitespace-only filtered should return raw");
    }

    #[test]
    fn test_validate_rule1_empty_raw_returns_empty_filtered() {
        // When raw is empty, filtering empty to empty is fine
        let result = validate_filter_output("", "");
        assert_eq!(result, "", "Rule 1: empty filtered from empty raw is ok");
    }

    #[test]
    fn test_validate_rule2_low_savings_returns_raw() {
        // Raw: 100 tokens, filtered: 90 tokens → 10% savings — below 20% threshold
        let raw = "a".repeat(400);    // ~100 tokens
        let filtered = "a".repeat(360); // ~90 tokens (10% savings)
        let result = validate_filter_output(&raw, &filtered);
        assert_eq!(result, raw, "Rule 2: <20% savings should return raw");
    }

    #[test]
    fn test_validate_rule2_sufficient_savings_returns_filtered() {
        // Raw: 100 tokens, filtered: 70 tokens → 30% savings — above 20% threshold
        let raw = "a".repeat(400);    // ~100 tokens
        let filtered = "a".repeat(280); // ~70 tokens (30% savings)
        let result = validate_filter_output(&raw, &filtered);
        assert_eq!(result, filtered, "Rule 2: ≥20% savings should return filtered");
    }

    #[test]
    fn test_validate_rule3_aggressive_small_output_returns_raw() {
        // Raw: 50 lines, filtered: 1 line → >95% reduction on <200 lines
        let raw = "line of content here\n".repeat(50); // 50 lines, substantial tokens
        let filtered = "x"; // essentially empty — >95% reduction
        let result = validate_filter_output(&raw, filtered);
        assert_eq!(result, raw, "Rule 3: >95% reduction on <200 lines should return raw");
    }

    #[test]
    fn test_validate_rule3_aggressive_large_output_passes() {
        // Raw: 200+ lines → rule 3 does not apply (raw_lines >= 200)
        let raw = "line of content here\n".repeat(200); // exactly 200 lines
        // filtered: just 1 token — >95% savings, but raw_lines is not < 200
        let filtered = "x";
        // With 200 lines, rule 3 doesn't fire; rule 2 might fire if savings > 0.20
        // ~200*5=1000 tokens raw, 1 token filtered → 99.9% savings > 20%
        // Rule 3: raw_lines < 200 is false (200 is not < 200), so filtered passes
        let result = validate_filter_output(&raw, filtered);
        assert_eq!(result, filtered, "Rule 3: ≥200 lines allows aggressive reduction");
    }

    #[test]
    fn test_validate_all_rules_pass_returns_filtered() {
        // 50 lines, 40% savings, not suspiciously aggressive
        let raw = "word word word word\n".repeat(50); // ~250 tokens
        let filtered = "word word word word\n".repeat(30); // ~150 tokens → 40% savings
        let result = validate_filter_output(&raw, &filtered);
        assert_eq!(result, filtered, "All rules pass: should return filtered output");
    }

    #[test]
    fn test_format_chunk_summary() {
        let summary = crate::hyphae_client::ChunkSummary {
            summary: "5 tests passed".to_string(),
            document_id: "abc123".to_string(),
            chunk_count: 3,
        };
        let result = format_chunk_summary("cargo test", &summary);
        assert!(result.contains("[mycelium→hyphae]"));
        assert!(result.contains("cargo test"));
        assert!(result.contains("5 tests passed"));
        assert!(result.contains("abc123"));
        assert!(result.contains("hyphae_get_command_chunks"));
    }

    #[test]
    fn test_add_filter_header_format() {
        let raw = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let filtered = "line 1\nline 5\n";
        let result = add_filter_header("git log", raw, filtered);

        // Verify header is present
        assert!(result.starts_with("[mycelium filtered"));
        // Verify header contains line count
        assert!(result.contains("5→2 lines"));
        // Verify header contains token count
        assert!(result.contains("tokens"));
        // Verify header contains savings percentage
        assert!(result.contains("%"));
        // Verify header contains proxy command
        assert!(result.contains("`mycelium proxy git log` for raw"));
        // Verify filtered output follows header
        assert!(result.contains("line 1\nline 5"));
    }

    #[test]
    fn test_add_filter_header_no_savings() {
        let raw = "hello";
        let filtered = "hello";
        let result = add_filter_header("cmd", raw, filtered);

        // Even with no compression, header should show 0% savings
        assert!(result.contains("(0%)"));
        assert!(result.contains("1→1 lines"));
    }
}
