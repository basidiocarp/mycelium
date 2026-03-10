use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let man_dir = out_dir.join("man");
    std::fs::create_dir_all(&man_dir).unwrap();

    let cmd = build_cli();

    // Generate top-level man page
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf).unwrap();
    std::fs::write(man_dir.join("mycelium.1"), buf).unwrap();

    // Generate subcommand man pages
    for subcommand in cmd.get_subcommands() {
        let sub_man = clap_mangen::Man::new(subcommand.clone());
        let mut buf = Vec::new();
        sub_man.render(&mut buf).unwrap();
        let name = format!("mycelium-{}.1", subcommand.get_name());
        std::fs::write(man_dir.join(&name), buf).unwrap();
    }

    println!("cargo:rerun-if-changed=src/commands.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
}

fn build_cli() -> clap::Command {
    clap::Command::new("mycelium")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Mycelium - Minimize LLM token consumption")
        .long_about(
            "A high-performance CLI proxy designed to filter and summarize system \
             outputs before they reach your LLM context. Achieves 60-90% token \
             savings on common development operations.",
        )
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::Count)
                .global(true)
                .help("Verbosity level (-v, -vv, -vvv)"),
        )
        .arg(
            clap::Arg::new("ultra-compact")
                .short('u')
                .long("ultra-compact")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Ultra-compact mode: ASCII icons, inline format"),
        )
        .arg(
            clap::Arg::new("skip-env")
                .long("skip-env")
                .action(clap::ArgAction::SetTrue)
                .global(true)
                .help("Set SKIP_ENV_VALIDATION=1 for child processes"),
        )
        .subcommand(
            clap::Command::new("ls").about("List directory contents with token-optimized output"),
        )
        .subcommand(clap::Command::new("tree").about("Directory tree with token-optimized output"))
        .subcommand(clap::Command::new("read").about("Read file with intelligent filtering"))
        .subcommand(
            clap::Command::new("smart")
                .about("Generate 2-line technical summary (heuristic-based)"),
        )
        .subcommand(clap::Command::new("git").about("Git commands with compact output"))
        .subcommand(
            clap::Command::new("gh").about("GitHub CLI (gh) commands with token-optimized output"),
        )
        .subcommand(
            clap::Command::new("aws").about("AWS CLI with compact output (force JSON, compress)"),
        )
        .subcommand(clap::Command::new("psql").about("PostgreSQL client with compact output"))
        .subcommand(clap::Command::new("pnpm").about("pnpm commands with ultra-compact output"))
        .subcommand(clap::Command::new("err").about("Run command and show only errors/warnings"))
        .subcommand(clap::Command::new("test").about("Run tests and show only failures"))
        .subcommand(clap::Command::new("json").about("Show JSON structure without values"))
        .subcommand(clap::Command::new("deps").about("Summarize project dependencies"))
        .subcommand(
            clap::Command::new("env")
                .about("Show environment variables (filtered, sensitive masked)"),
        )
        .subcommand(clap::Command::new("find").about("Find files with compact tree output"))
        .subcommand(clap::Command::new("diff").about("Ultra-condensed diff (only changed lines)"))
        .subcommand(clap::Command::new("log").about("Filter and deduplicate log output"))
        .subcommand(clap::Command::new("docker").about("Docker commands with compact output"))
        .subcommand(clap::Command::new("kubectl").about("Kubectl commands with compact output"))
        .subcommand(clap::Command::new("summary").about("Run command and show heuristic summary"))
        .subcommand(
            clap::Command::new("grep")
                .about("Compact grep - strips whitespace, truncates, groups by file"),
        )
        .subcommand(
            clap::Command::new("init").about("Initialize mycelium instructions in CLAUDE.md"),
        )
        .subcommand(
            clap::Command::new("wget").about("Download with compact output (strips progress bars)"),
        )
        .subcommand(clap::Command::new("wc").about("Word/line count with compact output"))
        .subcommand(clap::Command::new("gain").about("Show token savings statistics"))
        .subcommand(clap::Command::new("config").about("Show current mycelium configuration"))
        .subcommand(
            clap::Command::new("vitest")
                .about("Vitest test runner with failures-only output (99.5% token reduction)"),
        )
        .subcommand(
            clap::Command::new("prisma")
                .about("Prisma CLI without ASCII art (88% token reduction)"),
        )
        .subcommand(
            clap::Command::new("tsc")
                .about("TypeScript compiler errors grouped by file/code (83% token reduction)"),
        )
        .subcommand(
            clap::Command::new("next")
                .about("Next.js build with route/bundle metrics (87% token reduction)"),
        )
        .subcommand(
            clap::Command::new("lint")
                .about("ESLint/Biome with grouped rule violations (84% token reduction)"),
        )
        .subcommand(
            clap::Command::new("prettier")
                .about("Format checker showing files needing changes (70% token reduction)"),
        )
        .subcommand(clap::Command::new("format").about("Code formatter with compact output"))
        .subcommand(
            clap::Command::new("playwright")
                .about("E2E test results showing failures only (94% token reduction)"),
        )
        .subcommand(clap::Command::new("cargo").about("Cargo commands with compact output"))
        .subcommand(clap::Command::new("npm").about("npm commands with compact output"))
        .subcommand(clap::Command::new("npx").about("npx commands with compact output"))
        .subcommand(clap::Command::new("curl").about("HTTP requests with compact output"))
        .subcommand(
            clap::Command::new("discover")
                .about("Analyze Claude Code history for missed token savings"),
        )
        .subcommand(
            clap::Command::new("learn").about("Learn from past commands to improve filtering"),
        )
        .subcommand(
            clap::Command::new("proxy")
                .about("Execute command unfiltered but track usage for metrics"),
        )
        .subcommand(
            clap::Command::new("verify").about("Verify mycelium installation and configuration"),
        )
        .subcommand(
            clap::Command::new("ruff")
                .about("Ruff linter/formatter with JSON parsing (80%+ token reduction)"),
        )
        .subcommand(
            clap::Command::new("pytest")
                .about("Pytest test runner with state machine parser (90%+ token reduction)"),
        )
        .subcommand(
            clap::Command::new("mypy")
                .about("Mypy type checker grouped by file/error code (80% token reduction)"),
        )
        .subcommand(
            clap::Command::new("pip")
                .about("pip/uv package manager with compact output (70-85% token reduction)"),
        )
        .subcommand(
            clap::Command::new("go")
                .about("Go commands with compact output (80-90% token reduction)"),
        )
        .subcommand(clap::Command::new("gt").about("Graphite CLI with compact output"))
        .subcommand(
            clap::Command::new("golangci-lint")
                .about("golangci-lint with grouped violations (85% token reduction)"),
        )
        .subcommand(clap::Command::new("hook-audit").about("Audit shell hook configuration"))
        .subcommand(clap::Command::new("rewrite").about("Rewrite command for optimized execution"))
}
