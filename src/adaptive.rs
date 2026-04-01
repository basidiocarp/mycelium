//! Adaptive filtering — classifies output size to determine filtering aggressiveness.

use crate::config::{CompactionProfile, CompactionTuning, current_compaction_tuning};
use crate::tracking::utils::estimate_tokens;

const PASSTHROUGH_TOKEN_THRESHOLD: usize = 500;
const LIGHT_TOKEN_THRESHOLD: usize = 2000;

/// How aggressively to filter based on output size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdaptiveLevel {
    /// Small output — return as-is, no filtering needed.
    Passthrough,
    /// Medium output — apply light filtering.
    Light,
    /// Large output — apply full structured filtering.
    Structured,
}

/// Classify content by token count using default thresholds.
///
/// - `≤500 tokens` → `Passthrough`
/// - `≤2000 tokens` → `Light`
/// - `>2000 tokens` → `Structured`
pub fn classify_by_tokens(content: &str) -> AdaptiveLevel {
    let tokens = estimate_tokens(content);

    if tokens <= PASSTHROUGH_TOKEN_THRESHOLD {
        AdaptiveLevel::Passthrough
    } else if tokens <= LIGHT_TOKEN_THRESHOLD {
        AdaptiveLevel::Light
    } else {
        AdaptiveLevel::Structured
    }
}

/// Classify content with explicit compaction tuning values.
///
/// Token estimation is the primary signal. Line count ≤5 is a hard passthrough
/// override for very sparse outputs that fall below the token threshold anyway.
pub fn classify_with_tuning(content: &str, tuning: CompactionTuning) -> AdaptiveLevel {
    let tokens = estimate_tokens(content);

    // Token-based primary routing
    if tokens <= tuning.passthrough_tokens {
        return AdaptiveLevel::Passthrough;
    }

    // Secondary: line-based override for very short output
    let line_count = content.lines().count();
    if line_count <= 5 {
        return AdaptiveLevel::Passthrough;
    }

    if tokens <= tuning.light_tokens {
        AdaptiveLevel::Light
    } else {
        AdaptiveLevel::Structured
    }
}

/// Classify content with a named profile's default thresholds.
pub fn classify_with_profile(content: &str, profile: CompactionProfile) -> AdaptiveLevel {
    classify_with_tuning(content, profile.tuning())
}

/// Classify content by size to determine the appropriate `AdaptiveLevel`.
///
/// - `< 50 lines AND < 2048 bytes` → `Passthrough`
/// - `50–500 lines` → `Light`
/// - `> 500 lines` → `Structured`
pub fn classify(content: &str) -> AdaptiveLevel {
    classify_with_tuning(content, current_compaction_tuning())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── classify_by_tokens ────────────────────────────────────────────────────

    #[test]
    fn test_classify_by_tokens_passthrough_empty() {
        assert_eq!(classify_by_tokens(""), AdaptiveLevel::Passthrough);
    }

    #[test]
    fn test_classify_by_tokens_passthrough_short() {
        // ~10 tokens — well below 500
        let content = "hello world\n".repeat(10);
        assert_eq!(classify_by_tokens(&content), AdaptiveLevel::Passthrough);
    }

    #[test]
    fn test_classify_by_tokens_passthrough_at_threshold() {
        // Exactly 500 tokens worth: 500 * 4 = 2000 chars
        let content = "a".repeat(2000);
        assert_eq!(classify_by_tokens(&content), AdaptiveLevel::Passthrough);
    }

    #[test]
    fn test_classify_by_tokens_light_just_above_passthrough() {
        // 501 tokens: 2004 chars
        let content = "a".repeat(2004);
        assert_eq!(classify_by_tokens(&content), AdaptiveLevel::Light);
    }

    #[test]
    fn test_classify_by_tokens_light_at_threshold() {
        // Exactly 2000 tokens: 8000 chars
        let content = "a".repeat(8000);
        assert_eq!(classify_by_tokens(&content), AdaptiveLevel::Light);
    }

    #[test]
    fn test_classify_by_tokens_structured_above_light() {
        // 2001 tokens: 8004 chars
        let content = "a".repeat(8004);
        assert_eq!(classify_by_tokens(&content), AdaptiveLevel::Structured);
    }

    // ── classify_with_tuning (token-primary) ─────────────────────────────────

    #[test]
    fn test_passthrough_small_token_count() {
        // Short lines — stays under 500 tokens
        let content = "line\n".repeat(20);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_light_medium_token_count() {
        // ~1250 tokens (5000 chars / 4) — between 500 and 2000, multi-line to avoid line override
        let content = format!("{}\n", "a".repeat(50)).repeat(100); // 100 lines, ~1250 tokens
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
    }

    #[test]
    fn test_structured_large_token_count() {
        // ~2250 tokens — above 2000, multi-line to avoid line override
        let content = format!("{}\n", "a".repeat(50)).repeat(180); // 180 lines, ~2250 tokens
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Structured
        );
    }

    #[test]
    fn test_passthrough_for_very_small_output_even_when_large_in_bytes() {
        // 5 lines, each 600 bytes — line count hard passthrough (≤5 lines)
        let content = format!("{}\n", "x".repeat(600)).repeat(5);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_passthrough_hard_line_limit_overrides_tokens() {
        // 3 long lines that could exceed 500 tokens but ≤5 lines always passes through
        let content = format!("{}\n", "x".repeat(700)).repeat(3);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_all_profiles_share_same_token_thresholds() {
        // All profiles use passthrough_tokens=500 and light_tokens=2000.
        // A ~1000 token input (multi-line to avoid the ≤5 line override) should
        // be Light for all profiles.
        let content = format!("{}\n", "a".repeat(40)).repeat(100); // 100 lines, ~1000 tokens
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Debug),
            AdaptiveLevel::Light
        );
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Aggressive),
            AdaptiveLevel::Light
        );
    }
}
