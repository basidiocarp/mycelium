//! Mycelium CLI entry point — parses arguments and dispatches to command handlers.
mod aws_cmd;
mod benchmark_cmd;
mod cargo_cmd;
mod cargo_filters;
mod cc_economics;
mod ccusage;
mod commands;
mod completions_cmd;
mod config;
mod container_cmd;
mod curl_cmd;
mod deps;
mod discover;
mod dispatch;
mod display_helpers;
mod doctor_cmd;
mod env_cmd;
mod fileops;
pub use fileops::diff_cmd;
pub use fileops::find_cmd;
pub use fileops::grep_cmd;
pub use fileops::log_cmd;
pub use fileops::ls_cmd;
pub use fileops::read_cmd;
pub use fileops::tree_cmd;
pub use fileops::wc_cmd;
mod filter;
mod filtered_cmd;
mod format_cmd;
mod gain;
mod go_eco;
mod parse_health_cmd;
mod vcs;
pub use vcs::gh_cmd;
pub use vcs::git;
pub use vcs::gt_cmd;
mod hook_audit_cmd;
mod hook_check;
mod init;
mod integrity;
mod js;
mod json_cmd;
mod json_output;
mod learn;
mod lint_cmd;
mod local_llm;
mod parser;
mod plugin;
mod plugin_cmd;
mod psql_cmd;
mod python;
mod rewrite_cmd;
mod runner_cmd;
mod self_update_cmd;
mod streaming;
mod summary_cmd;
mod tee;
mod terraform_cmd;
mod tracking;
mod utils;
mod wget_cmd;

use anyhow::Result;
use clap::Parser;
use clap::error::ErrorKind;

use commands::Cli;

fn main() -> Result<()> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                e.exit();
            }
            return dispatch::run_fallback(e);
        }
    };

    // Warn if installed hook is outdated (1/day, non-blocking).
    // Deferred to after CLI parsing to avoid file I/O on --help/--version/meta-commands.
    hook_check::maybe_warn();

    dispatch::dispatch(cli)
}

#[cfg(test)]
mod tests {
    use super::commands::*;
    use clap::Parser;
    use clap::error::ErrorKind;

