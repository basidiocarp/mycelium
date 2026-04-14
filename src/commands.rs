//! Clap CLI definition with all subcommands and global flags.
mod subcommands;

use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;

use crate::filter;

pub use subcommands::*;

#[derive(Parser)]
#[command(
    name = "mycelium",
    version,
    about = "Mycelium - Minimize LLM token consumption",
    long_about = "A high-performance CLI proxy designed to filter and summarize system outputs before they reach your LLM context.",
    help_template = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

\x1b[1;4mCommands:\x1b[0m

  \x1b[1mFiles & Search:\x1b[0m     ls, tree, read, peek, find, grep, diff
  \x1b[1mVCS & Code Review:\x1b[0m  git, gh, gt
  \x1b[1mBuild & Compile:\x1b[0m    cargo, tsc, next, go
  \x1b[1mLint & Format:\x1b[0m      lint, prettier, format, ruff, mypy, golangci-lint
  \x1b[1mTest:\x1b[0m               test, vitest, playwright, pytest
  \x1b[1mPackage Managers:\x1b[0m   pnpm, pip, npm, npx
  \x1b[1mDatabases & APIs:\x1b[0m   psql, prisma, curl, wget
  \x1b[1mInfrastructure:\x1b[0m     docker, kubectl, terraform, aws, atmos
  \x1b[1mLogs & Data:\x1b[0m        json, log, err, summary, env, deps
  \x1b[1mAnalytics:\x1b[0m          gain, discover, learn, economics
  \x1b[1mSetup:\x1b[0m              init, config, doctor, verify, self-update, completions, proxy, invoke, benchmark, plugin

\x1b[1;4mOptions:\x1b[0m
{options}
Run \x1b[36mmycelium <command> --help\x1b[0m for details on any command.
{after-help}"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Ultra-compact mode: ASCII icons, inline format (Level 2 optimizations)
    #[arg(short = 'u', long, global = true)]
    pub ultra_compact: bool,

    /// Set SKIP_ENV_VALIDATION=1 for child processes (Next.js, tsc, lint, prisma)
    #[arg(long = "skip-env", global = true)]
    pub skip_env: bool,

    /// Output result as a JSON envelope with token savings metrics
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    // ── Files & Search ──────────────────────────────────────────────────────
    /// List directory contents with token-optimized output (proxy to native ls)
    #[command(display_order = 10)]
    Ls {
        /// Arguments passed to ls (supports all native ls flags like -l, -a, -h, -R)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Directory tree with token-optimized output (proxy to native tree)
    #[command(display_order = 11)]
    Tree {
        /// Arguments passed to tree (supports all native tree flags like -L, -d, -a)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Read file with intelligent filtering
    #[command(display_order = 12)]
    Read {
        /// File to read
        file: PathBuf,
        /// Filter: none, minimal, aggressive
        #[arg(short, long, default_value = "minimal")]
        level: filter::FilterLevel,
        /// Max lines
        #[arg(short, long)]
        max_lines: Option<usize>,
        /// Show line numbers
        #[arg(short = 'n', long)]
        line_numbers: bool,
    },

    /// Generate 2-line technical summary of a file (heuristic-based)
    #[command(display_order = 13)]
    Peek {
        /// File to analyze
        file: PathBuf,
        /// Model: heuristic
        #[arg(short, long, default_value = "heuristic")]
        model: String,
        /// Force model download
        #[arg(long)]
        force_download: bool,
    },

    /// Find files with compact tree output (accepts native find flags like -name, -type)
    #[command(display_order = 14)]
    Find {
        /// All find arguments (supports both Mycelium and native find syntax)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Compact grep - strips whitespace, truncates, groups by file
    #[command(display_order = 15)]
    Grep {
        /// Pattern to search
        pattern: String,
        /// Path to search in
        #[arg(default_value = ".")]
        path: String,
        /// Max line length
        #[arg(short = 'l', long, default_value = "80")]
        max_len: usize,
        /// Max results to show
        #[arg(short, long, default_value = "50")]
        max: usize,
        /// Show only match context (not full line)
        #[arg(short, long)]
        context_only: bool,
        /// Filter by file type (e.g., ts, py, rust)
        #[arg(short = 't', long)]
        file_type: Option<String>,
        /// Show line numbers (always on, accepted for grep/rg compatibility)
        #[arg(short = 'n', long)]
        line_numbers: bool,
        /// Extra ripgrep arguments (e.g., -i, -A 3, -w, --glob)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Ultra-condensed diff (only changed lines)
    #[command(display_order = 16)]
    Diff {
        /// First file or - for stdin (unified diff)
        file1: PathBuf,
        /// Second file (optional if stdin)
        file2: Option<PathBuf>,
    },

    // ── VCS & Code Review ────────────────────────────────────────────────────
    /// Git commands with compact output
    #[command(display_order = 20)]
    Git {
        /// Change to directory before executing (like git -C <path>, can be repeated)
        #[arg(short = 'C', action = clap::ArgAction::Append)]
        directory: Vec<String>,

        /// Git configuration override (like git -c key=value, can be repeated)
        #[arg(short = 'c', action = clap::ArgAction::Append)]
        config_override: Vec<String>,

        /// Set the path to the .git directory
        #[arg(long = "git-dir")]
        git_dir: Option<String>,

        /// Set the path to the working tree
        #[arg(long = "work-tree")]
        work_tree: Option<String>,

        /// Disable pager (like git --no-pager)
        #[arg(long = "no-pager")]
        no_pager: bool,

        /// Skip optional locks (like git --no-optional-locks)
        #[arg(long = "no-optional-locks")]
        no_optional_locks: bool,

        /// Treat repository as bare (like git --bare)
        #[arg(long)]
        bare: bool,

        /// Treat pathspecs literally (like git --literal-pathspecs)
        #[arg(long = "literal-pathspecs")]
        literal_pathspecs: bool,

        #[command(subcommand)]
        command: GitCommands,
    },

    /// GitHub CLI (gh) commands with token-optimized output
    #[command(display_order = 21)]
    Gh {
        #[command(subcommand)]
        command: GhCommands,
    },

    /// Graphite (gt) stacked PR commands with compact output
    #[command(display_order = 22)]
    Gt {
        #[command(subcommand)]
        command: GtCommands,
    },

    // ── Build & Compile ──────────────────────────────────────────────────────
    /// Cargo commands with compact output
    #[command(display_order = 30)]
    Cargo {
        #[command(subcommand)]
        command: CargoCommands,
    },

    /// TypeScript compiler with grouped error output
    #[command(display_order = 31)]
    Tsc {
        /// TypeScript compiler arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Next.js build with compact output
    #[command(display_order = 32)]
    Next {
        /// Next.js build arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Go commands with compact output
    #[command(display_order = 33)]
    Go {
        #[command(subcommand)]
        command: GoCommands,
    },

    // ── Lint & Format ────────────────────────────────────────────────────────
    /// ESLint with grouped rule violations
    #[command(display_order = 40)]
    Lint {
        /// Linter arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Prettier format checker with compact output
    #[command(display_order = 41)]
    Prettier {
        /// Prettier arguments (e.g., --check, --write)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Auto-detects formatter (prettier, black, ruff) from project files
    #[command(display_order = 42)]
    Format {
        /// Formatter arguments (auto-detects formatter from project files)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Ruff linter/formatter with compact output
    #[command(display_order = 43)]
    Ruff {
        #[command(subcommand)]
        command: RuffCommands,
    },

    /// Mypy type checker with grouped error output
    #[command(display_order = 44)]
    Mypy {
        /// Mypy arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// golangci-lint with compact output
    #[command(display_order = 45, name = "golangci-lint")]
    GolangciLint {
        /// golangci-lint arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    // ── Test ─────────────────────────────────────────────────────────────────
    /// Run tests and show only failures (generic wrapper — use specific runners when available)
    #[command(display_order = 50)]
    Test {
        /// Test command (e.g. cargo test)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Vitest commands with compact output
    #[command(display_order = 51)]
    Vitest {
        #[command(subcommand)]
        command: VitestCommands,
    },

    /// Playwright E2E tests with compact output
    #[command(display_order = 52)]
    Playwright {
        /// Playwright arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Pytest test runner with compact output
    #[command(display_order = 53)]
    Pytest {
        /// Pytest arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    // ── Package Managers ─────────────────────────────────────────────────────
    /// pnpm commands with ultra-compact output
    #[command(display_order = 60)]
    Pnpm {
        #[command(subcommand)]
        command: PnpmCommands,
    },

    /// Pip package manager with compact output (auto-detects uv)
    #[command(display_order = 61)]
    Pip {
        #[command(subcommand)]
        command: PipCommands,
    },

    /// npm run with filtered output (strip boilerplate)
    #[command(display_order = 62)]
    Npm {
        /// npm run arguments (script name + options)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// npx with intelligent routing (tsc, eslint, prisma -> specialized filters)
    #[command(display_order = 63)]
    Npx {
        /// npx arguments (command + options)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    // ── Databases & APIs ─────────────────────────────────────────────────────
    /// PostgreSQL client with compact output (strip borders, compress tables)
    #[command(display_order = 70)]
    Psql {
        /// psql arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Prisma commands with compact output (no ASCII art)
    #[command(display_order = 71)]
    Prisma {
        #[command(subcommand)]
        command: PrismaCommands,
    },

    /// Curl with auto-JSON detection and schema output
    #[command(display_order = 72)]
    Curl {
        /// Curl arguments (URL + options)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Download with compact output (strips progress bars)
    #[command(display_order = 73)]
    Wget {
        /// URL to download
        url: String,
        /// Output to stdout instead of file
        #[arg(short = 'O', long)]
        stdout: bool,
        /// Additional wget arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    // ── Infrastructure ───────────────────────────────────────────────────────
    /// Docker commands with compact output
    #[command(display_order = 80)]
    Docker {
        #[command(subcommand)]
        command: DockerCommands,
    },

    /// Kubectl commands with compact output
    #[command(display_order = 81)]
    Kubectl {
        #[command(subcommand)]
        command: KubectlCommands,
    },

    /// Terraform with compact plan/apply output
    #[command(display_order = 82)]
    Terraform {
        #[command(subcommand)]
        command: TerraformCommands,
    },

    /// AWS CLI with compact output (force JSON, compress)
    #[command(display_order = 83)]
    Aws {
        #[command(subcommand)]
        command: AwsCommands,
    },

    /// Atmos orchestration with compact output for common flows
    #[command(display_order = 84)]
    Atmos {
        #[command(subcommand)]
        command: AtmosCommands,
    },

    // ── Logs & Data ──────────────────────────────────────────────────────────
    /// Show JSON structure without values
    #[command(display_order = 90)]
    Json {
        /// JSON file
        file: PathBuf,
        /// Max depth
        #[arg(short, long, default_value = "5")]
        depth: usize,
    },

    /// Filter and deduplicate log output
    #[command(display_order = 91)]
    Log {
        /// Log file (omit for stdin)
        file: Option<PathBuf>,
    },

    /// Run command and show only errors/warnings
    #[command(display_order = 92)]
    Err {
        /// Command to run
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Run command and show heuristic summary
    #[command(display_order = 93)]
    Summary {
        /// Command to run and summarize
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Show environment variables (filtered, sensitive masked)
    #[command(display_order = 94)]
    Env {
        /// Filter by name (e.g. PATH, AWS)
        #[arg(short, long)]
        filter: Option<String>,
        /// Show all (include sensitive)
        #[arg(long)]
        show_all: bool,
    },

    /// Summarize project dependencies
    #[command(display_order = 95)]
    Deps {
        /// Project path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    // ── Analytics ────────────────────────────────────────────────────────────
    /// Show token savings summary and history
    #[command(display_order = 100)]
    Gain {
        /// Scope to a project. Bare --project or -p uses the current directory.
        /// --project <name> searches known project paths (full path, case-insensitive substring).
        /// --project all shows per-project breakdown.
        #[arg(short, long, num_args = 0..=1, default_missing_value = ".",
              value_name = "NAME", conflicts_with = "project_path")]
        project: Option<String>,
        /// Filter to a specific project path (use '.' for current directory)
        #[arg(long = "project-path", value_name = "PATH")]
        project_path: Option<String>,
        /// Show per-project breakdown table
        #[arg(long)]
        projects: bool,
        /// Show rewrite quality scoring and passthrough diagnostics
        #[arg(
            long,
            conflicts_with_all = [
                "projects",
                "graph",
                "history",
                "quota",
                "daily",
                "weekly",
                "monthly",
                "all",
                "failures",
                "status",
                "compare",
                "format",
            ]
        )]
        diagnostics: bool,
        /// Explain how diagnostics are computed and which metrics are scoped globally vs per-project
        #[arg(long, requires = "diagnostics")]
        explain: bool,
        /// Show ASCII graph of daily savings
        #[arg(short, long)]
        graph: bool,
        /// Show recent command history
        #[arg(short = 'H', long)]
        history: bool,
        /// Maximum entries to include in recent history and by-command JSON exports
        #[arg(long, default_value = "10")]
        limit: usize,
        /// Show monthly quota savings estimate
        #[arg(short, long)]
        quota: bool,
        /// Subscription tier for quota calculation: pro, 5x, 20x
        #[arg(short, long, default_value = "20x", requires = "quota")]
        tier: String,
        /// Show detailed daily breakdown (all days)
        #[arg(short, long)]
        daily: bool,
        /// Show weekly breakdown
        #[arg(short, long)]
        weekly: bool,
        /// Show monthly breakdown
        #[arg(short, long)]
        monthly: bool,
        /// Show all time breakdowns (daily + weekly + monthly)
        #[arg(short, long)]
        all: bool,
        /// Output format: text, json, csv
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Show parse failure log (commands that fell back to raw execution)
        #[arg(short = 'F', long)]
        failures: bool,
        /// Show tracking database path, source, and health details
        #[arg(long)]
        status: bool,
        /// Run side-by-side token comparison for a command
        #[arg(long)]
        compare: Option<String>,
    },

    /// Discover missed Mycelium savings from Claude Code and Codex history
    #[command(display_order = 101)]
    Discover {
        /// Filter by project path (substring match)
        #[arg(short, long)]
        project: Option<String>,
        /// Max commands per section
        #[arg(short, long, default_value = "15")]
        limit: usize,
        /// Scan all projects (default: current project only)
        #[arg(short, long)]
        all: bool,
        /// Limit to sessions from last N days
        #[arg(short, long, default_value = "30")]
        since: u64,
        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Learn CLI corrections from Claude Code and Codex error history
    #[command(display_order = 102)]
    Learn {
        /// Filter by project path (substring match)
        #[arg(short, long)]
        project: Option<String>,
        /// Scan all projects (default: current project only)
        #[arg(short, long)]
        all: bool,
        /// Limit to sessions from last N days
        #[arg(short, long, default_value = "30")]
        since: u64,
        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Generate .claude/rules/cli-corrections.md file
        #[arg(short, long)]
        write_rules: bool,
        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long, default_value = "0.6")]
        min_confidence: f64,
        /// Minimum occurrences to include in report
        #[arg(long, default_value = "1")]
        min_occurrences: usize,
    },

    /// Gather context from Hyphae for a task (memories, errors, sessions, code)
    #[command(display_order = 103)]
    Context {
        /// Task description to gather context for
        #[arg(trailing_var_arg = true, required = true)]
        task: Vec<String>,
        /// Project name to scope the search
        #[arg(short, long)]
        project: Option<String>,
        /// Token budget (default: 2000)
        #[arg(short, long, default_value = "2000")]
        budget: u64,
        /// Include specific sources (comma-separated: memories,errors,sessions,code)
        #[arg(short, long)]
        include: Option<String>,
    },

    // ── Setup ────────────────────────────────────────────────────────────────
    /// Initialize Mycelium CLAUDE.md instructions and Claude Code hook integration
    #[command(display_order = 110)]
    Init {
        /// Use global ~/.claude integration instead of local CLAUDE.md
        #[arg(short, long)]
        global: bool,

        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Run the interactive ecosystem onboarding wizard
        #[arg(long, group = "mode", conflicts_with_all = ["show", "uninstall"])]
        onboard: bool,

        /// Inject full instructions into CLAUDE.md (legacy/docs-only mode)
        #[arg(long = "claude-md", group = "mode")]
        claude_md: bool,

        /// Hook only, no Mycelium.md (Claude Code Bash hook adapter; macOS/Linux only)
        #[arg(long = "hook-only", group = "mode")]
        hook_only: bool,

        /// Auto-patch settings.json without prompting (hook adapter mode)
        #[arg(long = "auto-patch", group = "patch")]
        auto_patch: bool,

        /// Skip settings.json patching and print manual instructions (hook adapter mode)
        #[arg(long = "no-patch", group = "patch")]
        no_patch: bool,

        /// Remove all Mycelium artifacts (hook, CLAUDE.md reference, settings.json entry)
        #[arg(long)]
        uninstall: bool,
    },

    /// Show or create configuration file
    #[command(display_order = 111)]
    Config {
        /// Create default config file
        #[arg(long)]
        create: bool,
    },

    /// Run health checks on Mycelium installation
    #[command(display_order = 112)]
    Doctor,

    /// Verify hook integrity (SHA-256 check)
    #[command(display_order = 113)]
    Verify,

    /// Check for and install updates to mycelium
    #[command(display_order = 114, name = "self-update")]
    SelfUpdate {
        /// Only check for updates, don't download
        #[arg(long)]
        check: bool,
    },

    /// Generate shell completion scripts
    #[command(display_order = 115)]
    Completions {
        /// Shell to generate completions for (bash, zsh, fish)
        shell: String,
    },

    /// Execute command without filtering but track usage
    #[command(display_order = 116)]
    Proxy {
        /// Command and arguments to execute
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },

    /// Execute a shell command through Mycelium rewrite resolution
    #[command(display_order = 117)]
    Invoke {
        /// Raw shell command to execute
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        command: Vec<String>,
        /// Explain the resolved execution path without running it
        #[arg(long)]
        explain: bool,
    },

    /// Measure token savings across available commands
    #[command(display_order = 118)]
    Benchmark {
        /// CI mode: exit non-zero if <80% of tests show savings
        #[arg(long)]
        ci: bool,
    },

    /// Manage filter plugins
    #[command(display_order = 119)]
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    // ── Hidden (internal/debug) ──────────────────────────────────────────────
    /// Word/line/byte count with compact output (strips paths and padding)
    #[command(hide = true)]
    Wc {
        /// Arguments passed to wc (files, flags like -l, -w, -c)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Show parser degradation statistics
    #[command(hide = true, name = "parse-health")]
    ParseHealth {
        /// Number of days to analyze (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,
    },

    /// Economics: global Claude Code spend (ccusage) vs Mycelium savings
    #[command(display_order = 104, name = "economics", alias = "cc-economics")]
    CcEconomics {
        /// Filter Mycelium savings to the current project (ccusage spend remains global)
        #[arg(short, long)]
        project: bool,
        /// Filter Mycelium savings to a specific project path (use '.' for current directory)
        #[arg(long = "project-path", value_name = "PATH")]
        project_path: Option<String>,
        /// Show detailed daily breakdown
        #[arg(short, long)]
        daily: bool,
        /// Show weekly breakdown
        #[arg(short, long)]
        weekly: bool,
        /// Show monthly breakdown
        #[arg(short, long)]
        monthly: bool,
        /// Show all time breakdowns (daily + weekly + monthly)
        #[arg(short, long)]
        all: bool,
        /// Output format: text, json, csv
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show hook rewrite audit metrics (requires MYCELIUM_HOOK_AUDIT=1)
    #[command(hide = true, name = "hook-audit")]
    HookAudit {
        /// Show entries from last N days (0 = all time)
        #[arg(short, long, default_value = "7")]
        since: u64,
    },

    /// Rewrite a raw command to its Mycelium equivalent (single source of truth for hooks)
    #[command(hide = true)]
    Rewrite {
        /// Raw command to rewrite (e.g. "git status", "cargo test && git push")
        cmd: String,
        /// Explain why the command rewrites or not
        #[arg(long)]
        explain: bool,
    },
}

#[cfg(test)]
mod tests;
