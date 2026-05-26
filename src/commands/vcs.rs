use clap::Args;

pub use super::subcommands::{GitCommands, GhCommands, GtCommands};

/// Git commands with compact output
#[allow(clippy::struct_excessive_bools)]
#[derive(Args)]
pub struct Git {
    /// Change to directory before executing (like git -C <path>, can be repeated)
    #[arg(short = 'C', action = clap::ArgAction::Append)]
    pub directory: Vec<String>,

    /// Git configuration override (like git -c key=value, can be repeated)
    #[arg(short = 'c', action = clap::ArgAction::Append)]
    pub config_override: Vec<String>,

    /// Set the path to the .git directory
    #[arg(long = "git-dir")]
    pub git_dir: Option<String>,

    /// Set the path to the working tree
    #[arg(long = "work-tree")]
    pub work_tree: Option<String>,

    /// Disable pager (like git --no-pager)
    #[arg(long = "no-pager")]
    pub no_pager: bool,

    /// Skip optional locks (like git --no-optional-locks)
    #[arg(long = "no-optional-locks")]
    pub no_optional_locks: bool,

    /// Treat repository as bare (like git --bare)
    #[arg(long)]
    pub bare: bool,

    /// Treat pathspecs literally (like git --literal-pathspecs)
    #[arg(long = "literal-pathspecs")]
    pub literal_pathspecs: bool,

    #[command(subcommand)]
    pub command: GitCommands,
}

/// GitHub CLI (gh) commands with token-optimized output
#[derive(Args)]
pub struct Gh {
    #[command(subcommand)]
    pub command: GhCommands,
}

/// Graphite (gt) stacked PR commands with compact output
#[derive(Args)]
pub struct Gt {
    #[command(subcommand)]
    pub command: GtCommands,
}
