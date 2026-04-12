//! Rewrites shell commands to their Mycelium equivalents for transparent hook integration.
use crate::discover::registry;
use crate::learn::corrections_store;

/// Run the `mycelium rewrite` command.
///
/// Prints the Mycelium-rewritten command to stdout and exits 0.
/// Exits 1 (without output) if the command has no Mycelium equivalent.
///
/// Resolution order:
///   1. Built-in registry (`src/discover/registry.rs`)
///   2. User-learned corrections (`.claude/rules/cli-corrections.json` in cwd)
///
/// Used by shell hooks to rewrite commands transparently:
/// ```bash
/// REWRITTEN=$(mycelium rewrite "$CMD") || exit 0
/// [ "$CMD" = "$REWRITTEN" ] && exit 0  # already Mycelium, skip
/// ```
pub fn run(cmd: &str, explain_mode: bool) -> anyhow::Result<()> {
    if explain_mode {
        print!("{}", self::explain(cmd));
        return Ok(());
    }

    let resolution = resolve(cmd);
    if let Some(rewritten) = resolution.rewritten {
        print!("{}", rewritten);
        return Ok(());
    }

    std::process::exit(1);
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeResolution {
    pub input: String,
    pub command: String,
    pub rewritten: bool,
    pub source: String,
    pub reason: String,
    pub estimated_savings_pct: Option<f64>,
}

pub(crate) fn resolve_runtime_command(cmd: &str) -> RuntimeResolution {
    let resolution = resolve(cmd);
    let command = resolution
        .rewritten
        .clone()
        .unwrap_or_else(|| resolution.input.clone());

    RuntimeResolution {
        input: resolution.input.clone(),
        command,
        rewritten: matches!(
            resolution.source,
            RewriteSource::BuiltInRegistry | RewriteSource::LearnedCorrection
        ),
        source: source_label(resolution.source).to_string(),
        reason: resolution.reason,
        estimated_savings_pct: resolution.estimated_savings_pct,
    }
}

pub fn explain(cmd: &str) -> String {
    render_explanation(&resolve(cmd))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewriteSource {
    BuiltInRegistry,
    LearnedCorrection,
    Passthrough,
    NoRewrite,
}

#[derive(Debug, Clone)]
struct RewriteResolution {
    input: String,
    rewritten: Option<String>,
    source: RewriteSource,
    reason: String,
    estimated_savings_pct: Option<f64>,
}

fn resolve_with_inputs(
    cmd: &str,
    excluded: &[String],
    user_corrections: &[corrections_store::UserCorrection],
) -> RewriteResolution {
    let input = cmd.trim().to_string();

    if input.is_empty() {
        return RewriteResolution {
            input,
            rewritten: None,
            source: RewriteSource::NoRewrite,
            reason: "empty command".to_string(),
            estimated_savings_pct: None,
        };
    }

    if let Some(reason) = registry::learned_correction_block_reason(&input, excluded) {
        return RewriteResolution {
            input,
            rewritten: None,
            source: RewriteSource::NoRewrite,
            reason,
            estimated_savings_pct: None,
        };
    }

    if let Some(rewritten) = corrections_store::apply_correction(&input, user_corrections) {
        return RewriteResolution {
            input,
            rewritten: Some(rewritten),
            source: RewriteSource::LearnedCorrection,
            reason: format!("exact match in {}", corrections_store::CORRECTIONS_JSON),
            estimated_savings_pct: None,
        };
    }

    if let Some(rewritten) = registry::rewrite_command(&input, excluded) {
        let source = if rewritten == input {
            RewriteSource::Passthrough
        } else {
            RewriteSource::BuiltInRegistry
        };
        let reason = explain_registry_match(&input, &rewritten, excluded, source);
        let estimated_savings_pct =
            registry_estimated_savings(&input, &rewritten, excluded, source);
        return RewriteResolution {
            input,
            rewritten: Some(rewritten),
            source,
            reason,
            estimated_savings_pct,
        };
    }

    RewriteResolution {
        input: input.clone(),
        rewritten: None,
        source: RewriteSource::NoRewrite,
        reason: explain_no_rewrite(&input, excluded),
        estimated_savings_pct: None,
    }
}

fn resolve(cmd: &str) -> RewriteResolution {
    let excluded = crate::config::Config::load()
        .map(|c| c.hooks.exclude_commands)
        .unwrap_or_default();
    let user_corrections = corrections_store::load_corrections(corrections_store::CORRECTIONS_JSON);
    resolve_with_inputs(cmd, &excluded, &user_corrections)
}

fn render_explanation(resolution: &RewriteResolution) -> String {
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

fn compound_segment_lines(input: &str) -> Vec<String> {
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
        let resolution = resolve_with_inputs(trimmed, &excluded, &user_corrections);
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

fn source_label(source: RewriteSource) -> &'static str {
    match source {
        RewriteSource::BuiltInRegistry => "built-in registry",
        RewriteSource::LearnedCorrection => "learned corrections",
        RewriteSource::Passthrough => "already Mycelium",
        RewriteSource::NoRewrite => "none",
    }
}

fn registry_estimated_savings(
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

fn explain_registry_match(
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

fn explain_no_rewrite(input: &str, excluded: &[String]) -> String {
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

fn explain_no_rewrite_segment(segment: &str, excluded: &[String]) -> String {
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

#[cfg(test)]
mod explain_tests {
    use super::*;

    #[test]
    fn test_explain_supported_registry_rewrite() {
        let explanation = explain("git status");
        assert!(explanation.contains("Source: built-in registry"));
        assert!(explanation.contains("mycelium git status"));
        assert!(explanation.contains("matched Git rule"));
        assert!(explanation.contains("Estimated savings:"));
    }

    #[test]
    fn test_explain_unsupported_command() {
        let explanation = explain("ansible-playbook site.yml");
        assert!(explanation.contains("Result: no rewrite"));
        assert!(explanation.contains("ansible-playbook"));
    }

    #[test]
    fn test_explain_compound_command_lists_segment_breakdown() {
        let explanation = explain("git status && gh pr list --json number");
        assert!(explanation.contains("Segments:"));
        assert!(explanation.contains("git status"));
        assert!(explanation.contains("gh pr list --json number"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::registry::set_find_fd_rewrite_active_for_tests;

    #[test]
    fn test_run_supported_command_succeeds() {
        assert!(registry::rewrite_command("git status", &[]).is_some());
    }

    #[test]
    fn test_run_unsupported_returns_none() {
        assert!(registry::rewrite_command("ansible-playbook site.yml", &[]).is_none());
    }

    #[test]
    fn test_run_already_mycelium_returns_some() {
        assert_eq!(
            registry::rewrite_command("mycelium git status", &[]),
            Some("mycelium git status".into())
        );
    }

    #[test]
    fn test_resolve_runtime_command_prefers_mycelium_equivalent() {
        let resolution = resolve_runtime_command("git status");
        assert_eq!(resolution.command, "mycelium git status");
        assert_eq!(resolution.input, "git status");
        assert!(resolution.rewritten);
        assert_eq!(resolution.source, "built-in registry");
        assert!(resolution.estimated_savings_pct.is_some());
    }

    #[test]
    fn test_resolve_runtime_command_falls_back_to_original() {
        let resolution = resolve_runtime_command("ansible-playbook site.yml");
        assert_eq!(resolution.command, "ansible-playbook site.yml");
        assert!(!resolution.rewritten);
        assert_eq!(resolution.source, "none");
        assert!(resolution.estimated_savings_pct.is_none());
    }

    #[test]
    fn test_learned_correction_respects_rewrite_guards() {
        let corrections = vec![corrections_store::UserCorrection {
            wrong: "git log -10 | grep feat".to_string(),
            right: "mycelium git log -10 | grep feat".to_string(),
        }];

        let resolution = resolve_with_inputs("git log -10 | grep feat", &[], &corrections);
        assert!(resolution.rewritten.is_none());
        assert_eq!(resolution.source, RewriteSource::NoRewrite);
    }

    #[test]
    fn test_learned_correction_respects_nested_wrapper_gh_guards() {
        let corrections = vec![corrections_store::UserCorrection {
            wrong: "mise exec -- just -- gh pr list --json number".to_string(),
            right: "mise exec -- just -- mycelium gh pr list --json number".to_string(),
        }];

        let resolution = resolve_with_inputs(
            "mise exec -- just -- gh pr list --json number",
            &[],
            &corrections,
        );
        assert!(resolution.rewritten.is_none());
        assert_eq!(resolution.source, RewriteSource::NoRewrite);
    }

    #[test]
    fn test_learned_correction_respects_compound_segment_passthrough_guards() {
        let corrections = vec![corrections_store::UserCorrection {
            wrong: "git status && gh pr list --json number".to_string(),
            right: "mycelium git status && mycelium gh pr list --json number".to_string(),
        }];

        let resolution =
            resolve_with_inputs("git status && gh pr list --json number", &[], &corrections);
        assert!(resolution.rewritten.is_none());
        assert_eq!(resolution.source, RewriteSource::NoRewrite);
    }

    #[test]
    fn test_resolve_uses_fd_for_safe_find_commands_when_available() {
        let _guard = set_find_fd_rewrite_active_for_tests(true);

        let resolution = resolve_with_inputs("find . -name '*.rs' -type f", &[], &[]);

        assert_eq!(
            resolution.rewritten,
            Some("fd -e rs --type f .".to_string())
        );
        assert_eq!(resolution.source, RewriteSource::BuiltInRegistry);
        assert_eq!(resolution.estimated_savings_pct, Some(30.0));
        assert!(
            resolution
                .reason
                .contains("rewrote safe find command to `fd`")
        );
    }

    #[test]
    fn test_resolve_routes_diagnostic_commands_to_invoke_passthrough() {
        let resolution = resolve_with_inputs("which git", &[], &[]);

        assert_eq!(
            resolution.rewritten,
            Some("mycelium invoke which git".to_string())
        );
        assert_eq!(resolution.source, RewriteSource::BuiltInRegistry);
        assert_eq!(resolution.estimated_savings_pct, None);
        assert!(
            resolution
                .reason
                .contains("diagnostic passthrough allowlist")
        );
    }
}
