//! Hyphae integration — optional chunked storage for large command outputs.

use spore::{Tool, discover};
use std::sync::OnceLock;
use tracing::warn;

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
            discover(Tool::Hyphae).map(|info| info.binary_path.to_string_lossy().to_string())
        })
        .as_deref()
}

/// Check config override, then auto-detection.
pub fn should_use_hyphae() -> bool {
    let config = crate::config::Config::load_cached();
    if let Some(hyphae_config) = &config.filters.hyphae {
        if let Some(enabled) = hyphae_config.enabled {
            return enabled && is_available();
        }
    }
    is_available()
}

/// What to do with command output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputAction {
    /// Small output — return as-is.
    Passthrough,
    /// Filter for tokens or summarization.
    Filter,
    /// Large output + Hyphae available — chunk via Hyphae.
    Chunk,
    /// Output above summary threshold — replace with compact summary.
    Summarize,
}

/// Hard ceiling (512 KiB): outputs above this bypass the adaptive classifier and always
/// chunk (or summarize if Hyphae is unavailable). The classifier is unreliable at this scale.
const HARD_CHUNK_CEILING_BYTES: usize = 512 * 1024;

/// Decide how to handle command output based on size and Hyphae availability.
///
/// Dispatch order:
/// 1. **> 512 KiB** — hard ceiling: `Chunk` if Hyphae is available, `Summarize` otherwise.
/// 2. **Structured** (adaptive classifier) — `Chunk` if Hyphae is available.
/// 3. **≥ summary_threshold tokens** — `Summarize`.
/// 4. **Non-Passthrough adaptive level** — `Filter`.
/// 5. **Passthrough adaptive level** — `Passthrough`.
pub fn decide_action(output: &str, summary_threshold: usize) -> OutputAction {
    use tracing::debug;

    // Hard ceiling: bypass the classifier for very large outputs.
    if output.len() > HARD_CHUNK_CEILING_BYTES {
        let action = if is_available() {
            OutputAction::Chunk
        } else {
            OutputAction::Summarize
        };
        debug!(
            decision = ?action,
            output_bytes = output.len(),
            "decide_action: hard ceiling exceeded"
        );
        return action;
    }

    let level = mycelium::adaptive::classify(output);

    // Hyphae chunking takes priority — it preserves full retrievability.
    if level == mycelium::adaptive::AdaptiveLevel::Structured && should_use_hyphae() {
        debug!(
            decision = "Chunk",
            hyphae_chosen = true,
            "decide_action: hyphae path selected/skipped"
        );
        return OutputAction::Chunk;
    }

    // Summarize large outputs when Hyphae is unavailable.
    let tokens = crate::tracking::utils::estimate_tokens(output);
    if tokens >= summary_threshold {
        debug!(
            decision = "Summarize",
            tokens = tokens,
            threshold = summary_threshold,
            "decide_action: hyphae path selected/skipped"
        );
        return OutputAction::Summarize;
    }

    let action = match level {
        mycelium::adaptive::AdaptiveLevel::Passthrough => OutputAction::Passthrough,
        _ => OutputAction::Filter,
    };
    debug!(
        decision = ?action,
        hyphae_chosen = false,
        "decide_action: hyphae path selected/skipped"
    );
    action
}

fn get_summary_threshold() -> usize {
    crate::config::Config::load_cached()
        .filters
        .summary
        .map(|summary_config| summary_config.threshold_tokens)
        .unwrap_or(crate::summarizer::DEFAULT_SUMMARY_THRESHOLD_TOKENS)
}

