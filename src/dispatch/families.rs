use anyhow::{Context, Result};

use crate::commands::*;
use crate::{
    atmos_cmd, cargo_cmd, container_cmd, gh_cmd, git, go_eco, gt_cmd, init, js, lint_cmd, python,
    terraform_cmd, tracking,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_git_commands(
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

pub(super) fn dispatch_gh_commands(
    command: GhCommands,
    verbose: u8,
    ultra_compact: bool,
) -> Result<()> {
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

pub(super) fn dispatch_pnpm_commands(command: PnpmCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_prisma_commands(command: PrismaCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_cargo_commands(command: CargoCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_docker_commands(command: DockerCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_kubectl_commands(command: KubectlCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_ruff_commands(command: RuffCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_pip_commands(command: PipCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_go_commands(command: GoCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_gt_commands(command: GtCommands, verbose: u8) -> Result<()> {
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

#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_init_commands(
    global: bool,
    show: bool,
    onboard: bool,
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
    } else if onboard {
        init::onboard(global, verbose)?;
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

pub(super) fn dispatch_terraform_commands(command: TerraformCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_atmos_commands(command: AtmosCommands, verbose: u8) -> Result<()> {
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

pub(super) fn dispatch_npx(args: &[String], cli: &Cli) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("npx requires a command argument");
    }

    match args[0].as_str() {
        "tsc" | "typescript" => {
            js::tsc::run(&args[1..], cli.verbose)?;
        }
        "eslint" => {
            lint_cmd::run(&args[1..], cli.verbose)?;
        }
        "prisma" => {
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
            js::npm::run(args, cli.verbose, cli.skip_env)?;
        }
    }

    Ok(())
}
