//! Adaptive filtering — classifies output size to determine filtering aggressiveness.

use crate::config::{CompactionProfile, CompactionTuning, current_compaction_tuning};

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

/// Classify content with explicit compaction tuning values.
pub fn classify_with_tuning(content: &str, tuning: CompactionTuning) -> AdaptiveLevel {
    let line_count = content.lines().count();
    let byte_count = content.len();

    if line_count <= 5 {
        return AdaptiveLevel::Passthrough;
    }

    if line_count < tuning.adaptive_small_lines && byte_count < tuning.adaptive_small_bytes {
        AdaptiveLevel::Passthrough
    } else if line_count <= tuning.adaptive_large_lines {
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

    #[test]
    fn test_passthrough_small() {
        let content = "line\n".repeat(49);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_light_at_50_lines() {
        let content = "line\n".repeat(50);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
    }

    #[test]
    fn test_light_at_51_lines() {
        let content = "line\n".repeat(51);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
    }

    #[test]
    fn test_light_at_500_lines() {
        let content = "line\n".repeat(500);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
    }

    #[test]
    fn test_structured_at_501_lines() {
        let content = "line\n".repeat(501);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Structured
        );
    }

    #[test]
    fn test_light_when_large_bytes_but_few_lines() {
        // 30 lines but >2KB — bytes alone don't trigger Passthrough
        let content = format!("{}\n", "x".repeat(100)).repeat(30);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Light
        );
    }

    #[test]
    fn test_passthrough_for_very_small_output_even_when_large_in_bytes() {
        let content = format!("{}\n", "x".repeat(600)).repeat(5);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Balanced),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_debug_profile_is_more_permissive() {
        let content = "line\n".repeat(70);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Debug),
            AdaptiveLevel::Passthrough
        );
    }

    #[test]
    fn test_aggressive_profile_structures_sooner() {
        let content = "line\n".repeat(300);
        assert_eq!(
            classify_with_profile(&content, CompactionProfile::Aggressive),
            AdaptiveLevel::Structured
        );
    }
}
