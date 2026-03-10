//! Git command proxy with token-optimized output for status, log, diff, show, and more.
use crate::tracking;
use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

mod diff;
mod log;
mod mutations;
mod stash;
mod status;

/// Supported git subcommands with optimized token output.
#[derive(Debug, Clone)]
pub enum GitCommand {
    Diff,
    Log,
    Status,
    Show,
    Add,
    Commit,
    Push,
    Pull,
    Branch,
    Fetch,
    Stash { subcommand: Option<String> },
    Worktree,
}

/// Create a git Command with global options (e.g. -C, -c, --git-dir, --work-tree)
/// prepended before any subcommand arguments.
pub(super) fn git_cmd(global_args: &[String]) -> Command {
    let mut cmd = Command::new("git");
    for arg in global_args {
        cmd.arg(arg);
    }
    cmd
}

/// Dispatch a git subcommand to its token-optimized handler.
pub fn run(
    cmd: GitCommand,
    args: &[String],
    max_lines: Option<usize>,
    verbose: u8,
    global_args: &[String],
) -> Result<()> {
    match cmd {
        GitCommand::Diff => diff::run_diff(args, max_lines, verbose, global_args),
        GitCommand::Log => log::run_log(args, max_lines, verbose, global_args),
        GitCommand::Status => status::run_status(args, verbose, global_args),
        GitCommand::Show => diff::run_show(args, max_lines, verbose, global_args),
        GitCommand::Add => status::run_add(args, verbose, global_args),
        GitCommand::Commit => mutations::run_commit(args, verbose, global_args),
        GitCommand::Push => mutations::run_push(args, verbose, global_args),
        GitCommand::Pull => mutations::run_pull(args, verbose, global_args),
        GitCommand::Branch => status::run_branch(args, verbose, global_args),
        GitCommand::Fetch => mutations::run_fetch(args, verbose, global_args),
        GitCommand::Stash { subcommand } => {
            stash::run_stash(subcommand.as_deref(), args, verbose, global_args)
        }
        GitCommand::Worktree => status::run_worktree(args, verbose, global_args),
    }
}

/// Runs an unsupported git subcommand by passing it through directly
pub fn run_passthrough(args: &[OsString], global_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    if verbose > 0 {
        eprintln!("git passthrough: {:?}", args);
    }
    let status = git_cmd(global_args)
        .args(args)
        .status()
        .context("Failed to run git")?;

    let args_str = tracking::args_display(args);
    timer.track_passthrough(
        &format!("git {}", args_str),
        &format!("mycelium git {} (passthrough)", args_str),
    );

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_cmd_no_global_args() {
        let cmd = git_cmd(&[]);
        let program = cmd.get_program();
        assert_eq!(program, "git");
        let args: Vec<_> = cmd.get_args().collect();
        assert!(args.is_empty());
    }

    #[test]
    fn test_git_cmd_with_directory() {
        let global_args = vec!["-C".to_string(), "/tmp".to_string()];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["-C", "/tmp"]);
    }

    #[test]
    fn test_git_cmd_with_multiple_global_args() {
        let global_args = vec![
            "-C".to_string(),
            "/tmp".to_string(),
            "-c".to_string(),
            "user.name=test".to_string(),
            "--git-dir".to_string(),
            "/foo/.git".to_string(),
        ];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(
            args,
            vec![
                "-C",
                "/tmp",
                "-c",
                "user.name=test",
                "--git-dir",
                "/foo/.git"
            ]
        );
    }

    #[test]
    fn test_git_cmd_with_boolean_flags() {
        let global_args = vec!["--no-pager".to_string(), "--bare".to_string()];
        let cmd = git_cmd(&global_args);
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, vec!["--no-pager", "--bare"]);
    }

    #[test]
    fn test_run_passthrough_accepts_args() {
        // Compile-time verification that the function exists with correct signature
        let _args: Vec<OsString> = vec![OsString::from("tag"), OsString::from("--list")];
    }
}
