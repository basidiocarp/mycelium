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
        Commands::Ls(ls) => {
            ls_cmd::run(&ls.args, cli.verbose)?;
        }
        Commands::Tree(tree) => {
            tree_cmd::run(&tree.args, cli.verbose)?;
        }
        Commands::Read(r) => {
            if r.file == Path::new("-") {
                read_cmd::run_stdin(r.level, r.max_lines, r.line_numbers, cli.verbose)?;
            } else {
                read_cmd::run(&r.file, r.level, r.max_lines, r.line_numbers, cli.verbose)?;
            }
        }
        Commands::Peek(peek) => {
            local_llm::run(&peek.file, &peek.model, peek.force_download, cli.verbose)?;
        }
        Commands::Git(git) => {
            dispatch_git_commands(
                git.directory,
                git.config_override,
                git.git_dir,
                git.work_tree,
                git.no_pager,
                git.no_optional_locks,
                git.bare,
                git.literal_pathspecs,
                git.command,
                cli.verbose,
            )?;
        }
        Commands::Gh(gh) => {
            dispatch_gh_commands(gh.command, cli.verbose, cli.ultra_compact)?;
        }
        Commands::Aws(aws) => match aws.command {
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
        Commands::Atmos(atmos) => {
            dispatch_atmos_commands(atmos.command, cli.verbose)?;
        }
        Commands::Psql(psql) => {
            psql_cmd::run(&psql.args, cli.verbose)?;
        }
        Commands::Pnpm(pnpm) => {
            dispatch_pnpm_commands(pnpm.command, cli.verbose)?;
        }
        Commands::Vitest(vitest) => match vitest.command {
            VitestCommands::Run { args } => {
                js::vitest::run(js::vitest::VitestCommand::Run, &args, cli.verbose)?;
            }
        },
        Commands::Prisma(prisma) => {
            dispatch_prisma_commands(prisma.command, cli.verbose)?;
        }
        Commands::Tsc(tsc) => {
            js::tsc::run(&tsc.args, cli.verbose)?;
        }
        Commands::Next(next) => {
            js::next::run(&next.args, cli.verbose)?;
        }
        Commands::Lint(lint) => {
            crate::lint_cmd::run(&lint.args, cli.verbose)?;
        }
        Commands::Format(fmt) => {
            crate::format_cmd::run(&fmt.args, cli.verbose)?;
        }
        Commands::Prettier(prettier) => {
            js::prettier::run(&prettier.args, cli.verbose)?;
        }
        Commands::Ruff(ruff) => {
            dispatch_ruff_commands(ruff.command, cli.verbose)?;
        }
        Commands::Mypy(mypy) => {
            python::mypy::run(&mypy.args, cli.verbose)?;
        }
        Commands::GolangciLint(golangci) => {
            go_eco::golangci::run(&golangci.args, cli.verbose)?;
        }
        Commands::Test(test) => {
            let cmd = test.command.join(" ");
            runner_cmd::run_test(&cmd, cli.verbose)?;
        }
        Commands::Playwright(playwright) => {
            js::playwright::run(&playwright.args, cli.verbose)?;
        }
        Commands::Pytest(pytest) => {
            python::pytest::run(&pytest.args, cli.verbose)?;
        }
        Commands::Pip(pip) => {
            dispatch_pip_commands(pip.command, cli.verbose)?;
        }
        Commands::Npm(npm) => {
            js::npm::run(&npm.args, cli.verbose, cli.skip_env)?;
        }
        Commands::Npx(ref npx) => {
            dispatch_npx(&npx.args, &cli)?;
        }
        Commands::Curl(curl) => {
            curl_cmd::run(&curl.args, cli.verbose)?;
        }
        Commands::Wget(wget) => {
            if wget.stdout {
                wget_cmd::run_stdout(&wget.url, &wget.args, cli.verbose)?;
            } else {
                wget_cmd::run(&wget.url, &wget.args, cli.verbose)?;
            }
        }
        Commands::Docker(docker) => {
            dispatch_docker_commands(docker.command, cli.verbose)?;
        }
        Commands::Kubectl(kubectl) => {
            dispatch_kubectl_commands(kubectl.command, cli.verbose)?;
        }
        Commands::Terraform(terraform) => {
            dispatch_terraform_commands(terraform.command, cli.verbose)?;
        }
        Commands::Go(go) => {
            dispatch_go_commands(go.command, cli.verbose)?;
        }
        Commands::Gt(gt) => {
            dispatch_gt_commands(gt.command, cli.verbose)?;
        }
        Commands::Cargo(cargo) => {
            dispatch_cargo_commands(cargo.command, cli.verbose)?;
        }
        Commands::Err(err) => {
            let cmd = err.command.join(" ");
            runner_cmd::run_err(&cmd, cli.verbose)?;
        }
        Commands::Json(json) => {
            if json.file == Path::new("-") {
                json_cmd::run_stdin(json.depth, cli.verbose)?;
            } else {
                json_cmd::run(&json.file, json.depth, cli.verbose)?;
            }
        }
        Commands::Log(log) => {
            if let Some(f) = log.file {
                log_cmd::run_file(&f, cli.verbose)?;
            } else {
                log_cmd::run_stdin(cli.verbose)?;
            }
        }
        Commands::Summary(summary) => {
            let cmd = summary.command.join(" ");
            summary_cmd::run(&cmd, cli.verbose)?;
        }
        Commands::Env(env) => {
            env_cmd::run(env.filter.as_deref(), env.show_all, cli.verbose)?;
        }
        Commands::Deps(deps_cmd) => {
            deps::run(&deps_cmd.path, cli.verbose)?;
        }
        Commands::Find(find) => {
            find_cmd::run_from_args(&find.args, cli.verbose)?;
        }
        Commands::Diff(diff) => {
            if let Some(f2) = diff.file2 {
                diff_cmd::run(&diff.file1, &f2, cli.verbose)?;
            } else {
                diff_cmd::run_stdin(cli.verbose)?;
            }
        }
        Commands::Grep(grep) => {
            grep_cmd::run(
                &grep.pattern,
                &grep.path,
                grep.max_len,
                grep.max,
                grep.context_only,
                grep.file_type.as_deref(),
                &grep.extra_args,
                cli.verbose,
            )?;
        }
        Commands::Wc(wc) => {
            wc_cmd::run(&wc.args, cli.verbose)?;
        }
        Commands::Init(init_args) => {
            dispatch_init_commands(
                init_args.global,
                init_args.show,
                init_args.onboard,
                init_args.claude_md,
                init_args.hook_only,
                init_args.auto_patch,
                init_args.no_patch,
                init_args.uninstall,
                cli.verbose,
            )?;
        }
        Commands::Gain(gain_args) => {
            gain::run(
                gain_args.project.as_deref(),
                gain_args.project_path.as_deref(),
                gain_args.projects,
                gain_args.diagnostics,
                gain_args.explain,
                gain_args.graph,
                gain_args.history,
                gain_args.limit,
                gain_args.quota,
                &gain_args.tier,
                gain_args.daily,
                gain_args.weekly,
                gain_args.monthly,
                gain_args.all,
                &gain_args.format,
                gain_args.failures,
                gain_args.status,
                gain_args.compare.as_deref(),
                cli.verbose,
            )?;
        }
        Commands::Discover(discover_args) => {
            discover::run(discover_args.project.as_deref(), discover_args.all, discover_args.since, discover_args.limit, &discover_args.format, cli.verbose)?;
        }
        Commands::Learn(learn_args) => {
            learn::run(
                learn_args.project.as_deref(),
                learn_args.all,
                learn_args.since,
                &learn_args.format,
                learn_args.write_rules,
                learn_args.min_confidence,
                learn_args.min_occurrences,
            )?;
        }
        Commands::Context(context_args) => {
            let task_str = context_args.task.join(" ");
            init::context::run(
                &task_str,
                context_args.project.as_deref(),
                context_args.budget,
                context_args.include.as_deref(),
                cli.json,
            )?;
        }
        Commands::Config(config_args) => {
            if config_args.create {
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
        Commands::SelfUpdate(su) => {
            self_update_cmd::run(su.check)?;
        }
        Commands::Completions(completions) => {
            completions_cmd::run(&completions.shell)?;
        }
        Commands::Proxy(ref proxy) => {
            dispatch_proxy(&proxy.args, &cli)?;
        }
        Commands::Invoke(ref invoke) => {
            dispatch_invoke_command(&invoke.command, invoke.explain, &cli)?;
        }
        Commands::Benchmark(benchmark) => {
            crate::benchmark_cmd::run(benchmark.ci)?;
        }
        Commands::Plugin(plugin) => match plugin.command {
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
        Commands::ParseHealth(ph) => {
            parse_health_cmd::run(ph.days)?;
        }
        Commands::CcEconomics(cc) => {
            cc_economics::run(
                cc.project,
                cc.project_path.as_deref(),
                cc.daily,
                cc.weekly,
                cc.monthly,
                cc.all,
                &cc.format,
                cli.verbose,
            )?;
        }
        Commands::HookAudit(ha) => {
            hook_audit_cmd::run(ha.since, cli.verbose)?;
        }
        Commands::Rewrite(rewrite) => {
            rewrite_cmd::run(&rewrite.cmd, rewrite.explain)?;
        }
        Commands::Explain(explain) => {
            print!("{}", rewrite_cmd::explain(&explain.cmd));
        }
        // ServeSocket is handled early in dispatch::dispatch before routing reaches here.
        #[cfg(unix)]
        Commands::ServeSocket(_) => unreachable!("ServeSocket dispatched before routes"),
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
