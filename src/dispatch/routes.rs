use anyhow::Result;
use std::path::Path;

use crate::commands::*;
use crate::{
    aws_cmd, cc_economics, completions_cmd, config, curl_cmd, deps, diff_cmd, discover, env_cmd,
    find_cmd, gain, go_eco, grep_cmd, hook_audit_cmd, init, integrity, js, json_cmd, learn,
    local_llm, log_cmd, ls_cmd, parse_health_cmd, psql_cmd, python, read_cmd, rewrite_cmd,
    runner_cmd, self_update_cmd, summary_cmd, tree_cmd, wc_cmd, wget_cmd,
};

use super::exec::{dispatch_invoke_command, dispatch_proxy};
use super::families::{
    dispatch_atmos_commands, dispatch_cargo_commands, dispatch_docker_commands,
    dispatch_gh_commands, dispatch_git_commands, dispatch_go_commands, dispatch_gt_commands,
    dispatch_init_commands, dispatch_kubectl_commands, dispatch_npx, dispatch_pip_commands,
    dispatch_pnpm_commands, dispatch_prisma_commands, dispatch_ruff_commands,
    dispatch_terraform_commands,
};

pub(super) fn dispatch_command(cli: Cli) -> Result<()> {
    match cli.command {
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
        Commands::Gh { command } => {
            dispatch_gh_commands(command, cli.verbose, cli.ultra_compact)?;
        }
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
            crate::lint_cmd::run(&args, cli.verbose)?;
        }
        Commands::Format { args } => {
            crate::format_cmd::run(&args, cli.verbose)?;
        }
        Commands::Prettier { args } => {
            js::prettier::run(&args, cli.verbose)?;
        }
        Commands::Ruff { command } => {
            dispatch_ruff_commands(command, cli.verbose)?;
        }
        Commands::Mypy { args } => {
            python::mypy::run(&args, cli.verbose)?;
        }
        Commands::GolangciLint { args } => {
            go_eco::golangci::run(&args, cli.verbose)?;
        }
        Commands::Test { command } => {
            let cmd = command.join(" ");
            runner_cmd::run_test(&cmd, cli.verbose)?;
        }
        Commands::Playwright { args } => {
            js::playwright::run(&args, cli.verbose)?;
        }
        Commands::Pytest { args } => {
            python::pytest::run(&args, cli.verbose)?;
        }
        Commands::Pip { command } => {
            dispatch_pip_commands(command, cli.verbose)?;
        }
        Commands::Npm { args } => {
            js::npm::run(&args, cli.verbose, cli.skip_env)?;
        }
        Commands::Npx { ref args } => {
            dispatch_npx(args, &cli)?;
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
        Commands::Docker { command } => {
            dispatch_docker_commands(command, cli.verbose)?;
        }
        Commands::Kubectl { command } => {
            dispatch_kubectl_commands(command, cli.verbose)?;
        }
        Commands::Terraform { command } => {
            dispatch_terraform_commands(command, cli.verbose)?;
        }
        Commands::Go { command } => {
            dispatch_go_commands(command, cli.verbose)?;
        }
        Commands::Gt { command } => {
            dispatch_gt_commands(command, cli.verbose)?;
        }
        Commands::Cargo { command } => {
            dispatch_cargo_commands(command, cli.verbose)?;
        }
        Commands::Err { command } => {
            let cmd = command.join(" ");
            runner_cmd::run_err(&cmd, cli.verbose)?;
        }
        Commands::Json { file, depth } => {
            if file == Path::new("-") {
                json_cmd::run_stdin(depth, cli.verbose)?;
            } else {
                json_cmd::run(&file, depth, cli.verbose)?;
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
        Commands::Env { filter, show_all } => {
            env_cmd::run(filter.as_deref(), show_all, cli.verbose)?;
        }
        Commands::Deps { path } => {
            deps::run(&path, cli.verbose)?;
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
        Commands::Grep {
            pattern,
            path,
            max_len,
            max,
            context_only,
            file_type,
            line_numbers: _,
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
        Commands::Wc { args } => {
            wc_cmd::run(&args, cli.verbose)?;
        }
        Commands::Init {
            global,
            show,
            onboard,
            claude_md,
            hook_only,
            auto_patch,
            no_patch,
            uninstall,
        } => {
            dispatch_init_commands(
                global,
                show,
                onboard,
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
                project.as_deref(),
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
        Commands::Config { create } => {
            if create {
                let path = config::Config::create_default()?;
                println!("Created: {}", path.display());
            } else {
                config::show_config()?;
            }
        }
        Commands::Doctor => {
            crate::doctor_cmd::run()?;
        }
        Commands::Verify => {
            integrity::run_verify(cli.verbose)?;
        }
        Commands::SelfUpdate { check } => {
            self_update_cmd::run(check)?;
        }
        Commands::Completions { shell } => {
            completions_cmd::run(&shell)?;
        }
        Commands::Proxy { ref args } => {
            dispatch_proxy(args, &cli)?;
        }
        Commands::Invoke {
            ref command,
            explain,
        } => {
            dispatch_invoke_command(command, explain, &cli)?;
        }
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
        Commands::HookAudit { since } => {
            hook_audit_cmd::run(since, cli.verbose)?;
        }
        Commands::Rewrite { cmd, explain } => {
            rewrite_cmd::run(&cmd, explain)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse_command(args: &[&str]) -> Commands {
        let mut full_args = vec!["mycelium"];
        full_args.extend_from_slice(args);
        Cli::try_parse_from(full_args).unwrap().command
    }

    #[test]
    fn test_git_status_is_operational() {
        let cmd = parse_command(&["git", "status"]);
        assert!(super::super::exec::is_operational_command(&cmd));
    }

    #[test]
    fn test_init_is_not_operational() {
        let cmd = parse_command(&["init"]);
        assert!(!super::super::exec::is_operational_command(&cmd));
    }
}
