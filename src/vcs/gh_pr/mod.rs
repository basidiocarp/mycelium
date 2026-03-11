//! GitHub CLI PR command output compression.
//!
//! Handles all PR-related gh subcommands: list, view, checks, status,
//! create, merge, diff, comment, edit.

mod actions;
mod checks;
mod list;
mod view;

use anyhow::Result;

pub fn run_pr(args: &[String], verbose: u8, ultra_compact: bool) -> Result<()> {
    if args.is_empty() {
        return crate::vcs::gh_cmd::run_passthrough_fn("gh", "pr", args);
    }

    match args[0].as_str() {
        "list" => list::list_prs(&args[1..], verbose, ultra_compact),
        "view" => view::view_pr(&args[1..], verbose, ultra_compact),
        "checks" => checks::pr_checks(&args[1..], verbose, ultra_compact),
        "status" => checks::pr_status(verbose, ultra_compact),
        "create" => actions::pr_create(&args[1..], verbose),
        "merge" => actions::pr_merge(&args[1..], verbose),
        "diff" => actions::pr_diff(&args[1..], verbose),
        "comment" => actions::pr_action("commented", args, verbose),
        "edit" => actions::pr_action("edited", args, verbose),
        _ => crate::vcs::gh_cmd::run_passthrough_fn("gh", "pr", args),
    }
}
