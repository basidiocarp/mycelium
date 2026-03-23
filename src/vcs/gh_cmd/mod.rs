//! GitHub CLI (gh) command output compression.
//!
//! Provides token-optimized alternatives to verbose `gh` commands.
//! Focuses on extracting essential information from JSON outputs.

mod issue;
mod markdown;
mod parsers;
mod passthrough;
mod repo;
mod run;

// Re-export public types and helpers needed by sibling modules and external callers.
pub(crate) use markdown::filter_markdown_body;
pub use parsers::{GhIssueListParser, GhIssueViewParser, GhRepoViewParser, GhRunListParser};
pub use passthrough::run_passthrough_gh;
pub(crate) use passthrough::{run_passthrough_fn, run_passthrough_with_extra};

/// Check if args contain --json flag (user wants specific JSON fields, not Mycelium filtering)
fn has_json_flag_impl<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().any(|a| a.as_ref() == "--json")
}

/// Check if args contain --json flag for owned String slices.
pub(crate) fn has_json_flag(args: &[String]) -> bool {
    has_json_flag_impl(args)
}

/// Check if args contain --json flag for borrowed string slices.
fn has_json_flag_str(args: &[&str]) -> bool {
    has_json_flag_impl(args)
}

/// Extract a positional identifier (PR/issue number) from args, returning it
/// separately from the remaining extra flags (like -R, --repo, etc.).
/// Handles both `view 123 -R owner/repo` and `view -R owner/repo 123`.
fn extract_identifier_and_extra_args_impl(args: &[String]) -> (Option<String>, Vec<String>) {
    if args.is_empty() {
        return (None, Vec::new());
    }

    // Known gh flags that take a value — skip these and their values
    let flags_with_value = ["-R", "--repo", "-q", "--jq", "-t", "--template"];
    let mut identifier = None;
    let mut extra = Vec::new();
    let mut skip_next = false;

    for arg in args {
        if skip_next {
            extra.push(arg.clone());
            skip_next = false;
            continue;
        }
        if flags_with_value.contains(&arg.as_str()) {
            extra.push(arg.clone());
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            extra.push(arg.clone());
            continue;
        }
        // First non-flag arg is the identifier (number/URL)
        if identifier.is_none() {
            identifier = Some(arg.clone());
        } else {
            extra.push(arg.clone());
        }
    }

    (identifier, extra)
}

pub(crate) fn extract_identifier_and_extra_args(args: &[String]) -> Option<(String, Vec<String>)> {
    let (identifier, extra) = extract_identifier_and_extra_args_impl(args);
    identifier.map(|id| (id, extra))
}

pub(crate) fn extract_optional_identifier_and_extra_args(
    args: &[String],
) -> (Option<String>, Vec<String>) {
    extract_identifier_and_extra_args_impl(args)
}

fn str_slice_to_strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// Run a gh command with token-optimized output (legacy entry point kept for backward compat).
pub fn run(subcommand: &str, args: &[String], verbose: u8, ultra_compact: bool) -> Result<()> {
    // When user explicitly passes --json, they want raw gh JSON output, not Mycelium filtering
    if has_json_flag(args) {
        return passthrough::run_passthrough_fn("gh", subcommand, args);
    }

    match subcommand {
        "pr" => super::gh_pr::run_pr(args, verbose, ultra_compact),
        "issue" => issue::dispatch_issue(args, verbose, ultra_compact),
        "run" => run::run_workflow(args, verbose, ultra_compact),
        "repo" => repo::dispatch_repo(args, verbose, ultra_compact),
        "api" => repo::dispatch_api(args, verbose),
        _ => {
            // Unknown subcommand, pass through
            passthrough::run_passthrough_fn("gh", subcommand, args)
        }
    }
}

// ---------------------------------------------------------------------------
// Typed public entry points — called directly by the new dispatch layer.
// Each function accepts borrowed str slices to avoid Vec<String> allocations.
// ---------------------------------------------------------------------------

/// Dispatch a PR sub-command. `pr_sub` is e.g. "list", "view", "checks".
pub fn run_pr(pr_sub: &str, args: &[&str], verbose: u8, ultra_compact: bool) -> Result<()> {
    if has_json_flag_str(args) {
        let mut pt: Vec<&str> = vec!["pr", pr_sub];
        pt.extend_from_slice(args);
        return passthrough::run_passthrough_gh(&pt, verbose);
    }
    let mut owned = vec![pr_sub.to_string()];
    owned.extend(str_slice_to_strings(args));
    super::gh_pr::run_pr(&owned, verbose, ultra_compact)
}

/// Dispatch an issue sub-command. `issue_sub` is e.g. "list", "view".
pub fn run_issue(issue_sub: &str, args: &[&str], verbose: u8) -> Result<()> {
    if has_json_flag_str(args) {
        let mut pt: Vec<&str> = vec!["issue", issue_sub];
        pt.extend_from_slice(args);
        return passthrough::run_passthrough_gh(&pt, verbose);
    }
    let mut owned = vec![issue_sub.to_string()];
    owned.extend(str_slice_to_strings(args));
    issue::dispatch_issue(&owned, verbose, false)
}

/// Dispatch a workflow run sub-command. `run_sub` is e.g. "list", "view".
pub fn run_run(run_sub: &str, args: &[&str], verbose: u8, ultra_compact: bool) -> Result<()> {
    if has_json_flag_str(args) {
        let mut pt: Vec<&str> = vec!["run", run_sub];
        pt.extend_from_slice(args);
        return passthrough::run_passthrough_gh(&pt, verbose);
    }
    let mut owned = vec![run_sub.to_string()];
    owned.extend(str_slice_to_strings(args));
    run::run_workflow(&owned, verbose, ultra_compact)
}

