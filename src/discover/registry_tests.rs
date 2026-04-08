use super::super::report::MyceliumStatus;
use super::*;

#[test]
fn test_classify_git_status() {
    assert_eq!(
        classify_command("git status"),
        Classification::Supported {
            mycelium_equivalent: "mycelium git",
            category: "Git",
            estimated_savings_pct: 70.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_git_diff_cached() {
    assert_eq!(
        classify_command("git diff --cached"),
        Classification::Supported {
            mycelium_equivalent: "mycelium git",
            category: "Git",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cargo_test_filter() {
    assert_eq!(
        classify_command("cargo test filter::"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 90.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_npx_tsc() {
    assert_eq!(
        classify_command("npx tsc --noEmit"),
        Classification::Supported {
            mycelium_equivalent: "mycelium tsc",
            category: "Build",
            estimated_savings_pct: 83.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cat_file() {
    assert_eq!(
        classify_command("cat src/main.rs"),
        Classification::Supported {
            mycelium_equivalent: "mycelium read",
            category: "Files",
            estimated_savings_pct: 60.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cd_ignored() {
    assert_eq!(classify_command("cd /tmp"), Classification::Ignored);
}

#[test]
fn test_classify_mycelium_already() {
    assert_eq!(
        classify_command("mycelium git status"),
        Classification::Ignored
    );
}

#[test]
fn test_classify_echo_ignored() {
    assert_eq!(
        classify_command("echo hello world"),
        Classification::Ignored
    );
}

#[test]
fn test_rewrite_which_passthroughs_through_invoke() {
    assert_eq!(
        rewrite_command("which git", &[]),
        Some("mycelium invoke which git".into())
    );
}

#[test]
fn test_rewrite_type_passthroughs_through_invoke() {
    assert_eq!(
        rewrite_command("type cargo", &[]),
        Some("mycelium invoke type cargo".into())
    );
}

#[test]
fn test_rewrite_env_prefixed_diagnostic_passthrough() {
    assert_eq!(
        rewrite_command("FOO=1 echo hello", &[]),
        Some("FOO=1 mycelium invoke echo hello".into())
    );
}

#[test]
fn test_rewrite_ls_with_flags_passthroughs_through_invoke() {
    assert_eq!(
        rewrite_command("ls -la /tmp", &[]),
        Some("mycelium invoke ls -la /tmp".into())
    );
}

#[test]
fn test_rewrite_compound_diagnostic_and_supported_command() {
    assert_eq!(
        rewrite_command("which git && git status", &[]),
        Some("mycelium invoke which git && mycelium git status".into())
    );
}

#[test]
fn test_classify_terraform_supported() {
    match classify_command("terraform plan -var-file=prod.tfvars") {
        Classification::Supported {
            mycelium_equivalent,
            category,
            ..
        } => {
            assert_eq!(mycelium_equivalent, "mycelium terraform");
            assert_eq!(category, "Infra");
        }
        other => panic!("expected Supported, got {:?}", other),
    }
}

#[test]
fn test_classify_env_prefix_stripped() {
    assert_eq!(
        classify_command("GIT_SSH_COMMAND=ssh git push"),
        Classification::Supported {
            mycelium_equivalent: "mycelium git",
            category: "Git",
            estimated_savings_pct: 70.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_sudo_stripped() {
    assert_eq!(
        classify_command("sudo docker ps"),
        Classification::Supported {
            mycelium_equivalent: "mycelium docker",
            category: "Infra",
            estimated_savings_pct: 85.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cargo_check() {
    assert_eq!(
        classify_command("cargo check"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cargo_check_all_targets() {
    assert_eq!(
        classify_command("cargo check --all-targets"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_cargo_fmt_passthrough() {
    assert_eq!(
        classify_command("cargo fmt"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Passthrough,
        }
    );
}

#[test]
fn test_classify_cargo_clippy_savings() {
    assert_eq!(
        classify_command("cargo clippy --all-targets"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_patterns_rules_length_match() {
    assert_eq!(
        PATTERNS.len(),
        RULES.len(),
        "PATTERNS and RULES must be aligned"
    );
}

#[test]
fn test_registry_covers_all_cargo_subcommands() {
    // Verify that every CargoCommand variant (Build, Test, Clippy, Check, Fmt)
    // except Other has a matching pattern in the registry
    for subcmd in ["build", "test", "clippy", "check", "fmt"] {
        let cmd = format!("cargo {subcmd}");
        match classify_command(&cmd) {
            Classification::Supported { .. } => {}
            other => panic!("cargo {subcmd} should be Supported, got {other:?}"),
        }
    }
}

#[test]
fn test_registry_covers_all_git_subcommands() {
    // Verify that every GitCommand subcommand has a matching pattern
    for subcmd in [
        "status", "log", "diff", "show", "add", "commit", "push", "pull", "branch", "fetch",
        "stash", "worktree",
    ] {
        let cmd = format!("git {subcmd}");
        match classify_command(&cmd) {
            Classification::Supported { .. } => {}
            other => panic!("git {subcmd} should be Supported, got {other:?}"),
        }
    }
}

#[test]
fn test_classify_find_not_blocked_by_fi() {
    // Regression: "fi" in IGNORED_PREFIXES used to shadow "find" commands
    // because "find".starts_with("fi") is true. "fi" should only match exactly.
    assert_eq!(
        classify_command("find . -name foo"),
        Classification::Supported {
            mycelium_equivalent: "mycelium find",
            category: "Files",
            estimated_savings_pct: 70.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_fi_still_ignored_exact() {
    // Bare "fi" (shell keyword) should still be ignored
    assert_eq!(classify_command("fi"), Classification::Ignored);
}

#[test]
fn test_done_still_ignored_exact() {
    // Bare "done" (shell keyword) should still be ignored
    assert_eq!(classify_command("done"), Classification::Ignored);
}

#[test]
fn test_split_chain_and() {
    assert_eq!(split_command_chain("a && b"), vec!["a", "b"]);
}

#[test]
fn test_split_chain_semicolon() {
    assert_eq!(split_command_chain("a ; b"), vec!["a", "b"]);
}

#[test]
fn test_split_pipe_first_only() {
    assert_eq!(split_command_chain("a | b"), vec!["a | b"]);
}

#[test]
fn test_split_single() {
    assert_eq!(split_command_chain("git status"), vec!["git status"]);
}

#[test]
fn test_split_quoted_and() {
    assert_eq!(
        split_command_chain(r#"echo "a && b""#),
        vec![r#"echo "a && b""#]
    );
}

#[test]
fn test_split_heredoc_no_split() {
    let cmd = "cat <<'EOF'\nhello && world\nEOF";
    assert_eq!(split_command_chain(cmd), vec![cmd]);
}

#[test]
fn test_classify_mypy() {
    assert_eq!(
        classify_command("mypy src/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium mypy",
            category: "Build",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_classify_python_m_mypy() {
    assert_eq!(
        classify_command("python3 -m mypy --strict"),
        Classification::Supported {
            mycelium_equivalent: "mycelium mypy",
            category: "Build",
            estimated_savings_pct: 80.0,
            status: MyceliumStatus::Existing,
        }
    );
}

// --- rewrite_command tests ---

#[test]
fn test_rewrite_git_status() {
    assert_eq!(
        rewrite_command("git status", &[]),
        Some("mycelium git status".into())
    );
}

#[test]
fn test_rewrite_git_log() {
    assert_eq!(
        rewrite_command("git log -10", &[]),
        Some("mycelium git log -10".into())
    );
}

#[test]
fn test_rewrite_cargo_test() {
    assert_eq!(
        rewrite_command("cargo test", &[]),
        Some("mycelium cargo test".into())
    );
}

#[test]
fn test_rewrite_compound_and() {
    assert_eq!(
        rewrite_command("git add . && cargo test", &[]),
        Some("mycelium git add . && mycelium cargo test".into())
    );
}

#[test]
fn test_rewrite_compound_three_segments() {
    assert_eq!(
            rewrite_command(
                "cargo fmt --all && cargo clippy --all-targets && cargo test",
                &[]
            ),
            Some("mycelium cargo fmt --all && mycelium cargo clippy --all-targets && mycelium cargo test".into())
        );
}

#[test]
fn test_rewrite_already_mycelium() {
    assert_eq!(
        rewrite_command("mycelium git status", &[]),
        Some("mycelium git status".into())
    );
}

#[test]
fn test_rewrite_background_single_amp() {
    assert_eq!(
        rewrite_command("cargo test & git status", &[]),
        Some("mycelium cargo test & mycelium git status".into())
    );
}

#[test]
fn test_rewrite_background_unsupported_right() {
    assert_eq!(
        rewrite_command("cargo test & ansible-playbook site.yml", &[]),
        Some("mycelium cargo test & ansible-playbook site.yml".into())
    );
}

#[test]
fn test_rewrite_background_does_not_affect_double_amp() {
    // `&&` must still work after adding `&` support
    assert_eq!(
        rewrite_command("cargo test && git status", &[]),
        Some("mycelium cargo test && mycelium git status".into())
    );
}

#[test]
fn test_rewrite_unsupported_returns_none() {
    assert_eq!(rewrite_command("ansible-playbook site.yml", &[]), None);
}

#[test]
fn test_rewrite_ignored_cd() {
    assert_eq!(rewrite_command("cd /tmp", &[]), None);
}

#[test]
fn test_rewrite_with_env_prefix() {
    assert_eq!(
        rewrite_command("GIT_SSH_COMMAND=ssh git push", &[]),
        Some("GIT_SSH_COMMAND=ssh mycelium git push".into())
    );
}

#[test]
fn test_rewrite_npx_tsc() {
    assert_eq!(
        rewrite_command("npx tsc --noEmit", &[]),
        Some("mycelium tsc --noEmit".into())
    );
}

#[test]
fn test_rewrite_pnpm_tsc() {
    assert_eq!(
        rewrite_command("pnpm tsc --noEmit", &[]),
        Some("mycelium tsc --noEmit".into())
    );
}

#[test]
fn test_rewrite_cat_file() {
    assert_eq!(
        rewrite_command("cat src/main.rs", &[]),
        Some("mycelium read src/main.rs".into())
    );
}

#[test]
fn test_rewrite_rg_pattern() {
    assert_eq!(
        rewrite_command("rg \"fn main\"", &[]),
        Some("mycelium grep \"fn main\"".into())
    );
}

#[test]
fn test_rewrite_npx_playwright() {
    assert_eq!(
        rewrite_command("npx playwright test", &[]),
        Some("mycelium playwright test".into())
    );
}

#[test]
fn test_rewrite_next_build() {
    assert_eq!(
        rewrite_command("next build --turbo", &[]),
        Some("mycelium next --turbo".into())
    );
}

#[test]
fn test_rewrite_pipe_first_only() {
    // Piped commands stay raw so downstream stages receive native stdout.
    assert_eq!(rewrite_command("git log -10 | grep feat", &[]), None);
}

#[test]
fn test_rewrite_heredoc_returns_none() {
    assert_eq!(rewrite_command("cat <<'EOF'\nfoo\nEOF", &[]), None);
}

#[test]
fn test_rewrite_empty_returns_none() {
    assert_eq!(rewrite_command("", &[]), None);
    assert_eq!(rewrite_command("   ", &[]), None);
}

#[test]
fn test_rewrite_mixed_compound_partial() {
    // First segment already Mycelium, second gets rewritten
    assert_eq!(
        rewrite_command("mycelium git add . && cargo test", &[]),
        Some("mycelium git add . && mycelium cargo test".into())
    );
}

// --- #345: MYCELIUM_DISABLED ---

#[test]
fn test_rewrite_mycelium_disabled_curl() {
    assert_eq!(
        rewrite_command("MYCELIUM_DISABLED=1 curl https://example.com", &[]),
        None
    );
}

#[test]
fn test_rewrite_mycelium_disabled_git_status() {
    assert_eq!(rewrite_command("MYCELIUM_DISABLED=1 git status", &[]), None);
}

#[test]
fn test_rewrite_mycelium_disabled_multi_env() {
    assert_eq!(
        rewrite_command("FOO=1 MYCELIUM_DISABLED=1 git status", &[]),
        None
    );
}

#[test]
fn test_rewrite_non_mycelium_disabled_env_still_rewrites() {
    assert_eq!(
        rewrite_command("SOME_VAR=1 git status", &[]),
        Some("SOME_VAR=1 mycelium git status".into())
    );
}

// --- #346: 2>&1 and &> redirect detection ---

#[test]
fn test_rewrite_redirect_2_gt_amp_1_with_pipe() {
    assert_eq!(rewrite_command("cargo test 2>&1 | head", &[]), None);
}

#[test]
fn test_rewrite_redirect_2_gt_amp_1_trailing() {
    assert_eq!(rewrite_command("cargo test 2>&1", &[]), None);
}

#[test]
fn test_rewrite_redirect_plain_2_devnull() {
    assert_eq!(rewrite_command("git status 2>/dev/null", &[]), None);
}

#[test]
fn test_rewrite_redirect_2_gt_amp_1_with_and() {
    assert_eq!(rewrite_command("cargo test 2>&1 && echo done", &[]), None);
}

#[test]
fn test_rewrite_redirect_amp_gt_devnull() {
    assert_eq!(rewrite_command("cargo test &>/dev/null", &[]), None);
}

#[test]
fn test_rewrite_background_amp_non_regression() {
    // background `&` must still work after redirect fix
    assert_eq!(
        rewrite_command("cargo test & git status", &[]),
        Some("mycelium cargo test & mycelium git status".into())
    );
}

// --- P0.2: head -N rewrite ---

#[test]
fn test_rewrite_head_numeric_flag() {
    // head -20 file → mycelium read file --max-lines 20 (not mycelium read -20 file)
    assert_eq!(
        rewrite_command("head -20 src/main.rs", &[]),
        Some("mycelium read src/main.rs --max-lines 20".into())
    );
}

#[test]
fn test_rewrite_head_lines_long_flag() {
    assert_eq!(
        rewrite_command("head --lines=50 src/lib.rs", &[]),
        Some("mycelium read src/lib.rs --max-lines 50".into())
    );
}

#[test]
fn test_rewrite_head_no_flag_still_rewrites() {
    // plain `head file` → `mycelium read file` (no numeric flag)
    assert_eq!(
        rewrite_command("head src/main.rs", &[]),
        Some("mycelium read src/main.rs".into())
    );
}

#[test]
fn test_rewrite_head_other_flag_skipped() {
    // head -c 100 file: unsupported flag, skip rewriting
    assert_eq!(rewrite_command("head -c 100 src/main.rs", &[]), None);
}

// --- New registry entries ---

#[test]
fn test_classify_gh_release() {
    assert!(matches!(
        classify_command("gh release list"),
        Classification::Supported {
            mycelium_equivalent: "mycelium gh",
            ..
        }
    ));
}

#[test]
fn test_classify_cargo_install() {
    assert!(matches!(
        classify_command("cargo install mycelium"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            ..
        }
    ));
}

#[test]
fn test_classify_docker_run() {
    assert!(matches!(
        classify_command("docker run --rm ubuntu bash"),
        Classification::Supported {
            mycelium_equivalent: "mycelium docker",
            ..
        }
    ));
}

#[test]
fn test_classify_docker_exec() {
    assert!(matches!(
        classify_command("docker exec -it mycontainer bash"),
        Classification::Supported {
            mycelium_equivalent: "mycelium docker",
            ..
        }
    ));
}

#[test]
fn test_classify_docker_build() {
    assert!(matches!(
        classify_command("docker build -t myimage ."),
        Classification::Supported {
            mycelium_equivalent: "mycelium docker",
            ..
        }
    ));
}

#[test]
fn test_classify_kubectl_describe() {
    assert!(matches!(
        classify_command("kubectl describe pod mypod"),
        Classification::Supported {
            mycelium_equivalent: "mycelium kubectl",
            ..
        }
    ));
}

#[test]
fn test_classify_kubectl_apply() {
    assert!(matches!(
        classify_command("kubectl apply -f deploy.yaml"),
        Classification::Supported {
            mycelium_equivalent: "mycelium kubectl",
            ..
        }
    ));
}

#[test]
fn test_classify_tree() {
    assert!(matches!(
        classify_command("tree src/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium tree",
            ..
        }
    ));
}

#[test]
fn test_classify_diff() {
    assert!(matches!(
        classify_command("diff file1.txt file2.txt"),
        Classification::Supported {
            mycelium_equivalent: "mycelium diff",
            ..
        }
    ));
}

#[test]
fn test_rewrite_tree() {
    assert_eq!(
        rewrite_command("tree src/", &[]),
        Some("mycelium tree src/".into())
    );
}

#[test]
fn test_rewrite_diff() {
    assert_eq!(
        rewrite_command("diff file1.txt file2.txt", &[]),
        Some("mycelium diff file1.txt file2.txt".into())
    );
}

#[test]
fn test_rewrite_gh_release() {
    assert_eq!(
        rewrite_command("gh release list", &[]),
        Some("mycelium gh release list".into())
    );
}

#[test]
fn test_rewrite_cargo_install() {
    assert_eq!(
        rewrite_command("cargo install mycelium", &[]),
        Some("mycelium cargo install mycelium".into())
    );
}

#[test]
fn test_rewrite_kubectl_describe() {
    assert_eq!(
        rewrite_command("kubectl describe pod mypod", &[]),
        Some("mycelium kubectl describe pod mypod".into())
    );
}

#[test]
fn test_rewrite_docker_run() {
    assert_eq!(
        rewrite_command("docker run --rm ubuntu bash", &[]),
        Some("mycelium docker run --rm ubuntu bash".into())
    );
}

// --- #336: docker compose supported subcommands rewritten, unsupported skipped ---

#[test]
fn test_rewrite_docker_compose_ps() {
    assert_eq!(
        rewrite_command("docker compose ps", &[]),
        Some("mycelium docker compose ps".into())
    );
}

#[test]
fn test_rewrite_docker_compose_logs() {
    assert_eq!(
        rewrite_command("docker compose logs web", &[]),
        Some("mycelium docker compose logs web".into())
    );
}

#[test]
fn test_rewrite_docker_compose_build() {
    assert_eq!(
        rewrite_command("docker compose build", &[]),
        Some("mycelium docker compose build".into())
    );
}

#[test]
fn test_rewrite_docker_compose_up_skipped() {
    assert_eq!(rewrite_command("docker compose up -d", &[]), None);
}

#[test]
fn test_rewrite_docker_compose_down_skipped() {
    assert_eq!(rewrite_command("docker compose down", &[]), None);
}

#[test]
fn test_rewrite_docker_compose_config_skipped() {
    assert_eq!(
        rewrite_command("docker compose -f foo.yaml config --services", &[]),
        None
    );
}

// --- AWS / psql (PR #216) ---

#[test]
fn test_classify_aws() {
    assert!(matches!(
        classify_command("aws s3 ls"),
        Classification::Supported {
            mycelium_equivalent: "mycelium aws",
            ..
        }
    ));
}

#[test]
fn test_classify_aws_ec2() {
    assert!(matches!(
        classify_command("aws ec2 describe-instances"),
        Classification::Supported {
            mycelium_equivalent: "mycelium aws",
            ..
        }
    ));
}

#[test]
fn test_classify_psql() {
    assert!(matches!(
        classify_command("psql -U postgres"),
        Classification::Supported {
            mycelium_equivalent: "mycelium psql",
            ..
        }
    ));
}

#[test]
fn test_classify_psql_url() {
    assert!(matches!(
        classify_command("psql postgres://localhost/mydb"),
        Classification::Supported {
            mycelium_equivalent: "mycelium psql",
            ..
        }
    ));
}

#[test]
fn test_rewrite_aws() {
    assert_eq!(
        rewrite_command("aws s3 ls", &[]),
        Some("mycelium aws s3 ls".into())
    );
}

#[test]
fn test_rewrite_aws_ec2() {
    assert_eq!(
        rewrite_command("aws ec2 describe-instances --region us-east-1", &[]),
        Some("mycelium aws ec2 describe-instances --region us-east-1".into())
    );
}

#[test]
fn test_rewrite_psql() {
    assert_eq!(
        rewrite_command("psql -U postgres -d mydb", &[]),
        Some("mycelium psql -U postgres -d mydb".into())
    );
}

// --- Python tooling ---

#[test]
fn test_classify_ruff_check() {
    assert!(matches!(
        classify_command("ruff check ."),
        Classification::Supported {
            mycelium_equivalent: "mycelium ruff",
            ..
        }
    ));
}

#[test]
fn test_classify_ruff_format() {
    assert!(matches!(
        classify_command("ruff format src/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium ruff",
            ..
        }
    ));
}

#[test]
fn test_classify_pytest() {
    assert!(matches!(
        classify_command("pytest tests/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pytest",
            ..
        }
    ));
}

#[test]
fn test_classify_python_m_pytest() {
    assert!(matches!(
        classify_command("python -m pytest tests/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pytest",
            ..
        }
    ));
}

#[test]
fn test_classify_pip_list() {
    assert!(matches!(
        classify_command("pip list"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pip",
            ..
        }
    ));
}

#[test]
fn test_classify_uv_pip_list() {
    assert!(matches!(
        classify_command("uv pip list"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pip",
            ..
        }
    ));
}

#[test]
fn test_rewrite_ruff_check() {
    assert_eq!(
        rewrite_command("ruff check .", &[]),
        Some("mycelium ruff check .".into())
    );
}

#[test]
fn test_rewrite_ruff_format() {
    assert_eq!(
        rewrite_command("ruff format src/", &[]),
        Some("mycelium ruff format src/".into())
    );
}

#[test]
fn test_rewrite_pytest() {
    assert_eq!(
        rewrite_command("pytest tests/", &[]),
        Some("mycelium pytest tests/".into())
    );
}

#[test]
fn test_rewrite_python_m_pytest() {
    assert_eq!(
        rewrite_command("python -m pytest -x tests/", &[]),
        Some("mycelium pytest -x tests/".into())
    );
}

#[test]
fn test_rewrite_pip_list() {
    assert_eq!(
        rewrite_command("pip list", &[]),
        Some("mycelium pip list".into())
    );
}

#[test]
fn test_rewrite_pip_outdated() {
    assert_eq!(
        rewrite_command("pip outdated", &[]),
        Some("mycelium pip outdated".into())
    );
}

#[test]
fn test_rewrite_uv_pip_list() {
    assert_eq!(
        rewrite_command("uv pip list", &[]),
        Some("mycelium pip list".into())
    );
}

#[test]
fn test_rewrite_python3_m_pytest() {
    assert_eq!(
        rewrite_command("python3 -m pytest -x tests/", &[]),
        Some("mycelium pytest -x tests/".into())
    );
}

#[test]
fn test_classify_python3_m_pytest() {
    assert!(matches!(
        classify_command("python3 -m pytest tests/"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pytest",
            ..
        }
    ));
}

#[test]
fn test_rewrite_python3_m_pip_list() {
    assert_eq!(
        rewrite_command("python3 -m pip list", &[]),
        Some("mycelium pip list".into())
    );
}

#[test]
fn test_classify_python3_m_pip_list() {
    assert!(matches!(
        classify_command("python3 -m pip list"),
        Classification::Supported {
            mycelium_equivalent: "mycelium pip",
            ..
        }
    ));
}

#[test]
fn test_rewrite_python_m_pip_install() {
    assert_eq!(
        rewrite_command("python -m pip install requests", &[]),
        Some("mycelium pip install requests".into())
    );
}

// --- Go tooling ---

#[test]
fn test_classify_go_test() {
    assert!(matches!(
        classify_command("go test ./..."),
        Classification::Supported {
            mycelium_equivalent: "mycelium go",
            ..
        }
    ));
}

#[test]
fn test_classify_go_build() {
    assert!(matches!(
        classify_command("go build ./..."),
        Classification::Supported {
            mycelium_equivalent: "mycelium go",
            ..
        }
    ));
}

#[test]
fn test_classify_go_vet() {
    assert!(matches!(
        classify_command("go vet ./..."),
        Classification::Supported {
            mycelium_equivalent: "mycelium go",
            ..
        }
    ));
}

#[test]
fn test_classify_golangci_lint() {
    assert!(matches!(
        classify_command("golangci-lint run"),
        Classification::Supported {
            mycelium_equivalent: "mycelium golangci-lint",
            ..
        }
    ));
}

#[test]
fn test_rewrite_go_test() {
    assert_eq!(
        rewrite_command("go test ./...", &[]),
        Some("mycelium go test ./...".into())
    );
}

#[test]
fn test_rewrite_go_build() {
    assert_eq!(
        rewrite_command("go build ./...", &[]),
        Some("mycelium go build ./...".into())
    );
}

#[test]
fn test_rewrite_go_vet() {
    assert_eq!(
        rewrite_command("go vet ./...", &[]),
        Some("mycelium go vet ./...".into())
    );
}

#[test]
fn test_rewrite_golangci_lint() {
    assert_eq!(
        rewrite_command("golangci-lint run ./...", &[]),
        Some("mycelium golangci-lint run ./...".into())
    );
}

// --- JS/TS tooling ---

#[test]
fn test_classify_vitest() {
    assert!(matches!(
        classify_command("vitest run"),
        Classification::Supported {
            mycelium_equivalent: "mycelium vitest",
            ..
        }
    ));
}

#[test]
fn test_rewrite_vitest() {
    assert_eq!(
        rewrite_command("vitest run", &[]),
        Some("mycelium vitest run".into())
    );
}

#[test]
fn test_rewrite_pnpm_vitest() {
    assert_eq!(
        rewrite_command("pnpm vitest run", &[]),
        Some("mycelium vitest run".into())
    );
}

#[test]
fn test_classify_prisma() {
    assert!(matches!(
        classify_command("npx prisma migrate dev"),
        Classification::Supported {
            mycelium_equivalent: "mycelium prisma",
            ..
        }
    ));
}

#[test]
fn test_rewrite_prisma() {
    assert_eq!(
        rewrite_command("npx prisma migrate dev", &[]),
        Some("mycelium prisma migrate dev".into())
    );
}

#[test]
fn test_rewrite_prettier() {
    assert_eq!(
        rewrite_command("npx prettier --check src/", &[]),
        Some("mycelium prettier --check src/".into())
    );
}

#[test]
fn test_rewrite_pnpm_list() {
    assert_eq!(
        rewrite_command("pnpm list", &[]),
        Some("mycelium pnpm list".into())
    );
}

// --- Compound operator edge cases ---

#[test]
fn test_rewrite_compound_or() {
    // `||` fallback: left rewritten, right rewritten
    assert_eq!(
        rewrite_command("cargo test || cargo build", &[]),
        Some("mycelium cargo test || mycelium cargo build".into())
    );
}

#[test]
fn test_rewrite_compound_semicolon() {
    assert_eq!(
        rewrite_command("git status; cargo test", &[]),
        Some("mycelium git status; mycelium cargo test".into())
    );
}

#[test]
fn test_rewrite_compound_pipe_raw_filter() {
    // Pipe: keep raw so downstream filters see native stdout.
    assert_eq!(rewrite_command("cargo test | grep FAILED", &[]), None);
}

#[test]
fn test_rewrite_compound_pipe_git_grep() {
    assert_eq!(rewrite_command("git log -10 | grep feat", &[]), None);
}

#[test]
fn test_rewrite_compound_four_segments() {
    assert_eq!(
            rewrite_command(
                "cargo fmt --all && cargo clippy && cargo test && git status",
                &[]
            ),
            Some(
                "mycelium cargo fmt --all && mycelium cargo clippy && mycelium cargo test && mycelium git status"
                    .into()
            )
        );
}

#[test]
fn test_rewrite_compound_mixed_supported_unsupported() {
    // unsupported segments stay raw
    assert_eq!(
        rewrite_command("cargo test && ansible-playbook site.yml", &[]),
        Some("mycelium cargo test && ansible-playbook site.yml".into())
    );
}

#[test]
fn test_rewrite_compound_all_unsupported_returns_none() {
    // No rewrite at all: returns None
    assert_eq!(
        rewrite_command(
            "ansible-playbook site.yml && ansible-vault encrypt secrets.yml",
            &[]
        ),
        None
    );
}

// --- sudo / env prefix + rewrite ---

#[test]
fn test_rewrite_sudo_docker() {
    assert_eq!(
        rewrite_command("sudo docker ps", &[]),
        Some("sudo mycelium docker ps".into())
    );
}

#[test]
fn test_rewrite_env_var_prefix() {
    assert_eq!(
        rewrite_command("GIT_SSH_COMMAND=ssh git push origin main", &[]),
        Some("GIT_SSH_COMMAND=ssh mycelium git push origin main".into())
    );
}

// --- find with native flags ---

#[test]
fn test_rewrite_find_with_flags() {
    assert_eq!(
        rewrite_command("find . -name '*.rs' -type f", &[]),
        Some("mycelium find . -name '*.rs' -type f".into())
    );
}

#[test]
fn test_rewrite_find_with_flags_prefers_fd_when_available() {
    let _guard = set_find_fd_rewrite_active_for_tests(true);
    assert_eq!(
        rewrite_command("find . -name '*.rs' -type f", &[]),
        Some("fd -e rs --type f .".into())
    );
}

#[test]
fn test_rewrite_find_name_glob_prefers_fd_glob() {
    let _guard = set_find_fd_rewrite_active_for_tests(true);
    assert_eq!(
        rewrite_command("find src -name 'test_*'", &[]),
        Some(crate::platform::render_shell_command(&[
            "fd".to_string(),
            "--glob".to_string(),
            "test_*".to_string(),
            "src".to_string(),
        ]))
    );
}

#[test]
fn test_rewrite_find_complex_commands_stay_raw_even_with_fd() {
    let _guard = set_find_fd_rewrite_active_for_tests(true);
    assert_eq!(
        rewrite_command("find . -name '*.rs' -exec sed -n '1p' {} \\;", &[]),
        None
    );
}

#[test]
fn test_rewrite_find_uses_mycelium_find_when_fd_rewrite_disabled() {
    let _guard = set_find_fd_rewrite_active_for_tests(false);
    assert_eq!(
        rewrite_command("find . -name '*.rs' -type f", &[]),
        Some("mycelium find . -name '*.rs' -type f".into())
    );
}

// --- Ensure PATTERNS and RULES stay aligned after modifications ---

#[test]
fn test_patterns_rules_aligned_after_aws_psql() {
    // If this fails, someone added a PATTERN without a matching RULE (or vice versa)
    assert_eq!(
        PATTERNS.len(),
        RULES.len(),
        "PATTERNS[{}] != RULES[{}] — they must stay 1:1",
        PATTERNS.len(),
        RULES.len()
    );
}

// --- All RULES have non-empty mycelium_cmd and at least one rewrite_prefix ---

#[test]
fn test_all_rules_have_valid_mycelium_cmd() {
    for rule in RULES {
        assert!(
            !rule.mycelium_cmd.is_empty(),
            "Rule with empty mycelium_cmd found"
        );
        assert!(
            rule.mycelium_cmd.starts_with("mycelium "),
            "mycelium_cmd '{}' must start with 'mycelium '",
            rule.mycelium_cmd
        );
        assert!(
            !rule.rewrite_prefixes.is_empty(),
            "Rule '{}' has no rewrite_prefixes",
            rule.mycelium_cmd
        );
    }
}

// --- exclude_commands (#243) ---

#[test]
fn test_rewrite_excludes_curl() {
    let excluded = vec!["curl".to_string()];
    assert_eq!(
        rewrite_command("curl https://api.example.com/health", &excluded),
        None
    );
}

#[test]
fn test_rewrite_exclude_does_not_affect_other_commands() {
    let excluded = vec!["curl".to_string()];
    assert_eq!(
        rewrite_command("git status", &excluded),
        Some("mycelium git status".into())
    );
}

#[test]
fn test_rewrite_empty_excludes_rewrites_curl() {
    let excluded: Vec<String> = vec![];
    assert!(rewrite_command("curl https://api.example.com", &excluded).is_some());
}

#[test]
fn test_rewrite_compound_partial_exclude() {
    // curl excluded but git still rewrites
    let excluded = vec!["curl".to_string()];
    assert_eq!(
        rewrite_command("git status && curl https://api.example.com", &excluded),
        Some("mycelium git status && curl https://api.example.com".into())
    );
}

// --- Every PATTERN compiles to a valid Regex ---

#[test]
fn test_all_patterns_are_valid_regex() {
    use regex::Regex;
    for (i, pattern) in PATTERNS.iter().enumerate() {
        assert!(
            Regex::new(pattern).is_ok(),
            "PATTERNS[{i}] = '{pattern}' is not a valid regex"
        );
    }
}

// --- #196: gh --json/--jq/--template passthrough ---

#[test]
fn test_rewrite_gh_json_skipped() {
    assert_eq!(rewrite_command("gh pr list --json number,title", &[]), None);
}

#[test]
fn test_rewrite_gh_jq_skipped() {
    assert_eq!(
        rewrite_command("gh pr list --json number --jq '.[].number'", &[]),
        None
    );
}

#[test]
fn test_rewrite_gh_template_skipped() {
    assert_eq!(
        rewrite_command("gh pr view 42 --template '{{.title}}'", &[]),
        None
    );
}

#[test]
fn test_rewrite_gh_api_json_skipped() {
    assert_eq!(
        rewrite_command("gh api repos/owner/repo --jq '.name'", &[]),
        None
    );
}

#[test]
fn test_rewrite_gh_without_json_still_works() {
    assert_eq!(
        rewrite_command("gh pr list", &[]),
        Some("mycelium gh pr list".into())
    );
}

#[test]
fn test_classify_task_runner_direct_exec_commands() {
    assert_eq!(
        classify_command("mise exec -- cargo test"),
        Classification::Supported {
            mycelium_equivalent: "mycelium cargo",
            category: "Cargo",
            estimated_savings_pct: 90.0,
            status: MyceliumStatus::Existing,
        }
    );
}

#[test]
fn test_rewrite_task_runner_direct_exec_commands() {
    assert_eq!(
        rewrite_command("mise exec -- cargo test", &[]),
        Some("mise exec -- mycelium cargo test".into())
    );
    assert_eq!(
        rewrite_command("just -- git status", &[]),
        Some("just -- mycelium git status".into())
    );
    assert_eq!(
        rewrite_command("task -- cargo test", &[]),
        Some("task -- mycelium cargo test".into())
    );
}

#[test]
fn test_rewrite_nested_task_runner_structured_gh_stays_raw() {
    assert_eq!(
        rewrite_command("mise exec -- just -- gh pr list --json number", &[]),
        None
    );
}

#[test]
fn test_rewrite_task_runner_ambiguous_commands_stay_raw() {
    assert_eq!(rewrite_command("just test", &[]), None);
    assert_eq!(rewrite_command("task build", &[]), None);
}

#[test]
fn test_display_command_for_discover_uses_wrapped_command() {
    assert_eq!(
        display_command_for_discover("FOO=1 mise exec -- cargo test"),
        "cargo test"
    );
    assert_eq!(
        display_command_for_discover("just -- git status"),
        "git status"
    );
}

#[test]
fn test_rewrite_excluded_command_with_env_prefix_stays_raw() {
    let excluded = vec!["git".to_string()];
    assert_eq!(rewrite_command("FOO=1 git status", &excluded), None);
}

#[test]
fn test_rewrite_excluded_command_with_sudo_stays_raw() {
    let excluded = vec!["git".to_string()];
    assert_eq!(rewrite_command("sudo git status", &excluded), None);
}

#[test]
fn test_rewrite_literal_pipe_in_quoted_argument_still_rewrites() {
    assert_eq!(
        rewrite_command("git log --grep 'feat|fix'", &[]),
        Some("mycelium git log --grep 'feat|fix'".into())
    );
    assert_eq!(
        rewrite_command("rg 'foo|bar' src", &[]),
        Some("mycelium grep 'foo|bar' src".into())
    );
}

#[test]
fn test_rewrite_escaped_shell_metacharacters_still_rewrites() {
    assert_eq!(
        rewrite_command(r"rg foo\|bar src", &[]),
        Some(r"mycelium grep foo\|bar src".into())
    );
    assert_eq!(
        rewrite_command(r"rg foo\;bar src", &[]),
        Some(r"mycelium grep foo\;bar src".into())
    );
}

#[test]
fn test_rewrite_command_substitution_stays_raw() {
    assert_eq!(
        rewrite_command("echo $(git status && cargo test)", &[]),
        None
    );
}

#[test]
fn test_rewrite_backticks_stay_raw() {
    assert_eq!(
        rewrite_command("echo `git status && cargo test`", &[]),
        None
    );
}

#[test]
fn test_rewrite_subshell_group_stays_raw() {
    assert_eq!(
        rewrite_command("git status && (cargo test; git status)", &[]),
        None
    );
}

#[test]
fn test_rewrite_brace_group_stays_raw() {
    assert_eq!(rewrite_command("{ git status; cargo test; }", &[]), None);
}

#[test]
fn test_rewrite_literal_mycelium_disabled_value_still_rewrites() {
    assert_eq!(
        rewrite_command("git log --grep MYCELIUM_DISABLED=1", &[]),
        Some("mycelium git log --grep MYCELIUM_DISABLED=1".into())
    );
}

#[test]
fn test_rewrite_gh_literal_json_argument_still_rewrites() {
    assert_eq!(
        rewrite_command("gh pr list --search '--json'", &[]),
        Some("mycelium gh pr list --search '--json'".into())
    );
}

#[test]
fn test_rewrite_compound_preserves_structured_gh_passthrough_on_later_segment() {
    assert_eq!(
        rewrite_command("git status && gh pr list --json number", &[]),
        Some("mycelium git status && gh pr list --json number".into())
    );
}

#[test]
fn test_rewrite_compound_preserves_mycelium_disabled_on_later_segment() {
    assert_eq!(
        rewrite_command("git status && MYCELIUM_DISABLED=1 cargo test", &[]),
        Some("mycelium git status && MYCELIUM_DISABLED=1 cargo test".into())
    );
}

#[test]
fn test_rewrite_literal_heredoc_and_arithmetic_markers_still_rewrites() {
    assert_eq!(
        rewrite_command("git log --grep '<<'", &[]),
        Some("mycelium git log --grep '<<'".into())
    );
    assert_eq!(
        rewrite_command("git log --grep '$((value))'", &[]),
        Some("mycelium git log --grep '$((value))'".into())
    );
}

#[test]
fn test_rewrite_safe_quoted_compound_list_still_rewrites() {
    assert_eq!(
        rewrite_command("git log --grep 'feat|fix' && cargo test", &[]),
        Some("mycelium git log --grep 'feat|fix' && mycelium cargo test".into())
    );
}

#[test]
fn test_rewrite_ansi_c_quoted_argument_stays_raw() {
    assert_eq!(rewrite_command(r"rg $'foo\nbar' src", &[]), None);
}

#[test]
fn test_rewrite_here_string_stays_raw() {
    assert_eq!(rewrite_command("git status <<< foo", &[]), None);
}

#[test]
fn test_rewrite_process_substitution_stays_raw() {
    assert_eq!(rewrite_command("git diff <(cat old) <(cat new)", &[]), None);
}

#[test]
fn test_classify_js_runner_prefixes() {
    let r = classify_command("bunx vitest run");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "bunx vitest should be Supported"
    );

    let r = classify_command("bun run eslint src/");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "bun run eslint should be Supported"
    );

    let r = classify_command("yarn exec tsc --noEmit");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "yarn exec tsc should be Supported"
    );

    let r = classify_command("yarn dlx prettier --check .");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "yarn dlx prettier should be Supported"
    );

    let r = classify_command("pnpm exec playwright test");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "pnpm exec playwright should be Supported"
    );

    let r = classify_command("pnpm dlx prisma migrate");
    assert!(
        matches!(r, Classification::Supported { .. }),
        "pnpm dlx prisma should be Supported"
    );
}

#[test]
fn test_rewrite_js_runner_prefixes() {
    assert_eq!(
        rewrite_command("bunx vitest run", &[]),
        Some("mycelium vitest run".to_string())
    );
    assert_eq!(
        rewrite_command("bun run eslint src/", &[]),
        Some("mycelium lint src/".to_string())
    );
    assert_eq!(
        rewrite_command("yarn exec tsc --noEmit", &[]),
        Some("mycelium tsc --noEmit".to_string())
    );
    assert_eq!(
        rewrite_command("pnpm exec next build", &[]),
        Some("mycelium next".to_string())
    );
}

#[test]
fn test_classify_alt_tools() {
    // podman → docker
    assert!(
        matches!(
            classify_command("podman ps -a"),
            Classification::Supported { .. }
        ),
        "podman ps should be Supported"
    );
    if let Classification::Supported {
        mycelium_equivalent,
        ..
    } = classify_command("podman ps -a")
    {
        assert!(
            mycelium_equivalent.contains("docker"),
            "podman should map to docker"
        );
    }

    // tofu → terraform
    assert!(
        matches!(
            classify_command("tofu plan -out=plan.tfplan"),
            Classification::Supported { .. }
        ),
        "tofu plan should be Supported"
    );
    if let Classification::Supported {
        mycelium_equivalent,
        ..
    } = classify_command("tofu plan -out=plan.tfplan")
    {
        assert!(
            mycelium_equivalent.contains("terraform"),
            "tofu should map to terraform"
        );
    }

    // egrep → grep
    assert!(
        matches!(
            classify_command("egrep -r 'TODO' src/"),
            Classification::Supported { .. }
        ),
        "egrep should be Supported"
    );
    if let Classification::Supported {
        mycelium_equivalent,
        ..
    } = classify_command("egrep -r 'TODO' src/")
    {
        assert!(
            mycelium_equivalent.contains("grep"),
            "egrep should map to grep"
        );
    }

    // fgrep → grep
    assert!(
        matches!(
            classify_command("fgrep 'error' log.txt"),
            Classification::Supported { .. }
        ),
        "fgrep should be Supported"
    );
    if let Classification::Supported {
        mycelium_equivalent,
        ..
    } = classify_command("fgrep 'error' log.txt")
    {
        assert!(
            mycelium_equivalent.contains("grep"),
            "fgrep should map to grep"
        );
    }

    // diffsitter → diff
    assert!(
        matches!(
            classify_command("diffsitter old.rs new.rs"),
            Classification::Supported { .. }
        ),
        "diffsitter should be Supported"
    );
    if let Classification::Supported {
        mycelium_equivalent,
        ..
    } = classify_command("diffsitter old.rs new.rs")
    {
        assert!(
            mycelium_equivalent.contains("diff"),
            "diffsitter should map to diff"
        );
    }
}

#[test]
fn test_rewrite_alt_tools() {
    assert_eq!(
        rewrite_command("podman ps -a", &[]),
        Some("mycelium docker ps -a".to_string())
    );
    assert_eq!(
        rewrite_command("tofu plan", &[]),
        Some("mycelium terraform plan".to_string())
    );
    assert_eq!(
        rewrite_command("egrep -r 'TODO' src/", &[]),
        Some("mycelium grep -r 'TODO' src/".to_string())
    );
    assert_eq!(
        rewrite_command("diffsitter old.rs new.rs", &[]),
        Some("mycelium diff old.rs new.rs".to_string())
    );
    assert_eq!(
        rewrite_command("opentofu apply", &[]),
        Some("mycelium terraform apply".to_string())
    );
}

// --- Piped diagnostic passthrough commands ---

#[test]
fn test_rewrite_stat_piped_head_returns_none() {
    // `stat x | head -3` — first segment is a diagnostic passthrough, so skip rewriting
    assert_eq!(rewrite_command("stat x | head -3", &[]), None);
}

#[test]
fn test_rewrite_which_piped_grep_returns_none() {
    // `which git | grep usr` — first segment is a diagnostic passthrough
    assert_eq!(rewrite_command("which git | grep usr", &[]), None);
}

#[test]
fn test_rewrite_echo_piped_returns_none() {
    // `echo hello | cat` — first segment is a diagnostic passthrough
    assert_eq!(rewrite_command("echo hello | cat", &[]), None);
}

#[test]
fn test_rewrite_non_diagnostic_pipe_still_blocked() {
    // `git log | head -3` — first segment is NOT a diagnostic passthrough;
    // the pipe blocks rewriting via has_unsafe_shell_syntax
    assert_eq!(rewrite_command("git log | head -3", &[]), None);
}
