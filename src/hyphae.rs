//! Hyphae integration — optional chunked storage for large command outputs.

use std::sync::OnceLock;

/// Whether the Hyphae binary is available in PATH.
static HYPHAE_AVAILABLE: OnceLock<bool> = OnceLock::new();
/// Cached path to the Hyphae binary.
static HYPHAE_PATH: OnceLock<Option<String>> = OnceLock::new();

/// Check if the Hyphae binary is available in PATH.
/// Result is cached after first call.
pub fn is_available() -> bool {
    *HYPHAE_AVAILABLE.get_or_init(|| detect_hyphae().is_some())
}

/// Returns the cached path to the hyphae binary, if available.
pub fn hyphae_binary() -> Option<&'static str> {
    HYPHAE_PATH.get_or_init(detect_hyphae).as_deref()
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

fn detect_hyphae() -> Option<String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("where").arg("hyphae").output();

    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new("which").arg("hyphae").output();

    match result {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                // Take just the first line (in case `where` returns multiple)
                Some(path.lines().next().unwrap_or(&path).to_string())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_hyphae_returns_option() {
        // In CI/test environment, hyphae is likely not installed
        // This test just verifies the function doesn't panic
        let result = detect_hyphae();
        // result is either Some(path) or None — both are valid
        if let Some(path) = &result {
            assert!(!path.is_empty());
        }
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
    fn test_decide_action_large_output_no_hyphae() {
        // 600 lines — large, but Hyphae not available in test → Filter
        let large = "line\n".repeat(600);
        // Without Hyphae in PATH, large output falls back to Filter
        assert_eq!(decide_action(&large), OutputAction::Filter);
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
    fn test_route_or_filter_large_falls_back_without_hyphae() {
        // Large output, but Hyphae not installed → should fall back to filter
        let large = "line\n".repeat(600);
        let result = route_or_filter("test", &large, |_| "FILTERED".to_string());
        assert_eq!(result, "FILTERED");
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