/// Dispatch a repo sub-command (default: "view").
pub fn run_repo(args: &[&str], verbose: u8, ultra_compact: bool) -> Result<()> {
    if has_json_flag_str(args) {
        let mut pt: Vec<&str> = vec!["repo"];
        pt.extend_from_slice(args);
        return passthrough::run_passthrough_gh(&pt, verbose);
    }
    let owned = str_slice_to_strings(args);
    repo::dispatch_repo(&owned, verbose, ultra_compact)
}

/// Dispatch `gh api` — always passes through (preserves full JSON response).
pub fn run_api(args: &[&str], verbose: u8) -> Result<()> {
    let owned = str_slice_to_strings(args);
    repo::dispatch_api(&owned, verbose)
}

use anyhow::Result;

#[cfg(test)]
mod tests {
    use super::issue::should_passthrough_issue_view;
    use super::*;
    use crate::utils::truncate;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(
            truncate("this is a very long string", 15),
            "this is a ve..."
        );
    }

    #[test]
    fn test_truncate_multibyte_utf8() {
        // Emoji: 🚀 = 4 bytes, 1 char
        assert_eq!(truncate("🚀🎉🔥abc", 6), "🚀🎉🔥abc"); // 6 chars, fits
        assert_eq!(truncate("🚀🎉🔥abcdef", 8), "🚀🎉🔥ab..."); // 10 chars > 8
        // Edge case: all multibyte
        assert_eq!(truncate("🚀🎉🔥🌟🎯", 5), "🚀🎉🔥🌟🎯"); // exact fit
        assert_eq!(truncate("🚀🎉🔥🌟🎯x", 5), "🚀🎉..."); // 6 chars > 5
    }

    #[test]
    fn test_truncate_empty_and_short() {
        assert_eq!(truncate("", 10), "");
        assert_eq!(truncate("ab", 10), "ab");
        assert_eq!(truncate("abc", 3), "abc"); // exact fit
    }

    #[test]
    fn test_has_json_flag_present() {
        assert!(has_json_flag(&[
            "view".into(),
            "--json".into(),
            "number,url".into()
        ]));
    }

    #[test]
    fn test_has_json_flag_absent() {
        assert!(!has_json_flag(&["view".into(), "42".into()]));
    }

    #[test]
    fn test_extract_identifier_simple() {
        let args: Vec<String> = vec!["123".into()];
        let (id, extra) = extract_identifier_and_extra_args(&args).unwrap();
        assert_eq!(id, "123");
        assert!(extra.is_empty());
    }

    #[test]
    fn test_extract_identifier_with_repo_flag_after() {
        // gh issue view 185 -R OWNER/mycelium
        let args: Vec<String> = vec!["185".into(), "-R".into(), "mycelium-ai/mycelium".into()];
        let (id, extra) = extract_identifier_and_extra_args(&args).unwrap();
        assert_eq!(id, "185");
        assert_eq!(extra, vec!["-R", "mycelium-ai/mycelium"]);
    }

    #[test]
    fn test_extract_identifier_with_repo_flag_before() {
        // gh issue view -R OWNER/mycelium 185
        let args: Vec<String> = vec!["-R".into(), "mycelium-ai/mycelium".into(), "185".into()];
        let (id, extra) = extract_identifier_and_extra_args(&args).unwrap();
        assert_eq!(id, "185");
        assert_eq!(extra, vec!["-R", "mycelium-ai/mycelium"]);
    }

    #[test]
    fn test_extract_identifier_with_long_repo_flag() {
        let args: Vec<String> = vec!["42".into(), "--repo".into(), "owner/repo".into()];
        let (id, extra) = extract_identifier_and_extra_args(&args).unwrap();
        assert_eq!(id, "42");
        assert_eq!(extra, vec!["--repo", "owner/repo"]);
    }

    #[test]
    fn test_extract_identifier_empty() {
        let args: Vec<String> = vec![];
        assert!(extract_identifier_and_extra_args(&args).is_none());
    }

    #[test]
    fn test_extract_optional_identifier_with_web_flag() {
        let args: Vec<String> = vec!["--web".into()];
        let (identifier, extra) = extract_optional_identifier_and_extra_args(&args);
        assert!(identifier.is_none());
        assert_eq!(extra, vec!["--web"]);
    }

    #[test]
    fn test_extract_identifier_only_flags() {
        // No positional identifier, only flags
        let args: Vec<String> = vec!["-R".into(), "mycelium-ai/mycelium".into()];
        assert!(extract_identifier_and_extra_args(&args).is_none());
    }

    #[test]
    fn test_extract_identifier_with_web_flag() {
        let args: Vec<String> = vec!["123".into(), "--web".into()];
        let (id, extra) = extract_identifier_and_extra_args(&args).unwrap();
        assert_eq!(id, "123");
        assert_eq!(extra, vec!["--web"]);
    }

    #[test]
    fn test_should_passthrough_issue_view_comments() {
        assert!(should_passthrough_issue_view(&["--comments".into()]));
    }

    #[test]
    fn test_should_passthrough_issue_view_json_and_web() {
        assert!(should_passthrough_issue_view(&["--json".into()]));
        assert!(should_passthrough_issue_view(&["--web".into()]));
    }

    #[test]
    fn test_should_passthrough_issue_view_default() {
        assert!(!should_passthrough_issue_view(&[]));
    }
}