    #[test]
    fn test_git_commit_single_message() {
        let cli = Cli::try_parse_from(["mycelium", "git", "commit", "-m", "fix: typo"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["-m", "fix: typo"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_commit_multiple_messages() {
        let cli = Cli::try_parse_from([
            "mycelium",
            "git",
            "commit",
            "-m",
            "feat: add support",
            "-m",
            "Body paragraph here.",
        ])
        .unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(
                    args,
                    vec!["-m", "feat: add support", "-m", "Body paragraph here."]
                );
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    // #327: git commit -am "msg" was rejected by Clap
    #[test]
    fn test_git_commit_am_flag() {
        let cli = Cli::try_parse_from(["mycelium", "git", "commit", "-am", "quick fix"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["-am", "quick fix"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_commit_amend() {
        let cli =
            Cli::try_parse_from(["mycelium", "git", "commit", "--amend", "-m", "new msg"]).unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(args, vec!["--amend", "-m", "new msg"]);
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_git_global_options_parsing() {
        let cli = Cli::try_parse_from([
            "mycelium",
            "git",
            "--no-pager",
            "--no-optional-locks",
            "status",
        ])
        .unwrap();
        match cli.command {
            Commands::Git {
                no_pager,
                no_optional_locks,
                bare,
                literal_pathspecs,
                ..
            } => {
                assert!(no_pager);
                assert!(no_optional_locks);
                assert!(!bare);
                assert!(!literal_pathspecs);
            }
            _ => panic!("Expected Git command"),
        }
    }

    #[test]
    fn test_git_commit_long_flag_multiple() {
        let cli = Cli::try_parse_from([
            "mycelium",
            "git",
            "commit",
            "--message",
            "title",
            "--message",
            "body",
            "--message",
            "footer",
        ])
        .unwrap();
        match cli.command {
            Commands::Git {
                command: GitCommands::Commit { args },
                ..
            } => {
                assert_eq!(
                    args,
                    vec![
                        "--message",
                        "title",
                        "--message",
                        "body",
                        "--message",
                        "footer"
                    ]
                );
            }
            _ => panic!("Expected Git Commit command"),
        }
    }

    #[test]
    fn test_try_parse_valid_git_status() {
        let result = Cli::try_parse_from(["mycelium", "git", "status"]);
        assert!(result.is_ok(), "git status should parse successfully");
    }

    #[test]
    fn test_try_parse_help_is_display_help() {
        match Cli::try_parse_from(["mycelium", "--help"]) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayHelp),
            Ok(_) => panic!("Expected DisplayHelp error"),
        }
    }

    #[test]
    fn test_try_parse_version_is_display_version() {
        match Cli::try_parse_from(["mycelium", "--version"]) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayVersion),
            Ok(_) => panic!("Expected DisplayVersion error"),
        }
    }

    #[test]
    fn test_try_parse_unknown_subcommand_is_error() {
        match Cli::try_parse_from(["mycelium", "nonexistent-command"]) {
            Err(e) => assert!(!matches!(
                e.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            )),
            Ok(_) => panic!("Expected parse error for unknown subcommand"),
        }
    }

    #[test]
    fn test_try_parse_git_with_dash_c_succeeds() {
        let result = Cli::try_parse_from(["mycelium", "git", "-C", "/path", "status"]);
        assert!(
            result.is_ok(),
            "git -C /path status should parse successfully"
        );
        if let Ok(cli) = result {
            match cli.command {
                Commands::Git { directory, .. } => {
                    assert_eq!(directory, vec!["/path"]);
                }
                _ => panic!("Expected Git command"),
            }
        }
    }

    #[test]
    fn test_gain_failures_flag_parses() {
        let result = Cli::try_parse_from(["mycelium", "gain", "--failures"]);
        assert!(result.is_ok());
        if let Ok(cli) = result {
            match cli.command {
                Commands::Gain { failures, .. } => assert!(failures),
                _ => panic!("Expected Gain command"),
            }
        }
    }

    #[test]
    fn test_gain_failures_short_flag_parses() {
        let result = Cli::try_parse_from(["mycelium", "gain", "-F"]);
        assert!(result.is_ok());
        if let Ok(cli) = result {
            match cli.command {
                Commands::Gain { failures, .. } => assert!(failures),
                _ => panic!("Expected Gain command"),
            }
        }
    }

    #[test]
    fn test_meta_commands_reject_bad_flags() {
        // Mycelium meta-commands should produce parse errors (not fall through to raw execution).
        // Skip "proxy" because it uses trailing_var_arg (accepts any args by design).
        for cmd in MYCELIUM_META_COMMANDS {
            if *cmd == "proxy" {
                continue;
            }
            let result = Cli::try_parse_from(["mycelium", cmd, "--nonexistent-flag-xyz"]);
            assert!(
                result.is_err(),
                "Meta-command '{}' with bad flag should fail to parse",
                cmd
            );
        }
    }

    #[test]
    fn test_meta_command_list_is_complete() {
        // Verify all meta-commands are in the guard list by checking they parse with valid syntax
        let meta_cmds_that_parse = [
            vec!["mycelium", "gain"],
            vec!["mycelium", "discover"],
            vec!["mycelium", "learn"],
            vec!["mycelium", "init"],
            vec!["mycelium", "config"],
            vec!["mycelium", "proxy", "echo", "hi"],
            vec!["mycelium", "hook-audit"],
            vec!["mycelium", "cc-economics"],
        ];
        for args in &meta_cmds_that_parse {
            let result = Cli::try_parse_from(args.iter());
            assert!(
                result.is_ok(),
                "Meta-command {:?} should parse successfully",
                args
            );
        }
    }
}
