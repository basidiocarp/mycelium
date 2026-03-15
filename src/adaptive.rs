//! Adaptive filtering — classifies output size to determine filtering aggressiveness.

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

/// Classify content by size to determine the appropriate `AdaptiveLevel`.
///
/// - `< 50 lines AND < 2048 bytes` → `Passthrough`
/// - `50–500 lines` → `Light`
/// - `> 500 lines` → `Structured`
pub fn classify(content: &str) -> AdaptiveLevel {
    let line_count = content.lines().count();
    let byte_count = content.len();

    if line_count < 50 && byte_count < 2048 {
        AdaptiveLevel::Passthrough
    } else if line_count <= 500 {
        AdaptiveLevel::Light
    } else {
        AdaptiveLevel::Structured
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passthrough_small() {
        let content = "line\n".repeat(49);
        assert_eq!(classify(&content), AdaptiveLevel::Passthrough);
    }

    #[test]
    fn test_light_at_50_lines() {
        let content = "line\n".repeat(50);
        assert_eq!(classify(&content), AdaptiveLevel::Light);
    }

    #[test]
    fn test_light_at_51_lines() {
        let content = "line\n".repeat(51);
        assert_eq!(classify(&content), AdaptiveLevel::Light);
    }

    #[test]
    fn test_light_at_500_lines() {
        let content = "line\n".repeat(500);
        assert_eq!(classify(&content), AdaptiveLevel::Light);
    }

    #[test]
    fn test_structured_at_501_lines() {
        let content = "line\n".repeat(501);
        assert_eq!(classify(&content), AdaptiveLevel::Structured);
    }

    #[test]
    fn test_light_when_large_bytes_but_few_lines() {
        // 30 lines but >2KB — bytes alone don't trigger Passthrough
        let content = format!("{}\n", "x".repeat(100)).repeat(30);
        assert_eq!(classify(&content), AdaptiveLevel::Light);
    }
}
