use clap::Args;
use std::ffi::OsString;
use std::path::PathBuf;

pub use super::subcommands::PluginCommands;

/// Show JSON structure without values
#[derive(Args)]
pub struct Json {
    /// JSON file
    pub file: PathBuf,
    /// Max depth
    #[arg(short, long, default_value = "5")]
    pub depth: usize,
}

/// Filter and deduplicate log output
#[derive(Args)]
pub struct Log {
    /// Log file (omit for stdin)
    pub file: Option<PathBuf>,
}

/// Run command and show only errors/warnings
#[derive(Args)]
pub struct Err {
    /// Command to run
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

/// Run command and show heuristic summary
#[derive(Args)]
pub struct Summary {
    /// Command to run and summarize
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

/// Show environment variables (filtered, sensitive masked)
#[derive(Args)]
pub struct Env {
    /// Filter by name (e.g. PATH, AWS)
    #[arg(short, long)]
    pub filter: Option<String>,
    /// Show all (include sensitive)
    #[arg(long)]
    pub show_all: bool,
}

/// Summarize project dependencies
#[derive(Args)]
pub struct Deps {
    /// Project path
    #[arg(default_value = ".")]
    pub path: PathBuf,
}

/// Show token savings summary and history
#[derive(Args)]
pub struct Gain {
    /// Scope to a project. Bare --project or -p uses the current directory.
    /// --project <name> searches known project paths (full path, case-insensitive substring).
    /// --project all shows per-project breakdown.
    #[arg(short, long, num_args = 0..=1, default_missing_value = ".",
          value_name = "NAME", conflicts_with = "project_path")]
    pub project: Option<String>,
    /// Filter to a specific project path (use '.' for current directory)
    #[arg(long = "project-path", value_name = "PATH")]
    pub project_path: Option<String>,
    /// Show per-project breakdown table
    #[arg(long)]
    pub projects: bool,
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
    pub diagnostics: bool,
    /// Explain how diagnostics are computed and which metrics are scoped globally vs per-project
    #[arg(long, requires = "diagnostics")]
    pub explain: bool,
    /// Show ASCII graph of daily savings
    #[arg(short, long)]
    pub graph: bool,
    /// Show recent command history
    #[arg(short = 'H', long)]
    pub history: bool,
    /// Maximum entries to include in recent history and by-command JSON exports
    #[arg(long, default_value = "10")]
    pub limit: usize,
    /// Show monthly quota savings estimate
    #[arg(short, long)]
    pub quota: bool,
    /// Subscription tier for quota calculation: pro, 5x, 20x
    #[arg(short, long, default_value = "20x", requires = "quota")]
    pub tier: String,
    /// Show detailed daily breakdown (all days)
    #[arg(short, long)]
    pub daily: bool,
    /// Show weekly breakdown
    #[arg(short, long)]
    pub weekly: bool,
    /// Show monthly breakdown
    #[arg(short, long)]
    pub monthly: bool,
    /// Show all time breakdowns (daily + weekly + monthly)
    #[arg(short, long)]
    pub all: bool,
    /// Output format: text, json, csv
    #[arg(short, long, default_value = "text")]
    pub format: String,
    /// Show parse failure log (commands that fell back to raw execution)
    #[arg(short = 'F', long)]
    pub failures: bool,
    /// Show tracking database path, source, and health details
    #[arg(long)]
    pub status: bool,
    /// Run side-by-side token comparison for a command
    #[arg(long)]
    pub compare: Option<String>,
}

/// Start local unix-socket service endpoint (for cap and other local clients)
#[cfg(unix)]
#[derive(Args)]
pub struct ServeSocket {
    /// Enable compact output mode
    #[arg(long)]
    pub compact: bool,
}

/// Discover missed Mycelium savings from Claude Code and Codex history
#[derive(Args)]
pub struct Discover {
    /// Filter by project path (substring match)
    #[arg(short, long)]
    pub project: Option<String>,
    /// Max commands per section
    #[arg(short, long, default_value = "15")]
    pub limit: usize,
    /// Scan all projects (default: current project only)
    #[arg(short, long)]
    pub all: bool,
    /// Limit to sessions from last N days
    #[arg(short, long, default_value = "30")]
    pub since: u64,
    /// Output format: text, json
    #[arg(short, long, default_value = "text")]
    pub format: String,
}

/// Learn CLI corrections from Claude Code and Codex error history
#[derive(Args)]
pub struct Learn {
    /// Filter by project path (substring match)
    #[arg(short, long)]
    pub project: Option<String>,
    /// Scan all projects (default: current project only)
    #[arg(short, long)]
    pub all: bool,
    /// Limit to sessions from last N days
    #[arg(short, long, default_value = "30")]
    pub since: u64,
    /// Output format: text, json
    #[arg(short, long, default_value = "text")]
    pub format: String,
    /// Generate .claude/rules/cli-corrections.md file
    #[arg(short, long)]
    pub write_rules: bool,
    /// Minimum confidence threshold (0.0-1.0)
    #[arg(long, default_value = "0.6")]
    pub min_confidence: f64,
    /// Minimum occurrences to include in report
    #[arg(long, default_value = "1")]
    pub min_occurrences: usize,
}

/// Gather context from Hyphae for a task (memories, errors, sessions, code)
#[derive(Args)]
pub struct Context {
    /// Task description to gather context for
    #[arg(trailing_var_arg = true, required = true)]
    pub task: Vec<String>,
    /// Project name to scope the search
    #[arg(short, long)]
    pub project: Option<String>,
    /// Token budget (default: 2000)
    #[arg(short, long, default_value = "2000")]
    pub budget: u64,
    /// Include specific sources (comma-separated: memories,errors,sessions,code)
    #[arg(short, long)]
    pub include: Option<String>,
}

