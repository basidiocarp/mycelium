use super::*;
use clap::Parser;

#[test]
fn test_rewrite_explain_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "rewrite", "--explain", "git status"]).unwrap();
    match cli.command {
        Commands::Rewrite { cmd, explain } => {
            assert!(explain);
            assert_eq!(cmd, "git status");
        }
        _ => panic!("Expected Rewrite command"),
    }
}

#[test]
fn test_invoke_command_parses() {
    let cli = Cli::try_parse_from(["mycelium", "invoke", "git", "status"]).unwrap();
    match cli.command {
        Commands::Invoke { command, explain } => {
            assert_eq!(command, vec!["git".to_string(), "status".to_string()]);
            assert!(!explain);
        }
        _ => panic!("Expected Invoke command"),
    }
}

#[test]
fn test_invoke_preserves_single_argument_with_spaces() {
    let cli = Cli::try_parse_from(["mycelium", "invoke", "rg", "foo bar", "src"]).unwrap();
    match cli.command {
        Commands::Invoke { command, explain } => {
            assert_eq!(
                command,
                vec!["rg".to_string(), "foo bar".to_string(), "src".to_string()]
            );
            assert!(!explain);
        }
        _ => panic!("Expected Invoke command"),
    }
}

#[test]
fn test_cc_economics_project_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "economics", "--project"]).unwrap();
    match cli.command {
        Commands::CcEconomics {
            project,
            project_path,
            ..
        } => {
            assert!(project);
            assert!(project_path.is_none());
        }
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_cc_economics_project_path_parses() {
    let cli = Cli::try_parse_from(["mycelium", "economics", "--project-path", "."]).unwrap();
    match cli.command {
        Commands::CcEconomics {
            project,
            project_path,
            ..
        } => {
            assert!(!project);
            assert_eq!(project_path.as_deref(), Some("."));
        }
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_cc_economics_alias_still_parses() {
    let cli = Cli::try_parse_from(["mycelium", "cc-economics", "--project"]).unwrap();
    match cli.command {
        Commands::CcEconomics { project, .. } => assert!(project),
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_gain_diagnostics_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--diagnostics"]).unwrap();
    match cli.command {
        Commands::Gain { diagnostics, .. } => assert!(diagnostics),
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_limit_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--history", "--limit", "25"]).unwrap();
    match cli.command {
        Commands::Gain { history, limit, .. } => {
            assert!(history);
            assert_eq!(limit, 25);
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_diagnostics_conflicts_with_history() {
    let err = match Cli::try_parse_from(["mycelium", "gain", "--diagnostics", "--history"]) {
        Ok(_) => panic!("expected clap conflict"),
        Err(err) => err,
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn test_gain_diagnostics_conflicts_with_format() {
    let err = match Cli::try_parse_from(["mycelium", "gain", "--diagnostics", "--format", "json"]) {
        Ok(_) => panic!("expected clap conflict"),
        Err(err) => err,
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn test_gain_diagnostics_explain_flag_requires_diagnostics() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--diagnostics", "--explain"]).unwrap();
    match cli.command {
        Commands::Gain {
            diagnostics,
            explain,
            ..
        } => {
            assert!(diagnostics);
            assert!(explain);
        }
        _ => panic!("Expected Gain command"),
    }

    let err = match Cli::try_parse_from(["mycelium", "gain", "--explain"]) {
        Ok(_) => panic!("expected clap validation error"),
        Err(err) => err,
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}

#[test]
fn test_init_hook_modes_still_parse() {
    let cli = Cli::try_parse_from(["mycelium", "init", "--global", "--hook-only"]).unwrap();
    match cli.command {
        Commands::Init {
            global,
            hook_only,
            claude_md,
            onboard,
            show,
            uninstall,
            ..
        } => {
            assert!(global);
            assert!(hook_only);
            assert!(!claude_md);
            assert!(!onboard);
            assert!(!show);
            assert!(!uninstall);
        }
        _ => panic!("Expected Init command"),
    }

    let cli = Cli::try_parse_from(["mycelium", "init", "--claude-md"]).unwrap();
    match cli.command {
        Commands::Init {
            global,
            hook_only,
            claude_md,
            onboard,
            ..
        } => {
            assert!(!global);
            assert!(!hook_only);
            assert!(claude_md);
            assert!(!onboard);
        }
        _ => panic!("Expected Init command"),
    }
}

#[test]
fn test_init_onboard_parses() {
    let cli = Cli::try_parse_from(["mycelium", "init", "--onboard"]).unwrap();
    match cli.command {
        Commands::Init {
            global,
            hook_only,
            claude_md,
            onboard,
            show,
            uninstall,
            ..
        } => {
            assert!(!global);
            assert!(!hook_only);
            assert!(!claude_md);
            assert!(onboard);
            assert!(!show);
            assert!(!uninstall);
        }
        _ => panic!("Expected Init command"),
    }
}

#[test]
fn test_removed_init_setup_flags_are_rejected() {
    for argv in [
        ["mycelium", "init", "--ecosystem"].as_slice(),
        ["mycelium", "init", "--client", "codex"].as_slice(),
    ] {
        let err = match Cli::try_parse_from(argv) {
            Ok(_) => panic!("expected parse failure"),
            Err(err) => err,
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}
