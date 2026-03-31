//! Routes parsed CLI commands to their specialized handler modules.
use anyhow::{Context, Result};
use std::path::Path;

use crate::commands::*;
use crate::{
    atmos_cmd, aws_cmd, cargo_cmd, cc_economics, completions_cmd, config, container_cmd, curl_cmd,
    deps, diff_cmd, discover, env_cmd, find_cmd, format_cmd, gain, gh_cmd, git, go_eco, grep_cmd,
    gt_cmd, hook_audit_cmd, init, integrity, js, json_cmd, json_output, learn, lint_cmd, local_llm,
    log_cmd, ls_cmd, parse_health_cmd, psql_cmd, python, read_cmd, rewrite_cmd, runner_cmd,
    self_update_cmd, summary_cmd, terraform_cmd, tracking, tree_cmd, utils, wc_cmd, wget_cmd,
};

/// Handle Clap parse failures: fall back to raw execution for non-meta commands.
pub fn run_fallback(parse_error: clap::Error) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // No args → show Clap's error (user ran just "mycelium" with bad syntax)
    if args.is_empty() {
        parse_error.exit();
    }

    // Mycelium meta-commands should never fall back to raw execution.
    // e.g. `mycelium gain --badtypo` should show Clap's error, not try to run `gain` from $PATH.
    if MYCELIUM_META_COMMANDS.contains(&args[0].as_str()) {
        parse_error.exit();
    }

    let raw_command = args.join(" ");
    let error_message = utils::strip_ansi(&parse_error.to_string());

    // Start timer before execution to capture actual command runtime
    let timer = tracking::TimedExecution::start();

    // Check for a user plugin before raw passthrough.
    // Plugin lookup respects `[plugins] enabled = false` in config (checked inside find_plugin).
    if let Some(plugin_path) = crate::plugin::find_plugin(&args[0]) {
        // Run the raw command and capture its stdout for the plugin to filter.
        match std::process::Command::new(&args[0])
            .args(&args[1..])
            .output()
        {
            Ok(raw_output) => {
                let raw = String::from_utf8_lossy(&raw_output.stdout).to_string();
                match crate::plugin::run_plugin(&plugin_path, &raw) {
                    Ok(filtered) => {
                        // Track savings: raw input vs filtered output
                        timer.track(
                            &raw_command,
                            &format!("mycelium plugin: {}", raw_command),
                            &raw,
                            &filtered,
                        );
                        tracking::record_parse_failure_silent(
                            &raw_command,
                            &error_message,
                            true,
                            None,
                        );
                        print!("{}", filtered);
                        return Ok(());
                    }
                    Err(e) => {
                        // Plugin failed — log and fall through to passthrough
                        eprintln!("[mycelium: plugin {:?} failed: {}]", plugin_path, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[mycelium: plugin raw capture failed: {}]", e);
            }
        }
        // Fall through to normal passthrough on any plugin error
    }

    let status = std::process::Command::new(&args[0])
        .args(&args[1..])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) => {
            timer.track_passthrough(&raw_command, &format!("mycelium fallback: {}", raw_command));

            tracking::record_parse_failure_silent(&raw_command, &error_message, true, None);

            if !s.success() {
                std::process::exit(s.code().unwrap_or(1));
            }
        }
        Err(e) => {
            tracking::record_parse_failure_silent(&raw_command, &error_message, false, None);
            // Command not found or other OS error — show Clap's original error
            eprintln!("[mycelium: fallback failed: {}]", e);
            parse_error.exit();
        }
    }

    Ok(())
}