/// Initialize Mycelium CLAUDE.md instructions and Claude Code hook integration
#[derive(Args)]
pub struct Init {
    /// Use global ~/.claude integration instead of local CLAUDE.md
    #[arg(short, long)]
    pub global: bool,

    /// Show current configuration
    #[arg(long)]
    pub show: bool,

    /// Run the interactive ecosystem onboarding wizard
    #[arg(long, group = "mode", conflicts_with_all = ["show", "uninstall"])]
    pub onboard: bool,

    /// Inject full instructions into CLAUDE.md (legacy/docs-only mode)
    #[arg(long = "claude-md", group = "mode")]
    pub claude_md: bool,

    /// Hook only, no Mycelium.md (Claude Code Bash hook adapter; macOS/Linux only)
    #[arg(long = "hook-only", group = "mode")]
    pub hook_only: bool,

    /// Auto-patch settings.json without prompting (hook adapter mode)
    #[arg(long = "auto-patch", group = "patch")]
    pub auto_patch: bool,

    /// Skip settings.json patching and print manual instructions (hook adapter mode)
    #[arg(long = "no-patch", group = "patch")]
    pub no_patch: bool,

    /// Remove all Mycelium artifacts (hook, CLAUDE.md reference, settings.json entry)
    #[arg(long)]
    pub uninstall: bool,
}

/// Show or create configuration file
#[derive(Args)]
pub struct Config {
    /// Create default config file
    #[arg(long)]
    pub create: bool,
}

/// Run health checks on Mycelium installation
#[derive(Args)]
pub struct Doctor;

/// Verify hook integrity (SHA-256 check)
#[derive(Args)]
pub struct Verify;

/// Check for and install updates to mycelium
#[derive(Args)]
#[command(name = "self-update")]
pub struct SelfUpdate {
    /// Only check for updates, don't download
    #[arg(long)]
    pub check: bool,
}

/// Generate shell completion scripts
#[derive(Args)]
pub struct Completions {
    /// Shell to generate completions for (bash, zsh, fish)
    pub shell: String,
}

/// Execute command without filtering but track usage
#[derive(Args)]
pub struct Proxy {
    /// Command and arguments to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

/// Execute a shell command through Mycelium rewrite resolution
#[derive(Args)]
pub struct Invoke {
    /// Raw shell command to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
    /// Explain the resolved execution path without running it
    #[arg(long)]
    pub explain: bool,
}

/// Measure token savings across available commands
#[derive(Args)]
pub struct Benchmark {
    /// CI mode: exit non-zero if <80% of tests show savings
    #[arg(long)]
    pub ci: bool,
}

/// Manage filter plugins
#[derive(Args)]
pub struct Plugin {
    #[command(subcommand)]
    pub command: PluginCommands,
}

/// Word/line/byte count with compact output (strips paths and padding)
#[derive(Args)]
pub struct Wc {
    /// Arguments passed to wc (files, flags like -l, -w, -c)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Show parser degradation statistics
#[derive(Args)]
#[command(name = "parse-health")]
pub struct ParseHealth {
    /// Number of days to analyze (default: 30)
    #[arg(long, default_value = "30")]
    pub days: u32,
}

/// Economics: global Claude Code spend (ccusage) vs Mycelium savings
#[derive(Args)]
#[command(name = "economics")]
pub struct CcEconomics {
    /// Filter Mycelium savings to the current project (ccusage spend remains global)
    #[arg(short, long)]
    pub project: bool,
    /// Filter Mycelium savings to a specific project path (use '.' for current directory)
    #[arg(long = "project-path", value_name = "PATH")]
    pub project_path: Option<String>,
    /// Show detailed daily breakdown
    #[arg(short, long)]
    pub daily: bool,
    /// Show weekly breakdown
    #[arg(short, long)]
    pub weekly: bool,
    /// Show monthly breakdown
    #[arg(short, long)]
    pub monthly: bool,
    /// Show all time breakdowns (daily + weekly + monthly)
    #[arg(short, long)]
    pub all: bool,
    /// Output format: text, json, csv
    #[arg(short, long, default_value = "text")]
    pub format: String,
}

/// Show hook rewrite audit metrics (requires MYCELIUM_HOOK_AUDIT=1)
#[derive(Args)]
#[command(name = "hook-audit")]
pub struct HookAudit {
    /// Show entries from last N days (0 = all time)
    #[arg(short, long, default_value = "7")]
    pub since: u64,
}

/// Rewrite a raw command to its Mycelium equivalent (single source of truth for hooks)
#[derive(Args)]
pub struct Rewrite {
    /// Raw command to rewrite (e.g. "git status", "cargo test && git push")
    pub cmd: String,
    /// Explain why the command rewrites or not
    #[arg(long)]
    pub explain: bool,
}

/// Explain which filter would match a command and why (does not execute)
#[derive(Args)]
pub struct Explain {
    /// Command to explain (e.g. "git log -20" or "cargo test")
    pub cmd: String,
}

/// pnpm commands with ultra-compact output
#[derive(Args)]
pub struct Pnpm {
    #[command(subcommand)]
    pub command: super::subcommands::PnpmCommands,
}

/// Pip package manager with compact output (auto-detects uv)
#[derive(Args)]
pub struct Pip {
    #[command(subcommand)]
    pub command: super::subcommands::PipCommands,
}

/// npm run with filtered output (strip boilerplate)
#[derive(Args)]
pub struct Npm {
    /// npm run arguments (script name + options)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// npx with intelligent routing (tsc, eslint, prisma -> specialized filters)
#[derive(Args)]
pub struct Npx {
    /// npx arguments (command + options)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}
