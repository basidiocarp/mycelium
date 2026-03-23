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

/// Route command output through Hyphae or fall back to local filtering.
///
/// - Small outputs pass through unchanged.
/// - Large outputs are sent to Hyphae for chunked storage (if available).
/// - On Hyphae failure or medium outputs, `filter_fn` is applied.
pub fn route_or_filter(command: &str, raw: &str, filter_fn: impl FnOnce(&str) -> String) -> String {
    match decide_action(raw) {
        OutputAction::Passthrough => raw.to_string(),
        OutputAction::Filter => filter_fn(raw),
        OutputAction::Chunk => match crate::hyphae_client::store_output(command, raw, None) {
            Ok(summary) => format_chunk_summary(command, &summary),
            Err(e) => {
                eprintln!(
                    "[mycelium] Hyphae chunking failed, falling back to filter: {}",
                    e
                );
                filter_fn(raw)
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
        // 100 lines — medium, should be Filter (Hyphae likely not in PATH during tests)
        let medium = "line\n".repeat(100);
        assert_eq!(decide_action(&medium), OutputAction::Filter);
    }

    #[test]
    fn test_decide_action_large_output() {
        // 600 lines — large output
        let large = "line\n".repeat(600);
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
        let medium = "line\n".repeat(100);
        let result = route_or_filter("test", &medium, |_| "FILTERED".to_string());
        assert_eq!(result, "FILTERED");
    }

    #[test]
    fn test_route_or_filter_large_output() {
        // Large output — routes through Hyphae if available, otherwise falls back to filter
        let large = "line\n".repeat(600);
        let result = route_or_filter("test", &large, |_| "FILTERED".to_string());
        if is_available() {
            assert!(
                result.contains("[mycelium→hyphae]") || result == "FILTERED",
                "Expected Hyphae summary or fallback filter, got: {}",
                &result[..result.len().min(100)]
            );
        } else {
            assert_eq!(result, "FILTERED");
        }
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
}
