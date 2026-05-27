//! Display and rendering functions for rewrite resolution explanations.

use crate::discover::registry;
use crate::learn::corrections_store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RewriteSource {
    BuiltInRegistry,
    LearnedCorrection,
    Passthrough,
    NoRewrite,
}

#[derive(Debug, Clone)]
pub(crate) struct RewriteResolution {
    pub input: String,
    pub rewritten: Option<String>,
    pub source: RewriteSource,
    pub reason: String,
    pub estimated_savings_pct: Option<f64>,
}

pub(crate) fn render_explanation(resolution: &RewriteResolution) -> String {
    let mut out = String::new();
    out.push_str("Mycelium rewrite explanation\n");
    out.push_str(&format!("Input: {}\n", resolution.input));
    out.push_str(&format!("Result: {}\n", result_label(resolution)));
    out.push_str(&format!("Source: {}\n", source_label(resolution.source)));
    if let Some(rewritten) = &resolution.rewritten {
        out.push_str(&format!("Output: {}\n", rewritten));
    }
    if let Some(estimated_savings_pct) = resolution.estimated_savings_pct {
        out.push_str(&format!(
            "Estimated savings: {:.1}%\n",
            estimated_savings_pct
        ));
    }
    out.push_str(&format!("Reason: {}\n", resolution.reason));
    for line in compound_segment_lines(&resolution.input) {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

pub(crate) fn compound_segment_lines(input: &str) -> Vec<String> {
    let segments = registry::split_command_chain(input.trim());
    if segments.len() <= 1 {
        return Vec::new();
    }

    let excluded = crate::config::Config::load()
        .map(|c| c.hooks.exclude_commands)
        .unwrap_or_default();
    let user_corrections = corrections_store::load_corrections(corrections_store::CORRECTIONS_JSON);

    let mut lines = vec!["Segments:".to_string()];
    for segment in segments {
        let trimmed = segment.trim();
        let resolution = super::resolve_with_inputs_internal(trimmed, &excluded, &user_corrections);
        let output = resolution.rewritten.as_deref().unwrap_or(trimmed);
        lines.push(format!(
            "  - {} => {} ({}, reason: {})",
            trimmed,
            output,
            result_label(&resolution),
            resolution.reason
        ));
    }
    lines
}

fn result_label(resolution: &RewriteResolution) -> &'static str {
    match resolution.source {
        RewriteSource::BuiltInRegistry | RewriteSource::LearnedCorrection => "rewritten",
        RewriteSource::Passthrough => "passthrough",
        RewriteSource::NoRewrite => "no rewrite",
    }
}

pub(crate) fn source_label(source: RewriteSource) -> &'static str {
    match source {
        RewriteSource::BuiltInRegistry => "built-in registry",
        RewriteSource::LearnedCorrection => "learned corrections",
        RewriteSource::Passthrough => "already Mycelium",
        RewriteSource::NoRewrite => "none",
    }
}

pub(crate) fn registry_estimated_savings(
    input: &str,
    rewritten: &str,
    excluded: &[String],
    source: RewriteSource,
) -> Option<f64> {
    if matches!(source, RewriteSource::Passthrough) {
        return None;
    }

    if rewritten.starts_with("mycelium invoke ")
        && registry::is_diagnostic_passthrough_command(
            rewritten.trim_start_matches("mycelium invoke ").trim(),
        )
    {
        return None;
    }

    if rewritten.starts_with("fd ") {
        return Some(30.0);
    }

    let trimmed = input.trim();
    let base = registry::rewrite_primary_command(trimmed)?;
    if excluded.iter().any(|entry| entry == &base) {
        return None;
    }

    match registry::classify_command(trimmed) {
        registry::Classification::Supported {
            estimated_savings_pct,
            ..
        } => Some(estimated_savings_pct),
        registry::Classification::Unsupported { .. } | registry::Classification::Ignored => None,
    }
}

