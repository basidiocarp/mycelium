use super::*;
use clap::Parser;

#[test]
fn test_rewrite_explain_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "rewrite", "--explain", "git status"]).unwrap();
    match cli.command {
        Commands::Rewrite(r) => {
            assert!(r.explain);
            assert_eq!(r.cmd, "git status");
        }
        _ => panic!("Expected Rewrite command"),
    }
}

#[test]
fn test_invoke_command_parses() {
    let cli = Cli::try_parse_from(["mycelium", "invoke", "git", "status"]).unwrap();
    match cli.command {
        Commands::Invoke(invoke) => {
            assert_eq!(
                invoke.command,
                vec!["git".to_string(), "status".to_string()]
            );
            assert!(!invoke.explain);
        }
        _ => panic!("Expected Invoke command"),
    }
}

#[test]
fn test_invoke_preserves_single_argument_with_spaces() {
    let cli = Cli::try_parse_from(["mycelium", "invoke", "rg", "foo bar", "src"]).unwrap();
    match cli.command {
        Commands::Invoke(invoke) => {
            assert_eq!(
                invoke.command,
                vec!["rg".to_string(), "foo bar".to_string(), "src".to_string()]
            );
            assert!(!invoke.explain);
        }
        _ => panic!("Expected Invoke command"),
    }
}

#[test]
fn test_cc_economics_project_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "economics", "--project"]).unwrap();
    match cli.command {
        Commands::CcEconomics(cc) => {
            assert!(cc.project);
            assert!(cc.project_path.is_none());
        }
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_cc_economics_project_path_parses() {
    let cli = Cli::try_parse_from(["mycelium", "economics", "--project-path", "."]).unwrap();
    match cli.command {
        Commands::CcEconomics(cc) => {
            assert!(!cc.project);
            assert_eq!(cc.project_path.as_deref(), Some("."));
        }
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_cc_economics_alias_still_parses() {
    let cli = Cli::try_parse_from(["mycelium", "cc-economics", "--project"]).unwrap();
    match cli.command {
        Commands::CcEconomics(cc) => assert!(cc.project),
        _ => panic!("Expected CcEconomics command"),
    }
}

#[test]
fn test_gain_project_bare_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--project"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            // Bare --project uses default_missing_value "."
            assert_eq!(gain.project.as_deref(), Some("."));
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_project_name_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--project", "mycelium"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            assert_eq!(gain.project.as_deref(), Some("mycelium"));
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_project_all_parses() {
    // Clap produces Some("all"), which gain::run() intercepts before
    // resolve_project_scope to route to show_projects_table.
    let cli = Cli::try_parse_from(["mycelium", "gain", "--project", "all"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            assert_eq!(gain.project.as_deref(), Some("all"));
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_project_conflicts_with_project_path() {
    let err = match Cli::try_parse_from([
        "mycelium",
        "gain",
        "--project",
        "foo",
        "--project-path",
        ".",
    ]) {
        Ok(_) => panic!("expected clap conflict"),
        Err(err) => err,
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn test_gain_project_short_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "-p"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            // Bare -p uses default_missing_value "."
            assert_eq!(gain.project.as_deref(), Some("."));
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_no_project_flag_is_none() {
    let cli = Cli::try_parse_from(["mycelium", "gain"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            assert!(gain.project.is_none());
        }
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_diagnostics_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--diagnostics"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => assert!(gain.diagnostics),
        _ => panic!("Expected Gain command"),
    }
}

#[test]
fn test_gain_limit_flag_parses() {
    let cli = Cli::try_parse_from(["mycelium", "gain", "--history", "--limit", "25"]).unwrap();
    match cli.command {
        Commands::Gain(gain) => {
            assert!(gain.history);
            assert_eq!(gain.limit, 25);
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
        Commands::Gain(gain) => {
            assert!(gain.diagnostics);
            assert!(gain.explain);
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
        Commands::Init(init) => {
            assert!(init.global);
            assert!(init.hook_only);
            assert!(!init.claude_md);
            assert!(!init.onboard);
            assert!(!init.show);
            assert!(!init.uninstall);
        }
        _ => panic!("Expected Init command"),
    }

    let cli = Cli::try_parse_from(["mycelium", "init", "--claude-md"]).unwrap();
    match cli.command {
        Commands::Init(init) => {
            assert!(!init.global);
            assert!(!init.hook_only);
            assert!(init.claude_md);
            assert!(!init.onboard);
        }
        _ => panic!("Expected Init command"),
    }
}

#[test]
fn test_init_onboard_parses() {
    let cli = Cli::try_parse_from(["mycelium", "init", "--onboard"]).unwrap();
    match cli.command {
        Commands::Init(init) => {
            assert!(!init.global);
            assert!(!init.hook_only);
            assert!(!init.claude_md);
            assert!(init.onboard);
            assert!(!init.show);
            assert!(!init.uninstall);
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