/// Dispatch a parsed CLI command to its handler module.
pub fn dispatch(cli: Cli) -> Result<()> {
    // JSON mode: re-invoke self without --json, capture output, wrap in envelope.
    if cli.json {
        return dispatch_json(cli);
    }

    // Runtime integrity check for operational commands.
    // Meta commands (init, gain, verify, config, etc.) skip the check
    // because they don't go through the hook pipeline.
    if is_operational_command(&cli.command) {
        integrity::runtime_check()?;
    }

    match cli.command {
        // ─────────────────────────────────────────────────────────────────────────────
        // File Operations
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Ls { args } => {
            ls_cmd::run(&args, cli.verbose)?;
        }

        Commands::Tree { args } => {
            tree_cmd::run(&args, cli.verbose)?;
        }

        Commands::Read {
            file,
            level,
            max_lines,
            line_numbers,
        } => {
            if file == Path::new("-") {
                read_cmd::run_stdin(level, max_lines, line_numbers, cli.verbose)?;
            } else {
                read_cmd::run(&file, level, max_lines, line_numbers, cli.verbose)?;
            }
        }

        Commands::Peek {
            file,
            model,
            force_download,
        } => {
            local_llm::run(&file, &model, force_download, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Git Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Git {
            directory,
            config_override,
            git_dir,
            work_tree,
            no_pager,
            no_optional_locks,
            bare,
            literal_pathspecs,
            command,
        } => {
            dispatch_git_commands(
                directory,
                config_override,
                git_dir,
                work_tree,
                no_pager,
                no_optional_locks,
                bare,
                literal_pathspecs,
                command,
                cli.verbose,
            )?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // GitHub Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Gh { command } => {
            dispatch_gh_commands(command, cli.verbose, cli.ultra_compact)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // AWS Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Aws { command } => match command {
            AwsCommands::Sts { args } => aws_cmd::run_sts(&args, cli.verbose)?,
            AwsCommands::S3 { args } => aws_cmd::run_s3(&args, cli.verbose)?,
            AwsCommands::Ec2 { args } => aws_cmd::run_ec2(&args, cli.verbose)?,
            AwsCommands::Ecs { args } => aws_cmd::run_ecs(&args, cli.verbose)?,
            AwsCommands::Rds { args } => aws_cmd::run_rds(&args, cli.verbose)?,
            AwsCommands::Cloudformation { args } => {
                aws_cmd::run_cloudformation(&args, cli.verbose)?
            }
            AwsCommands::Other(raw) => {
                let service = raw[0].to_string_lossy().to_string();
                let args: Vec<String> = raw[1..]
                    .iter()
                    .map(|a| a.to_string_lossy().to_string())
                    .collect();
                aws_cmd::run_generic(&service, &args, cli.verbose)?;
            }
        },

        Commands::Atmos { command } => {
            dispatch_atmos_commands(command, cli.verbose)?;
        }

        Commands::Psql { args } => {
            psql_cmd::run(&args, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // JavaScript/Node Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Pnpm { command } => {
            dispatch_pnpm_commands(command, cli.verbose)?;
        }

        Commands::Vitest { command } => match command {
            VitestCommands::Run { args } => {
                js::vitest::run(js::vitest::VitestCommand::Run, &args, cli.verbose)?;
            }
        },

        Commands::Prisma { command } => {
            dispatch_prisma_commands(command, cli.verbose)?;
        }

        Commands::Tsc { args } => {
            js::tsc::run(&args, cli.verbose)?;
        }

        Commands::Next { args } => {
            js::next::run(&args, cli.verbose)?;
        }

        Commands::Lint { args } => {
            lint_cmd::run(&args, cli.verbose)?;
        }

        Commands::Prettier { args } => {
            js::prettier::run(&args, cli.verbose)?;
        }

        Commands::Format { args } => {
            format_cmd::run(&args, cli.verbose)?;
        }

        Commands::Playwright { args } => {
            js::playwright::run(&args, cli.verbose)?;
        }

        Commands::Npm { args } => {
            js::npm::run(&args, cli.verbose, cli.skip_env)?;
        }

        Commands::Npx { ref args } => {
            dispatch_npx(args, &cli)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Cargo Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Cargo { command } => {
            dispatch_cargo_commands(command, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Container Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Docker { command } => {
            dispatch_docker_commands(command, cli.verbose)?;
        }

        Commands::Kubectl { command } => {
            dispatch_kubectl_commands(command, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Python Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Ruff { command } => {
            dispatch_ruff_commands(command, cli.verbose)?;
        }

        Commands::Pytest { args } => {
            python::pytest::run(&args, cli.verbose)?;
        }

        Commands::Mypy { args } => {
            python::mypy::run(&args, cli.verbose)?;
        }

        Commands::Pip { command } => {
            dispatch_pip_commands(command, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Go Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Go { command } => {
            dispatch_go_commands(command, cli.verbose)?;
        }

        Commands::GolangciLint { args } => {
            go_eco::golangci::run(&args, cli.verbose)?;
        }

        Commands::Gt { command } => {
            dispatch_gt_commands(command, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Analysis & Utility Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Err { command } => {
            let cmd = command.join(" ");
            runner_cmd::run_err(&cmd, cli.verbose)?;
        }

        Commands::Test { command } => {
            let cmd = command.join(" ");
            runner_cmd::run_test(&cmd, cli.verbose)?;
        }

        Commands::Json { file, depth } => {
            if file == Path::new("-") {
                json_cmd::run_stdin(depth, cli.verbose)?;
            } else {
                json_cmd::run(&file, depth, cli.verbose)?;
            }
        }

        Commands::Deps { path } => {
            deps::run(&path, cli.verbose)?;
        }

        Commands::Env { filter, show_all } => {
            env_cmd::run(filter.as_deref(), show_all, cli.verbose)?;
        }

        Commands::Find { args } => {
            find_cmd::run_from_args(&args, cli.verbose)?;
        }

        Commands::Diff { file1, file2 } => {
            if let Some(f2) = file2 {
                diff_cmd::run(&file1, &f2, cli.verbose)?;
            } else {
                diff_cmd::run_stdin(cli.verbose)?;
            }
        }

        Commands::Log { file } => {
            if let Some(f) = file {
                log_cmd::run_file(&f, cli.verbose)?;
            } else {
                log_cmd::run_stdin(cli.verbose)?;
            }
        }

        Commands::Summary { command } => {
            let cmd = command.join(" ");
            summary_cmd::run(&cmd, cli.verbose)?;
        }

        Commands::Grep {
            pattern,
            path,
            max_len,
            max,
            context_only,
            file_type,
            line_numbers: _, // no-op: line numbers always enabled in grep_cmd::run
            extra_args,
        } => {
            grep_cmd::run(
                &pattern,
                &path,
                max_len,
                max,
                context_only,
                file_type.as_deref(),
                &extra_args,
                cli.verbose,
            )?;
        }

        Commands::Curl { args } => {
            curl_cmd::run(&args, cli.verbose)?;
        }

        Commands::Wget { url, stdout, args } => {
            if stdout {
                wget_cmd::run_stdout(&url, &args, cli.verbose)?;
            } else {
                wget_cmd::run(&url, &args, cli.verbose)?;
            }
        }

        Commands::Wc { args } => {
            wc_cmd::run(&args, cli.verbose)?;
        }

        // ─────────────────────────────────────────────────────────────────────────────
        // Meta & Configuration Commands
        // ─────────────────────────────────────────────────────────────────────────────
        Commands::Init {
            global,
            show,
            claude_md,
            hook_only,
            auto_patch,
            no_patch,
            uninstall,
        } => {
            dispatch_init_commands(
                global,
                show,
                claude_md,
                hook_only,
                auto_patch,
                no_patch,
                uninstall,
                cli.verbose,
            )?;
        }

        Commands::Gain {
            project,
            project_path,
            projects,
            diagnostics,
            explain,
            graph,
            history,
            limit,
            quota,
            tier,
            daily,
            weekly,
            monthly,
            all,
            format,
            failures,
            status,
            compare,
        } => {
            gain::run(
                project,
                project_path.as_deref(),
                projects,
                diagnostics,
                explain,
                graph,
                history,
                limit,
                quota,
                &tier,
                daily,
                weekly,
                monthly,
                all,
                &format,
                failures,
                status,
                compare.as_deref(),
                cli.verbose,
            )?;
        }

        Commands::ParseHealth { days } => {
            parse_health_cmd::run(days)?;
        }

        Commands::CcEconomics {
            project,
            project_path,
            daily,
            weekly,
            monthly,
            all,
            format,
        } => {
            cc_economics::run(
                project,
                project_path.as_deref(),
                daily,
                weekly,
                monthly,
                all,
                &format,
                cli.verbose,
            )?;
        }

        Commands::Config { create } => {
            if create {
                let path = config::Config::create_default()?;
                println!("Created: {}", path.display());
            } else {
                config::show_config()?;
            }
        }

        Commands::Discover {
            project,
            limit,
            all,
            since,
            format,
        } => {
            discover::run(project.as_deref(), all, since, limit, &format, cli.verbose)?;
        }

        Commands::Learn {
            project,
            all,
            since,
            format,
            write_rules,
            min_confidence,
            min_occurrences,
        } => {
            learn::run(
                project,
                all,
                since,
                format,
                write_rules,
                min_confidence,
                min_occurrences,
            )?;
        }

        Commands::Context {
            task,
            project,
            budget,
            include,
        } => {
            let task_str = task.join(" ");
            init::context::run(
                &task_str,
                project.as_deref(),
                budget,
                include.as_deref(),
                cli.json,
            )?;
        }

        Commands::Terraform { command } => {
            dispatch_terraform_commands(command, cli.verbose)?;
        }

        Commands::HookAudit { since } => {
            hook_audit_cmd::run(since, cli.verbose)?;
        }

        Commands::Rewrite { cmd, explain } => {
            rewrite_cmd::run(&cmd, explain)?;
        }

        Commands::Invoke {
            ref command,
            explain,
        } => {
            dispatch_invoke_command(command, explain, &cli)?;
        }

        Commands::Proxy { ref args } => {
            dispatch_proxy(args, &cli)?;
        }

        Commands::Verify => {
            integrity::run_verify(cli.verbose)?;
        }

        Commands::Doctor => {
            crate::doctor_cmd::run()?;
        }

        Commands::Completions { shell } => completions_cmd::run(&shell)?,

        Commands::SelfUpdate { check } => self_update_cmd::run(check)?,

        Commands::Benchmark { ci } => {
            crate::benchmark_cmd::run(ci)?;
        }

        Commands::Plugin { command } => match command {
            PluginCommands::List => {
                crate::plugin_cmd::run_list()?;
            }
            PluginCommands::Install { ref name, force } => {
                if name == "--all" || name == "all" {
                    crate::plugin_cmd::run_install_all(force)?;
                } else {
                    crate::plugin_cmd::run_install(name, force)?;
                }
            }
        },
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Dispatch Functions (Organized by Command Family)
// ─────────────────────────────────────────────────────────────────────────────

/// Route git subcommands to git::run with global arguments.
#[allow(clippy::too_many_arguments)]
fn dispatch_git_commands(
    directory: Vec<String>,
    config_override: Vec<String>,
    git_dir: Option<String>,
    work_tree: Option<String>,
    no_pager: bool,
    no_optional_locks: bool,
    bare: bool,
    literal_pathspecs: bool,
    command: GitCommands,
    verbose: u8,
) -> Result<()> {
    // Build global git args (inserted between "git" and subcommand)
    let mut global_args: Vec<String> = Vec::new();
    for dir in &directory {
        global_args.push("-C".to_string());
        global_args.push(dir.clone());
    }
    for cfg in &config_override {
        global_args.push("-c".to_string());
        global_args.push(cfg.clone());
    }
    if let Some(ref dir) = git_dir {
        global_args.push("--git-dir".to_string());
        global_args.push(dir.clone());
    }
    if let Some(ref tree) = work_tree {
        global_args.push("--work-tree".to_string());
        global_args.push(tree.clone());
    }
    if no_pager {
        global_args.push("--no-pager".to_string());
    }
    if no_optional_locks {
        global_args.push("--no-optional-locks".to_string());
    }
    if bare {
        global_args.push("--bare".to_string());
    }
    if literal_pathspecs {
        global_args.push("--literal-pathspecs".to_string());
    }

    match command {
        GitCommands::Diff { args } => {
            git::run(git::GitCommand::Diff, &args, None, verbose, &global_args)?;
        }
        GitCommands::Log { args } => {
            git::run(git::GitCommand::Log, &args, None, verbose, &global_args)?;
        }
        GitCommands::Status { args } => {
            git::run(git::GitCommand::Status, &args, None, verbose, &global_args)?;
        }
        GitCommands::Show { args } => {
            git::run(git::GitCommand::Show, &args, None, verbose, &global_args)?;
        }
        GitCommands::Add { args } => {
            git::run(git::GitCommand::Add, &args, None, verbose, &global_args)?;
        }
        GitCommands::Commit { args } => {
            git::run(git::GitCommand::Commit, &args, None, verbose, &global_args)?;
        }
        GitCommands::Push { args } => {
            git::run(git::GitCommand::Push, &args, None, verbose, &global_args)?;
        }
        GitCommands::Pull { args } => {
            git::run(git::GitCommand::Pull, &args, None, verbose, &global_args)?;
        }
        GitCommands::Branch { args } => {
            git::run(git::GitCommand::Branch, &args, None, verbose, &global_args)?;
        }
        GitCommands::Fetch { args } => {
            git::run(git::GitCommand::Fetch, &args, None, verbose, &global_args)?;
        }
        GitCommands::Stash { subcommand, args } => {
            git::run(
                git::GitCommand::Stash { subcommand },
                &args,
                None,
                verbose,
                &global_args,
            )?;
        }
        GitCommands::Worktree { args } => {
            git::run(
                git::GitCommand::Worktree,
                &args,
                None,
                verbose,
                &global_args,
            )?;
        }
        GitCommands::Other(args) => {
            git::run_passthrough(&args, &global_args, verbose)?;
        }
    }

    Ok(())
}

/// Route GitHub CLI commands (pr, issue, run, repo, api, etc).
fn dispatch_gh_commands(command: GhCommands, verbose: u8, ultra_compact: bool) -> Result<()> {
    match command {
        GhCommands::Pr { command } => {
            let (sub, raw_args) = match command {
                GhPrCommands::List { args } => ("list", args),
                GhPrCommands::View { args } => ("view", args),
                GhPrCommands::Checks { args } => ("checks", args),
                GhPrCommands::Status { args } => ("status", args),
                GhPrCommands::Create { args } => ("create", args),
                GhPrCommands::Merge { args } => ("merge", args),
                GhPrCommands::Diff { args } => ("diff", args),
                GhPrCommands::Comment { args } => ("comment", args),
                GhPrCommands::Edit { args } => ("edit", args),
                GhPrCommands::Other(args) => {
                    let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                    gh_cmd::run_passthrough_gh(&borrowed, verbose)?;
                    return Ok(());
                }
            };
            let borrowed: Vec<&str> = raw_args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_pr(sub, &borrowed, verbose, ultra_compact)?;
        }
        GhCommands::Issue { command } => {
            let (sub, raw_args) = match command {
                GhIssueCommands::List { args } => ("list", args),
                GhIssueCommands::View { args } => ("view", args),
                GhIssueCommands::Other(args) => {
                    let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                    gh_cmd::run_passthrough_gh(&borrowed, verbose)?;
                    return Ok(());
                }
            };
            let borrowed: Vec<&str> = raw_args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_issue(sub, &borrowed, verbose)?;
        }
        GhCommands::Run { command } => {
            let (sub, raw_args) = match command {
                GhRunCommands::List { args } => ("list", args),
                GhRunCommands::View { args } => ("view", args),
                GhRunCommands::Other(args) => {
                    let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                    gh_cmd::run_passthrough_gh(&borrowed, verbose)?;
                    return Ok(());
                }
            };
            let borrowed: Vec<&str> = raw_args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_run(sub, &borrowed, verbose, ultra_compact)?;
        }
        GhCommands::Repo { args } => {
            let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_repo(&borrowed, verbose, ultra_compact)?;
        }
        GhCommands::Api { args } => {
            let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_api(&borrowed, verbose)?;
        }
        GhCommands::Other(args) => {
            let borrowed: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            gh_cmd::run_passthrough_gh(&borrowed, verbose)?;
        }
    }

    Ok(())
}

/// Route pnpm subcommands (list, outdated, install, build, typecheck).
fn dispatch_pnpm_commands(command: PnpmCommands, verbose: u8) -> Result<()> {
    match command {
        PnpmCommands::List { depth, args } => {
            js::pnpm::run(js::pnpm::PnpmCommand::List { depth }, &args, verbose)?;
        }
        PnpmCommands::Outdated { args } => {
            js::pnpm::run(js::pnpm::PnpmCommand::Outdated, &args, verbose)?;
        }
        PnpmCommands::Install { packages } => {
            let (pkg_list, extra_args): (Vec<String>, Vec<String>) =
                packages.into_iter().partition(|a| !a.starts_with('-'));
            js::pnpm::run(
                js::pnpm::PnpmCommand::Install { packages: pkg_list },
                &extra_args,
                verbose,
            )?;
        }
        PnpmCommands::Build { args } => {
            js::next::run(&args, verbose)?;
        }
        PnpmCommands::Typecheck { args } => {
            js::tsc::run(&args, verbose)?;
        }
        PnpmCommands::Other(args) => {
            js::pnpm::run_passthrough(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route prisma subcommands (generate, migrate, db push).
fn dispatch_prisma_commands(command: PrismaCommands, verbose: u8) -> Result<()> {
    match command {
        PrismaCommands::Generate { args } => {
            js::prisma::run(js::prisma::PrismaCommand::Generate, &args, verbose)?;
        }
        PrismaCommands::Migrate { command } => match command {
            PrismaMigrateCommands::Dev { name, args } => {
                js::prisma::run(
                    js::prisma::PrismaCommand::Migrate {
                        subcommand: js::prisma::MigrateSubcommand::Dev { name },
                    },
                    &args,
                    verbose,
                )?;
            }
            PrismaMigrateCommands::Status { args } => {
                js::prisma::run(
                    js::prisma::PrismaCommand::Migrate {
                        subcommand: js::prisma::MigrateSubcommand::Status,
                    },
                    &args,
                    verbose,
                )?;
            }
            PrismaMigrateCommands::Deploy { args } => {
                js::prisma::run(
                    js::prisma::PrismaCommand::Migrate {
                        subcommand: js::prisma::MigrateSubcommand::Deploy,
                    },
                    &args,
                    verbose,
                )?;
            }
        },
        PrismaCommands::DbPush { args } => {
            js::prisma::run(js::prisma::PrismaCommand::DbPush, &args, verbose)?;
        }
    }

    Ok(())
}

/// Route cargo subcommands (build, test, clippy, check, install, nextest).
fn dispatch_cargo_commands(command: CargoCommands, verbose: u8) -> Result<()> {
    match command {
        CargoCommands::Build { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Build, &args, verbose)?;
        }
        CargoCommands::Test { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Test, &args, verbose)?;
        }
        CargoCommands::Clippy { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Clippy, &args, verbose)?;
        }
        CargoCommands::Check { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Check, &args, verbose)?;
        }
        CargoCommands::Install { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Install, &args, verbose)?;
        }
        CargoCommands::Nextest { args } => {
            cargo_cmd::run(cargo_cmd::CargoCommand::Nextest, &args, verbose)?;
        }
        CargoCommands::Other(args) => {
            cargo_cmd::run_passthrough(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route docker subcommands (ps, images, logs, compose).
fn dispatch_docker_commands(command: DockerCommands, verbose: u8) -> Result<()> {
    match command {
        DockerCommands::Ps => {
            container_cmd::run(container_cmd::ContainerCmd::DockerPs, &[], verbose)?;
        }
        DockerCommands::Images => {
            container_cmd::run(container_cmd::ContainerCmd::DockerImages, &[], verbose)?;
        }
        DockerCommands::Logs { container: c } => {
            container_cmd::run(container_cmd::ContainerCmd::DockerLogs, &[c], verbose)?;
        }
        DockerCommands::Compose { command: compose } => match compose {
            ComposeCommands::Ps => {
                container_cmd::run_compose_ps(verbose)?;
            }
            ComposeCommands::Logs { service } => {
                container_cmd::run_compose_logs(service.as_deref(), verbose)?;
            }
            ComposeCommands::Build { service } => {
                container_cmd::run_compose_build(service.as_deref(), verbose)?;
            }
            ComposeCommands::Other(args) => {
                container_cmd::run_compose_passthrough(&args, verbose)?;
            }
        },
        DockerCommands::Other(args) => {
            container_cmd::run_docker_passthrough(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route kubectl subcommands (pods, services, logs).
fn dispatch_kubectl_commands(command: KubectlCommands, verbose: u8) -> Result<()> {
    match command {
        KubectlCommands::Pods { namespace, all } => {
            let mut args: Vec<String> = Vec::new();
            if all {
                args.push("-A".to_string());
            } else if let Some(n) = namespace {
                args.push("-n".to_string());
                args.push(n);
            }
            container_cmd::run(container_cmd::ContainerCmd::KubectlPods, &args, verbose)?;
        }
        KubectlCommands::Services { namespace, all } => {
            let mut args: Vec<String> = Vec::new();
            if all {
                args.push("-A".to_string());
            } else if let Some(n) = namespace {
                args.push("-n".to_string());
                args.push(n);
            }
            container_cmd::run(container_cmd::ContainerCmd::KubectlServices, &args, verbose)?;
        }
        KubectlCommands::Logs { pod, container: c } => {
            let mut args = vec![pod];
            if let Some(cont) = c {
                args.push("-c".to_string());
                args.push(cont);
            }
            container_cmd::run(container_cmd::ContainerCmd::KubectlLogs, &args, verbose)?;
        }
        KubectlCommands::Other(args) => {
            container_cmd::run_kubectl_passthrough(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route ruff subcommands (check, format).
fn dispatch_ruff_commands(command: RuffCommands, verbose: u8) -> Result<()> {
    match command {
        RuffCommands::Check { args } => {
            python::ruff::run_check(&args, verbose)?;
        }
        RuffCommands::Format { args } => {
            python::ruff::run_format(&args, verbose)?;
        }
        RuffCommands::Other(raw) => {
            let args: Vec<String> = raw
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let looks_like_path = args
                .first()
                .map(|a| !a.starts_with('-') && a != "version" && a != "rule" && a != "help")
                .unwrap_or(false);
            if looks_like_path {
                python::ruff::run_check(&args, verbose)?;
            } else {
                python::ruff::run_passthrough(&args, verbose)?;
            }
        }
    }

    Ok(())
}

/// Route pip subcommands (list, outdated, install, uninstall, show).
fn dispatch_pip_commands(command: PipCommands, verbose: u8) -> Result<()> {
    match command {
        PipCommands::List { args } => {
            python::pip::run_list(&args, verbose)?;
        }
        PipCommands::Outdated { args } => {
            python::pip::run_outdated(&args, verbose)?;
        }
        PipCommands::Install { args } => {
            python::pip::run_install(&args, verbose)?;
        }
        PipCommands::Uninstall { args } => {
            python::pip::run_uninstall(&args, verbose)?;
        }
        PipCommands::Show { args } => {
            python::pip::run_show(&args, verbose)?;
        }
        PipCommands::Other(args) => {
            python::pip::run_other(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route go subcommands (test, build, vet).
fn dispatch_go_commands(command: GoCommands, verbose: u8) -> Result<()> {
    match command {
        GoCommands::Test { args } => {
            go_eco::commands::run_test(&args, verbose)?;
        }
        GoCommands::Build { args } => {
            go_eco::commands::run_build(&args, verbose)?;
        }
        GoCommands::Vet { args } => {
            go_eco::commands::run_vet(&args, verbose)?;
        }
        GoCommands::Other(args) => {
            go_eco::commands::run_other(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route gt subcommands (log, submit, sync, restack, create, branch).
fn dispatch_gt_commands(command: GtCommands, verbose: u8) -> Result<()> {
    match command {
        GtCommands::Log { args } => {
            gt_cmd::run_log(&args, verbose)?;
        }
        GtCommands::Submit { args } => {
            gt_cmd::run_submit(&args, verbose)?;
        }
        GtCommands::Sync { args } => {
            gt_cmd::run_sync(&args, verbose)?;
        }
        GtCommands::Restack { args } => {
            gt_cmd::run_restack(&args, verbose)?;
        }
        GtCommands::Create { args } => {
            gt_cmd::run_create(&args, verbose)?;
        }
        GtCommands::Branch { args } => {
            gt_cmd::run_branch(&args, verbose)?;
        }
        GtCommands::Other(args) => {
            gt_cmd::run_other(&args, verbose)?;
        }
    }

    Ok(())
}

/// Route init subcommands and options.
#[allow(clippy::too_many_arguments)]
fn dispatch_init_commands(
    global: bool,
    show: bool,
    claude_md: bool,
    hook_only: bool,
    auto_patch: bool,
    no_patch: bool,
    uninstall: bool,
    verbose: u8,
) -> Result<()> {
    if show {
        init::show_config()?;
    } else if uninstall {
        init::uninstall(global, verbose)?;
    } else {
        let patch_mode = if auto_patch {
            init::PatchMode::Auto
        } else if no_patch {
            init::PatchMode::Skip
        } else {
            init::PatchMode::Ask
        };
        init::run(global, claude_md, hook_only, patch_mode, verbose)?;
    }

    Ok(())
}

/// Route terraform subcommands (plan, apply, init).
fn dispatch_terraform_commands(command: TerraformCommands, verbose: u8) -> Result<()> {
    match command {
        TerraformCommands::Plan { args } => {
            terraform_cmd::run_plan(&args, verbose)?;
        }
        TerraformCommands::Apply { args } => {
            terraform_cmd::run_apply(&args, verbose)?;
        }
        TerraformCommands::Init { args } => {
            terraform_cmd::run_init(&args, verbose)?;
        }
        TerraformCommands::Other(args) => {
            terraform_cmd::run_passthrough(
                &args
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
                verbose,
            )?;
        }
    }

    Ok(())
}

fn dispatch_atmos_commands(command: AtmosCommands, verbose: u8) -> Result<()> {
    match command {
        AtmosCommands::Terraform { args } => {
            atmos_cmd::run_terraform(&args, verbose)?;
        }
        AtmosCommands::Describe { args } => {
            atmos_cmd::run_describe(&args, verbose)?;
        }
        AtmosCommands::Validate { args } => {
            atmos_cmd::run_validate(&args, verbose)?;
        }
        AtmosCommands::Workflow { args } => {
            atmos_cmd::run_workflow(&args, verbose)?;
        }
        AtmosCommands::Version { args } => {
            atmos_cmd::run_version(&args, verbose)?;
        }
        AtmosCommands::Other(args) => {
            atmos_cmd::run_passthrough(
                &args
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
                verbose,
            )?;
        }
    }

    Ok(())
}

fn dispatch_npx(args: &[String], cli: &Cli) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("npx requires a command argument");
    }

    // Intelligent routing: delegate to specialized filters
    match args[0].as_str() {
        "tsc" | "typescript" => {
            js::tsc::run(&args[1..], cli.verbose)?;
        }
        "eslint" => {
            lint_cmd::run(&args[1..], cli.verbose)?;
        }
        "prisma" => {
            // Route to js::prisma based on subcommand
            if args.len() > 1 {
                let prisma_args: Vec<String> = args[2..].to_vec();
                match args[1].as_str() {
                    "generate" => {
                        js::prisma::run(
                            js::prisma::PrismaCommand::Generate,
                            &prisma_args,
                            cli.verbose,
                        )?;
                    }
                    "db" if args.len() > 2 && args[2] == "push" => {
                        js::prisma::run(
                            js::prisma::PrismaCommand::DbPush,
                            &args[3..],
                            cli.verbose,
                        )?;
                    }
                    _ => {
                        // Passthrough other prisma subcommands
                        let timer = tracking::TimedExecution::start();
                        let mut cmd = std::process::Command::new("npx");
                        for arg in args {
                            cmd.arg(arg);
                        }
                        let status = cmd.status().context("Failed to run npx prisma")?;
                        let args_str = args.join(" ");
                        timer.track_passthrough(
                            &format!("npx {}", args_str),
                            &format!("mycelium npx {} (passthrough)", args_str),
                        );
                        if !status.success() {
                            std::process::exit(status.code().unwrap_or(1));
                        }
                    }
                }
            } else {
                let timer = tracking::TimedExecution::start();
                let status = std::process::Command::new("npx")
                    .arg("prisma")
                    .status()
                    .context("Failed to run npx prisma")?;
                timer.track_passthrough("npx prisma", "mycelium npx prisma (passthrough)");
                if !status.success() {
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
        }
        "next" => {
            js::next::run(&args[1..], cli.verbose)?;
        }
        "prettier" => {
            js::prettier::run(&args[1..], cli.verbose)?;
        }
        "playwright" => {
            js::playwright::run(&args[1..], cli.verbose)?;
        }
        _ => {
            // Generic passthrough with npm boilerplate filter
            js::npm::run(args, cli.verbose, cli.skip_env)?;
        }
    }

    Ok(())
}

fn dispatch_proxy(args: &[std::ffi::OsString], cli: &Cli) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!(
            "proxy requires a command to execute\nUsage: mycelium proxy <command> [args...]"
        );
    }

    let timer = tracking::TimedExecution::start();

    let cmd_name = args[0].to_string_lossy();
    let cmd_args: Vec<String> = args[1..]
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();

    if cli.verbose > 0 {
        eprintln!("Proxy mode: {} {}", cmd_name, cmd_args.join(" "));
    }

    let mut command = std::process::Command::new(cmd_name.as_ref());
    command.args(&cmd_args);

    run_spawned_command(
        command,
        &format!("{} {}", cmd_name, cmd_args.join(" ")),
        &format!("mycelium proxy {} {}", cmd_name, cmd_args.join(" ")),
        timer,
    )
}

fn dispatch_invoke_command(command: &[String], explain: bool, cli: &Cli) -> Result<()> {
    let rendered_command = crate::platform::render_shell_command(command);
    let resolution = rewrite_cmd::resolve_runtime_command(&rendered_command);

    if explain {
        println!("Mycelium invoke");
        println!("Input: {}", rendered_command.trim());
        println!("Execute: {}", resolution.command);
        println!(
            "Mode: {}",
            if resolution.rewritten {
                "rewritten through Mycelium"
            } else {
                "raw shell command"
            }
        );
        if let Some(estimated_savings_pct) = resolution.estimated_savings_pct {
            println!("Estimated savings: {:.1}%", estimated_savings_pct);
        }
        println!("Reason: {}", resolution.reason);
        return Ok(());
    }

    if cli.verbose > 0 {
        eprintln!("Invoke mode: {}", resolution.command);
    }

    let timer = tracking::TimedExecution::start();
    let mut child_command = crate::platform::invoke_shell_command(&resolution.command);
    if cli.skip_env {
        child_command.env("SKIP_ENV_VALIDATION", "1");
    }

    run_spawned_command(
        child_command,
        &rendered_command,
        &format!("mycelium invoke {}", resolution.command),
        timer,
    )
}

fn run_spawned_command(
    mut command: std::process::Command,
    tracked_input: &str,
    tracked_output: &str,
    timer: tracking::TimedExecution,
) -> Result<()> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::thread;

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute command")?;

    let stdout_pipe = child
        .stdout
        .take()
        .context("Failed to capture child stdout")?;
    let stderr_pipe = child
        .stderr
        .take()
        .context("Failed to capture child stderr")?;

    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stdout_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            captured.extend_from_slice(&buf[..count]);
            let mut out = std::io::stdout().lock();
            out.write_all(&buf[..count])?;
            out.flush()?;
        }

        Ok(captured)
    });

    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stderr_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            captured.extend_from_slice(&buf[..count]);
            let mut err = std::io::stderr().lock();
            err.write_all(&buf[..count])?;
            err.flush()?;
        }

        Ok(captured)
    });

    let status = child.wait().context("Failed waiting for command")?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;

    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let full_output = format!("{}{}", stdout, stderr);

    timer.track(tracked_input, tracked_output, &full_output, &full_output);

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Returns true for commands that are invoked via the hook pipeline
/// (i.e., commands that process rewritten shell commands).
/// Meta commands (init, gain, verify, etc.) are excluded because
/// they are run directly by the user, not through the hook.
///
/// SECURITY: whitelist pattern — new commands are NOT integrity-checked
/// until explicitly added here. A forgotten command fails open (no check)
/// rather than creating false confidence about what's protected.
pub fn is_operational_command(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Ls { .. }
            | Commands::Tree { .. }
            | Commands::Read { .. }
            | Commands::Peek { .. }
            | Commands::Git { .. }
            | Commands::Gh { .. }
            | Commands::Pnpm { .. }
            | Commands::Err { .. }
            | Commands::Test { .. }
            | Commands::Json { .. }
            | Commands::Deps { .. }
            | Commands::Env { .. }
            | Commands::Find { .. }
            | Commands::Diff { .. }
            | Commands::Log { .. }
            | Commands::Docker { .. }
            | Commands::Kubectl { .. }
            | Commands::Summary { .. }
            | Commands::Grep { .. }
            | Commands::Wget { .. }
            | Commands::Vitest { .. }
            | Commands::Prisma { .. }
            | Commands::Tsc { .. }
            | Commands::Next { .. }
            | Commands::Lint { .. }
            | Commands::Prettier { .. }
            | Commands::Playwright { .. }
            | Commands::Cargo { .. }
            | Commands::Npm { .. }
            | Commands::Npx { .. }
            | Commands::Curl { .. }
            | Commands::Ruff { .. }
            | Commands::Pytest { .. }
            | Commands::Pip { .. }
            | Commands::Go { .. }
            | Commands::GolangciLint { .. }
            | Commands::Gt { .. }
            | Commands::Invoke { .. }
    )
}

/// Re-invoke `mycelium` without `--json`, capture stdout, and wrap output in a JSON envelope.
fn dispatch_json(cli: Cli) -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).filter(|a| a != "--json").collect();

    let original_cmd = args.join(" ");
    let rewrite_resolution = rewrite_cmd::resolve_runtime_command(&original_cmd);
    let project_path = std::env::current_dir()
        .ok()
        .and_then(|path| path.canonicalize().ok().or(Some(path)))
        .map(|path| path.to_string_lossy().to_string());

    // Capture the raw command output (first arg is the subcommand name as Mycelium sees it).
    // We need to know the underlying command name to get the "raw" token count.
    // Run the first token as a plain command to get unfiltered output.
    let raw_output = if !args.is_empty() {
        let raw_result = std::process::Command::new(&args[0])
            .args(&args[1..])
            .output();
        match raw_result {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(_) => String::new(),
        }
    } else {
        String::new()
    };

    // Run through mycelium (self) without --json to get filtered output.
    let mycelium_exe = std::env::current_exe().context("Failed to locate mycelium executable")?;
    let filtered_result = std::process::Command::new(&mycelium_exe)
        .args(&args)
        .output();

    let envelope = match filtered_result {
        Ok(output) if output.status.success() || !output.stdout.is_empty() => {
            let filtered = String::from_utf8_lossy(&output.stdout).to_string();
            json_output::wrap_output(
                &original_cmd,
                &format!("mycelium {original_cmd}"),
                &filtered,
                &raw_output,
                project_path.as_deref(),
                Some(&rewrite_resolution),
            )
        }
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            json_output::wrap_error(&stderr, exit_code)
        }
        Err(e) => json_output::wrap_error(&e.to_string(), 1),
    };

    // Suppress the unused-variable warning; cli is consumed by the json branch.
    let _ = cli;

    println!("{envelope}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Helper: parse CLI args and return the Commands variant.
    fn parse_command(args: &[&str]) -> Commands {
        let mut full_args = vec!["mycelium"];
        full_args.extend_from_slice(args);
        Cli::try_parse_from(full_args).unwrap().command
    }

    // --- is_operational_command: operational commands return true ---

    #[test]
    fn test_git_status_is_operational() {
        let cmd = parse_command(&["git", "status"]);
        assert!(is_operational_command(&cmd));
    }

    #[test]
    fn test_cargo_build_is_operational() {
        let cmd = parse_command(&["cargo", "build"]);
        assert!(is_operational_command(&cmd));
    }

    #[test]
    fn test_grep_is_operational() {
        let cmd = parse_command(&["grep", "pattern"]);
        assert!(is_operational_command(&cmd));
    }

    #[test]
    fn test_ls_is_operational() {
        let cmd = parse_command(&["ls"]);
        assert!(is_operational_command(&cmd));
    }

    #[test]
    fn test_go_test_is_operational() {
        let cmd = parse_command(&["go", "test"]);
        assert!(is_operational_command(&cmd));
    }

    // --- is_operational_command: meta commands return false ---

    #[test]
    fn test_gain_is_not_operational() {
        let cmd = parse_command(&["gain"]);
        assert!(!is_operational_command(&cmd));
    }

    #[test]
    fn test_init_is_not_operational() {
        let cmd = parse_command(&["init"]);
        assert!(!is_operational_command(&cmd));
    }

    #[test]
    fn test_config_is_not_operational() {
        let cmd = parse_command(&["config"]);
        assert!(!is_operational_command(&cmd));
    }

    #[test]
    fn test_invoke_is_operational() {
        let cmd = parse_command(&["invoke", "git", "status"]);
        assert!(is_operational_command(&cmd));
    }

    // --- MYCELIUM_META_COMMANDS constant covers the expected set ---

    #[test]
    fn test_meta_commands_list_includes_gain_and_init() {
        assert!(MYCELIUM_META_COMMANDS.contains(&"gain"));
        assert!(MYCELIUM_META_COMMANDS.contains(&"init"));
        assert!(MYCELIUM_META_COMMANDS.contains(&"config"));
        assert!(MYCELIUM_META_COMMANDS.contains(&"discover"));
        assert!(MYCELIUM_META_COMMANDS.contains(&"proxy"));
        assert!(MYCELIUM_META_COMMANDS.contains(&"invoke"));
        // Operational commands should NOT be in the meta list
        assert!(!MYCELIUM_META_COMMANDS.contains(&"git"));
        assert!(!MYCELIUM_META_COMMANDS.contains(&"cargo"));
        assert!(!MYCELIUM_META_COMMANDS.contains(&"ls"));
    }

    // --- run_fallback: unrecognized commands trigger fallback ---
    #[test]
    fn test_unrecognized_command_fails_parse() {
        let result = Cli::try_parse_from(["mycelium", "nonexistent-command"]);
        assert!(result.is_err(), "Unknown command should fail Clap parsing");
    }
}