pub(crate) fn explain_registry_match(
    input: &str,
    rewritten: &str,
    excluded: &[String],
    source: RewriteSource,
) -> String {
    if matches!(source, RewriteSource::Passthrough) {
        return "command already starts with `mycelium`".to_string();
    }

    if rewritten.starts_with("mycelium invoke ")
        && registry::is_diagnostic_passthrough_command(
            rewritten.trim_start_matches("mycelium invoke ").trim(),
        )
    {
        return "matched diagnostic passthrough allowlist and will execute with raw shell semantics".to_string();
    }

    if rewritten.starts_with("fd ") {
        return "rewrote safe find command to `fd` because `fd` is available and respects .gitignore by default".to_string();
    }

    let trimmed = input.trim();
    if let Some(reason) = registry::rewrite_block_reason(trimmed, excluded) {
        return reason;
    }

    let segments = registry::split_command_chain(trimmed);
    if segments.len() > 1 {
        return format!(
            "compound command matched the built-in registry; {} segment(s) were rewritten independently",
            segments.len()
        );
    }

    let classification = registry::classify_command(trimmed);
    match classification {
        registry::Classification::Supported {
            mycelium_equivalent,
            category,
            estimated_savings_pct,
            status,
        } => {
            let savings = format!("{:.1}", estimated_savings_pct);
            let base = registry::rewrite_primary_command(trimmed).unwrap_or_else(|| {
                trimmed
                    .split_whitespace()
                    .next()
                    .unwrap_or(trimmed)
                    .to_string()
            });
            if excluded.iter().any(|entry| entry == &base) {
                format!("command base `{}` is excluded by config", base)
            } else {
                format!(
                    "matched {} rule (`{}` -> `{}`; status: {}; estimated savings: {}%)",
                    category,
                    base,
                    mycelium_equivalent,
                    status.as_str(),
                    savings
                )
            }
        }
        registry::Classification::Unsupported { base_command } => {
            format!("no built-in rule matched `{}`", base_command)
        }
        registry::Classification::Ignored => "command is ignored by the registry".to_string(),
    }
}

pub(crate) fn explain_no_rewrite(input: &str, excluded: &[String]) -> String {
    if let Some(reason) = registry::rewrite_block_reason(input, excluded) {
        return reason;
    }

    let segments = registry::split_command_chain(input);
    if segments.len() > 1 {
        let reasons: Vec<String> = segments
            .iter()
            .map(|segment| explain_no_rewrite_segment(segment, excluded))
            .collect();
        return format!("compound command was not rewritten: {}", reasons.join("; "));
    }

    explain_no_rewrite_segment(input, excluded)
}

pub(crate) fn explain_no_rewrite_segment(segment: &str, excluded: &[String]) -> String {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return "empty command".to_string();
    }

    if trimmed.starts_with("mycelium ") || trimmed == "mycelium" {
        return "command already starts with `mycelium`".to_string();
    }

    if trimmed.starts_with("head -") {
        return "head is only rewritten for supported numeric forms".to_string();
    }

    let classification = registry::classify_command(trimmed);
    match classification {
        registry::Classification::Supported {
            mycelium_equivalent,
            category,
            estimated_savings_pct,
            status,
        } => {
            let base = registry::rewrite_primary_command(trimmed).unwrap_or_else(|| {
                trimmed
                    .split_whitespace()
                    .next()
                    .unwrap_or(trimmed)
                    .to_string()
            });
            if excluded.iter().any(|entry| entry == &base) {
                format!("command base `{}` is excluded by config", base)
            } else {
                format!(
                    "matched {} rule (`{}` -> `{}`; status: {}; estimated savings: {:.1}%) but the rewrite path did not produce output",
                    category,
                    base,
                    mycelium_equivalent,
                    status.as_str(),
                    estimated_savings_pct
                )
            }
        }
        registry::Classification::Unsupported { base_command } => {
            format!("no built-in rule matched `{}`", base_command)
        }
        registry::Classification::Ignored => "command is ignored by the registry".to_string(),
    }
}