/// Validate a filter's output against the raw input.
///
/// Four rules determine whether the filtered output is returned or the raw
/// input is used as a fallback:
///
/// 1. Never return empty from non-empty input.
/// 2. If savings < 20%, filtering isn't worth the information loss.
/// 3. If filter reported Degraded quality and savings < 40%, prefer raw.
/// 4. If >95% reduction on output <200 lines, the result is suspiciously aggressive.
#[allow(clippy::cast_precision_loss)]
pub(crate) fn validate_filter_output(
    raw: &str,
    result: crate::filter::FilterResult,
) -> crate::filter::FilterResult {
    use crate::filter::{FilterQuality, FilterResult};

    // Rule 1: Never return empty from non-empty input.
    if result.output.trim().is_empty() && !raw.trim().is_empty() {
        return FilterResult::passthrough(raw);
    }

    if result.input_tokens > 0 {
        let savings = 1.0 - (result.output_tokens as f64 / result.input_tokens as f64);

        // Rule 2: If savings < 20%, not worth the information loss.
        if savings < 0.20 {
            tracing::debug!(
                "filter rejected (rule 2 — savings below threshold): savings={:.1}%",
                savings * 100.0
            );
            return FilterResult::passthrough(raw);
        }

        // Rule 3: Degraded filter with modest savings — not worth the risk.
        if result.quality == FilterQuality::Degraded && savings < 0.40 {
            tracing::debug!(
                "filter rejected (rule 3 — degraded quality with insufficient savings): savings={:.1}%",
                savings * 100.0
            );
            return FilterResult::passthrough(raw);
        }

        // Rule 4: Suspiciously aggressive — >95% reduction on small output.
        let raw_lines = raw.lines().count();
        if raw_lines < 200 && savings > 0.95 {
            tracing::debug!(
                "filter rejected (rule 4 — excessive reduction on short content): savings={:.1}%",
                savings * 100.0
            );
            return FilterResult::passthrough(raw);
        }
    }

    result
}

/// Check if the filter header should be shown.
fn should_show_filter_header() -> bool {
    crate::config::Config::load_cached()
        .filters
        .show_filter_header
}

/// Route command output through Hyphae, summarize, or fall back to local filtering.
///
/// - Small outputs pass through unchanged.
/// - Outputs above summary threshold are replaced with compact summary.
/// - Large outputs are sent to Hyphae for chunked storage (if available).
/// - On Hyphae failure or medium outputs, `filter_fn` is applied.
/// - All filter results pass through `validate_filter_output` before returning.
pub fn route_or_filter(
    command: &str,
    raw: &str,
    filter_fn: impl FnOnce(&str) -> crate::filter::FilterResult,
) -> crate::filter::FilterResult {
    use crate::filter::{FilterQuality, FilterResult};

    let summary_threshold = get_summary_threshold();
    match decide_action(raw, summary_threshold) {
        OutputAction::Passthrough => FilterResult::passthrough(raw),
        OutputAction::Summarize => {
            // exit_code not available in this routing context; pass 0 so the
            // summarizer doesn't emit a spurious FAIL line from the exit-code path.
            if let Some(summary) = crate::summarizer::summarize(raw, command, summary_threshold, 0)
            {
                // Record summary silently (don't fail if tracking has issues)
                if let Ok(tracker) = crate::tracking::Tracker::new() {
                    if let Err(e) = tracker.record_summary(
                        command,
                        &summary.summary,
                        summary.input_tokens,
                        summary.output_tokens,
                        0,    // exec_time_ms not available in this context
                        None, // exit_code not available
                    ) {
                        warn!("Failed to record summary in hyphae tracking: {e}");
                    }
                }
                FilterResult::full(raw, summary.summary)
            } else {
                // Fallback to filter if summarization returns None
                let result = filter_fn(raw);
                validate_filter_output(raw, result)
            }
        }
        OutputAction::Filter => {
            let result = filter_fn(raw);
            let validated = validate_filter_output(raw, result);
            if validated.quality != FilterQuality::Passthrough && should_show_filter_header() {
                let output = add_filter_header(command, raw, &validated.output);
                FilterResult {
                    output,
                    ..validated
                }
            } else {
                validated
            }
        }
        OutputAction::Chunk => match crate::hyphae_client::store_output(command, raw, None) {
            Ok(summary) => FilterResult::full(raw, format_chunk_summary(command, &summary)),
            Err(e) => {
                warn!(
                    "[mycelium] Hyphae chunking failed, falling back to filter: {}",
                    e
                );
                let result = filter_fn(raw);
                let validated = validate_filter_output(raw, result);
                if validated.quality != FilterQuality::Passthrough && should_show_filter_header() {
                    let output = add_filter_header(command, raw, &validated.output);
                    FilterResult {
                        output,
                        ..validated
                    }
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
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
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
#[path = "hyphae_tests.rs"]
mod tests;
