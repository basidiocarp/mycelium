//! Mycelium library interface — curated helpers for embedding rewrite, filtering,
//! and tracking behavior in other tools.
pub mod adaptive;
#[path = "config.rs"]
pub mod config;
pub mod discover;
#[path = "vcs/git_filters/mod.rs"]
pub mod git_filters;
pub mod learn;
pub mod platform;
#[allow(dead_code)]
#[path = "plugin.rs"]
mod plugin;
#[allow(dead_code)]
#[path = "tee.rs"]
mod tee;
#[path = "tracking/mod.rs"]
pub mod tracking;

pub use adaptive::{AdaptiveLevel, classify, classify_with_profile, classify_with_tuning};
pub use config::{
    CompactionProfile, CompactionTuning, Config, config_path, current_compaction_profile,
    current_compaction_tuning,
};
pub use discover::registry::{
    Classification, classify_command, rewrite_command, split_command_chain,
};
pub use git_filters::{
    compact_diff, compact_diff_with_profile, filter_branch_output, filter_log_output,
    filter_stash_list, filter_status_with_args, filter_worktree_list, format_status_output,
    format_status_output_with_profile,
};
pub use tracking::{DbPathInfo, DbPathSource, TimedExecution, Tracker, resolve_db_path_info};
