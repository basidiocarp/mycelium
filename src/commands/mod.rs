//! Clap CLI definition with all subcommands and global flags.

mod files;
mod vcs;
mod build;
mod infra;
mod setup;
pub mod subcommands;
pub use subcommands::*;

use clap::{Parser, Subcommand};

pub use files::{Diff, Find, Grep, Ls, Peek, Read, Tree, Wc};
pub use vcs::{Gh, Git, Gt};
pub use build::{
    Cargo, Format, Go, GolangciLint, Lint, Mypy, Next, Prettier, Pytest, Ruff, Test, Tsc, Vitest,
    Playwright,
};
pub use infra::{Atmos, Aws, Curl, Docker, Kubectl, Prisma, Psql, Terraform, Wget};
pub use setup::{
    Benchmark, CcEconomics, Completions, Config, Context, Deps, Discover, Doctor, Env, Err,
    Explain, Gain, HookAudit, Init, Invoke, Json, Learn, Log, Npm, Npx, ParseHealth, Pip, Plugin,
    Pnpm, Proxy, Rewrite, SelfUpdate, ServeSocket, Summary, Verify,
};

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
    Ls(files::Ls),

    /// Directory tree with token-optimized output (proxy to native tree)
    #[command(display_order = 11)]
    Tree(files::Tree),

    /// Read file with intelligent filtering
    #[command(display_order = 12)]
    Read(files::Read),

    /// Generate 2-line technical summary of a file (heuristic-based)
    #[command(display_order = 13)]
    Peek(files::Peek),

    /// Find files with compact tree output (accepts native find flags like -name, -type)
    #[command(display_order = 14)]
    Find(files::Find),

    /// Compact grep - strips whitespace, truncates, groups by file
    #[command(display_order = 15)]
    Grep(files::Grep),

    /// Ultra-condensed diff (only changed lines)
    #[command(display_order = 16)]
    Diff(files::Diff),

    // ── VCS & Code Review ────────────────────────────────────────────────────
    /// Git commands with compact output
    #[command(display_order = 20)]
    Git(vcs::Git),

    /// GitHub CLI (gh) commands with token-optimized output
    #[command(display_order = 21)]
    Gh(vcs::Gh),

    /// Graphite (gt) stacked PR commands with compact output
    #[command(display_order = 22)]
    Gt(vcs::Gt),

    // ── Build & Compile ──────────────────────────────────────────────────────
    /// Cargo commands with compact output
    #[command(display_order = 30)]
    Cargo(build::Cargo),

    /// TypeScript compiler with grouped error output
    #[command(display_order = 31)]
    Tsc(build::Tsc),

    /// Next.js build with compact output
    #[command(display_order = 32)]
    Next(build::Next),

    /// Go commands with compact output
    #[command(display_order = 33)]
    Go(build::Go),

    // ── Lint & Format ────────────────────────────────────────────────────────
    /// ESLint with grouped rule violations
    #[command(display_order = 40)]
    Lint(build::Lint),

    /// Prettier format checker with compact output
    #[command(display_order = 41)]
    Prettier(build::Prettier),

    /// Auto-detects formatter (prettier, black, ruff) from project files
    #[command(display_order = 42)]
    Format(build::Format),

    /// Ruff linter/formatter with compact output
    #[command(display_order = 43)]
    Ruff(build::Ruff),

    /// Mypy type checker with grouped error output
    #[command(display_order = 44)]
    Mypy(build::Mypy),

    /// golangci-lint with compact output
    #[command(display_order = 45, name = "golangci-lint")]
    GolangciLint(build::GolangciLint),

    // ── Test ─────────────────────────────────────────────────────────────────
    /// Run tests and show only failures (generic wrapper — use specific runners when available)
    #[command(display_order = 50)]
    Test(build::Test),

    /// Vitest commands with compact output
    #[command(display_order = 51)]
    Vitest(build::Vitest),

    /// Playwright E2E tests with compact output
    #[command(display_order = 52)]
    Playwright(build::Playwright),

    /// Pytest test runner with compact output
    #[command(display_order = 53)]
    Pytest(build::Pytest),

    // ── Package Managers ─────────────────────────────────────────────────────
    /// pnpm commands with ultra-compact output
    #[command(display_order = 60)]
    Pnpm(setup::Pnpm),

    /// Pip package manager with compact output (auto-detects uv)
    #[command(display_order = 61)]
    Pip(setup::Pip),

    /// npm run with filtered output (strip boilerplate)
    #[command(display_order = 62)]
    Npm(setup::Npm),

    /// npx with intelligent routing (tsc, eslint, prisma -> specialized filters)
    #[command(display_order = 63)]
    Npx(setup::Npx),

    // ── Databases & APIs ─────────────────────────────────────────────────────
    /// PostgreSQL client with compact output (strip borders, compress tables)
    #[command(display_order = 70)]
    Psql(infra::Psql),

    /// Prisma commands with compact output (no ASCII art)
    #[command(display_order = 71)]
    Prisma(infra::Prisma),

    /// Curl with auto-JSON detection and schema output
    #[command(display_order = 72)]
    Curl(infra::Curl),

    /// Download with compact output (strips progress bars)
    #[command(display_order = 73)]
    Wget(infra::Wget),

    // ── Infrastructure ───────────────────────────────────────────────────────
    /// Docker commands with compact output
    #[command(display_order = 80)]
    Docker(infra::Docker),

    /// Kubectl commands with compact output
    #[command(display_order = 81)]
    Kubectl(infra::Kubectl),

    /// Terraform with compact plan/apply output
    #[command(display_order = 82)]
    Terraform(infra::Terraform),

    /// AWS CLI with compact output (force JSON, compress)
    #[command(display_order = 83)]
    Aws(infra::Aws),

    /// Atmos orchestration with compact output for common flows
    #[command(display_order = 84)]
    Atmos(infra::Atmos),

    // ── Logs & Data ──────────────────────────────────────────────────────────
    /// Show JSON structure without values
    #[command(display_order = 90)]
    Json(setup::Json),

    /// Filter and deduplicate log output
    #[command(display_order = 91)]
    Log(setup::Log),

    /// Run command and show only errors/warnings
    #[command(display_order = 92)]
    Err(setup::Err),

    /// Run command and show heuristic summary
    #[command(display_order = 93)]
    Summary(setup::Summary),

    /// Show environment variables (filtered, sensitive masked)
    #[command(display_order = 94)]
    Env(setup::Env),

    /// Summarize project dependencies
    #[command(display_order = 95)]
    Deps(setup::Deps),

    // ── Analytics ────────────────────────────────────────────────────────────
    /// Show token savings summary and history
    #[command(display_order = 100)]
    Gain(setup::Gain),

    /// Start local unix-socket service endpoint (for cap and other local clients)
    #[cfg(unix)]
    #[command(hide = true)]
    ServeSocket(setup::ServeSocket),

    /// Discover missed Mycelium savings from Claude Code and Codex history
    #[command(display_order = 101)]
    Discover(setup::Discover),

    /// Learn CLI corrections from Claude Code and Codex error history
    #[command(display_order = 102)]
    Learn(setup::Learn),

    /// Gather context from Hyphae for a task (memories, errors, sessions, code)
    #[command(display_order = 103)]
    Context(setup::Context),

    // ── Setup ────────────────────────────────────────────────────────────────
    /// Initialize Mycelium CLAUDE.md instructions and Claude Code hook integration
    #[command(display_order = 110)]
    Init(setup::Init),

    /// Show or create configuration file
    #[command(display_order = 111)]
    Config(setup::Config),

    /// Run health checks on Mycelium installation
    #[command(display_order = 112)]
    Doctor,

    /// Verify hook integrity (SHA-256 check)
    #[command(display_order = 113)]
    Verify,

    /// Check for and install updates to mycelium
    #[command(display_order = 114, name = "self-update")]
    SelfUpdate(setup::SelfUpdate),

    /// Generate shell completion scripts
    #[command(display_order = 115)]
    Completions(setup::Completions),

    /// Execute command without filtering but track usage
    #[command(display_order = 116)]
    Proxy(setup::Proxy),

    /// Execute a shell command through Mycelium rewrite resolution
    #[command(display_order = 117)]
    Invoke(setup::Invoke),

    /// Measure token savings across available commands
    #[command(display_order = 118)]
    Benchmark(setup::Benchmark),

    /// Manage filter plugins
    #[command(display_order = 119)]
    Plugin(setup::Plugin),

    // ── Hidden (internal/debug) ──────────────────────────────────────────────
    /// Word/line/byte count with compact output (strips paths and padding)
    #[command(hide = true)]
    Wc(files::Wc),

    /// Show parser degradation statistics
    #[command(hide = true, name = "parse-health")]
    ParseHealth(setup::ParseHealth),

    /// Economics: global Claude Code spend (ccusage) vs Mycelium savings
    #[command(display_order = 104, name = "economics", alias = "cc-economics")]
    CcEconomics(setup::CcEconomics),

    /// Show hook rewrite audit metrics (requires MYCELIUM_HOOK_AUDIT=1)
    #[command(hide = true, name = "hook-audit")]
    HookAudit(setup::HookAudit),

    /// Rewrite a raw command to its Mycelium equivalent (single source of truth for hooks)
    #[command(hide = true)]
    Rewrite(setup::Rewrite),

    /// Explain which filter would match a command and why (does not execute)
    #[command(display_order = 120)]
    Explain(setup::Explain),
}

#[cfg(test)]
mod tests;
