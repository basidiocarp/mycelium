//! Rewrites shell commands to their Mycelium equivalents for transparent hook integration.
use crate::discover::registry;
use crate::learn::corrections_store;

mod rewrite_display;
pub(crate) use rewrite_display::{
    explain_no_rewrite, explain_registry_match, render_explanation,
    registry_estimated_savings, source_label, RewriteResolution, RewriteSource,
};

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


pub(crate) fn resolve_with_inputs_internal(
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
    resolve_with_inputs_internal(cmd, &excluded, &user_corrections)
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

        let resolution = resolve_with_inputs_internal("git log -10 | grep feat", &[], &corrections);
        assert!(resolution.rewritten.is_none());
        assert_eq!(resolution.source, RewriteSource::NoRewrite);
    }

    #[test]
    fn test_learned_correction_respects_nested_wrapper_gh_guards() {
        let corrections = vec![corrections_store::UserCorrection {
            wrong: "mise exec -- just -- gh pr list --json number".to_string(),
            right: "mise exec -- just -- mycelium gh pr list --json number".to_string(),
        }];

        let resolution = resolve_with_inputs_internal(
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
            resolve_with_inputs_internal("git status && gh pr list --json number", &[], &corrections);
        assert!(resolution.rewritten.is_none());
        assert_eq!(resolution.source, RewriteSource::NoRewrite);
    }

    #[test]
    fn test_resolve_uses_fd_for_safe_find_commands_when_available() {
        let _guard = set_find_fd_rewrite_active_for_tests(true);

        let resolution = resolve_with_inputs_internal("find . -name '*.rs' -type f", &[], &[]);

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
        let resolution = resolve_with_inputs_internal("which git", &[], &[]);

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
