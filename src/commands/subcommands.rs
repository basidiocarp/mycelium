use clap::Subcommand;
use std::ffi::OsString;

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List available and installed plugins
    List,
    /// Install a shipped plugin to the plugin directory, if this release includes any
    Install {
        /// Plugin name or "--all" to install all shipped plugins
        name: String,
        /// Overwrite existing plugin without prompting
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum GitCommands {
    /// Condensed diff output
    Diff {
        /// Git arguments (supports all git diff flags like --stat, --cached, etc)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// One-line commit history
    Log {
        /// Git arguments (supports all git log flags like --oneline, --graph, --all)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact status (supports all git status flags)
    Status {
        /// Git arguments (supports all git status flags like --porcelain, --short, -s)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact show (commit summary + stat + compacted diff)
    Show {
        /// Git arguments (supports all git show flags)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Add files → "ok ✓ \<hash\>"
    Add {
        /// Files and flags to add (supports all git add flags like -A, -p, --all, etc)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Commit → "ok ✓ \<hash\>"
    Commit {
        /// Git commit arguments (supports -a, -m, --amend, --allow-empty, etc)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Push → "ok ✓ \<branch\>"
    Push {
        /// Git push arguments (supports -u, remote, branch, etc.)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Pull → "ok ✓ \<stats\>"
    Pull {
        /// Git pull arguments (supports --rebase, remote, branch, etc.)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact branch listing (current/local/remote)
    Branch {
        /// Git branch arguments (supports -d, -D, -m, etc.)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Fetch → "ok fetched (N new refs)"
    Fetch {
        /// Git fetch arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Stash management (list, show, pop, apply, drop)
    Stash {
        /// Subcommand: list, show, pop, apply, drop, push
        subcommand: Option<String>,
        /// Additional arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact worktree listing
    Worktree {
        /// Git worktree arguments (add, remove, prune, or empty for list)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported git subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum PnpmCommands {
    /// List installed packages (ultra-dense)
    List {
        /// Depth level (default: 0)
        #[arg(short, long, default_value = "0")]
        depth: usize,
        /// Additional pnpm arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show outdated packages (condensed: "pkg: old → new")
    Outdated {
        /// Additional pnpm arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Install packages (filter progress bars)
    Install {
        /// Packages to install and additional pnpm arguments (flags starting with - are forwarded)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        packages: Vec<String>,
    },
    /// Build (delegates to next build filter)
    Build {
        /// Additional build arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Typecheck (delegates to tsc filter)
    Typecheck {
        /// Additional typecheck arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported pnpm subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum DockerCommands {
    /// List running containers
    Ps,
    /// List images
    Images,
    /// Show container logs (deduplicated)
    Logs { container: String },
    /// Docker Compose commands with compact output
    Compose {
        #[command(subcommand)]
        command: ComposeCommands,
    },
    /// Passthrough: runs any unsupported docker subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum ComposeCommands {
    /// List compose services (compact)
    Ps,
    /// Show compose logs (deduplicated)
    Logs {
        /// Optional service name
        service: Option<String>,
    },
    /// Build compose services (summary)
    Build {
        /// Optional service name
        service: Option<String>,
    },
    /// Passthrough: runs any unsupported compose subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum KubectlCommands {
    /// List pods
    Pods {
        #[arg(short, long)]
        namespace: Option<String>,
        /// All namespaces
        #[arg(short = 'A', long)]
        all: bool,
    },
    /// List services
    Services {
        #[arg(short, long)]
        namespace: Option<String>,
        /// All namespaces
        #[arg(short = 'A', long)]
        all: bool,
    },
    /// Show pod logs (deduplicated)
    Logs {
        pod: String,
        #[arg(short, long)]
        container: Option<String>,
    },
    /// Passthrough: runs any unsupported kubectl subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum VitestCommands {
    /// Run tests with filtered output (90% token reduction)
    Run {
        /// Additional vitest arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum PrismaCommands {
    /// Generate Prisma Client (strip ASCII art)
    Generate {
        /// Additional prisma arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage migrations
    Migrate {
        #[command(subcommand)]
        command: PrismaMigrateCommands,
    },
    /// Push schema to database
    DbPush {
        /// Additional prisma arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum PrismaMigrateCommands {
    /// Create and apply migration
    Dev {
        /// Migration name
        #[arg(short, long)]
        name: Option<String>,
        /// Additional arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Check migration status
    Status {
        /// Additional arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Deploy migrations to production
    Deploy {
        /// Additional arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum CargoCommands {
    /// Build with compact output (strip Compiling lines, keep errors)
    Build {
        /// Additional cargo build arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Test with failures-only output
    Test {
        /// Additional cargo test arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Clippy with warnings grouped by lint rule
    Clippy {
        /// Additional cargo clippy arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Check with compact output (strip Checking lines, keep errors)
    Check {
        /// Additional cargo check arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Install with compact output (strip dep compilation, keep installed/errors)
    Install {
        /// Additional cargo install arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Nextest with failures-only output
    Nextest {
        /// Additional cargo nextest arguments (e.g., run, list, --lib)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported cargo subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum GoCommands {
    /// Run tests with compact output (90% token reduction via JSON streaming)
    Test {
        /// Additional go test arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Build with compact output (errors only)
    Build {
        /// Additional go build arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Vet with compact output
    Vet {
        /// Additional go vet arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported go subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand)]
pub enum GtCommands {
    /// Compact stack log output
    Log {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact submit output
    Submit {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact sync output
    Sync {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact restack output
    Restack {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Compact create output
    Create {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Branch info and management
    Branch {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: git-passthrough detection or direct gt execution
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand, Debug)]
pub enum GhCommands {
    /// Pull-request sub-commands (list, view, checks, status, create, merge, diff, comment, edit)
    Pr {
        #[command(subcommand)]
        command: GhPrCommands,
    },
    /// Issue sub-commands (list, view)
    Issue {
        #[command(subcommand)]
        command: GhIssueCommands,
    },
    /// Workflow-run sub-commands (list, view)
    Run {
        #[command(subcommand)]
        command: GhRunCommands,
    },
    /// Repo commands (defaults to repo view)
    Repo {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `gh api` — passed through with metric tracking
    Api {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Any other `gh` sub-command — passed through without filtering
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum GhPrCommands {
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    View {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Checks {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Status {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Create {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Merge {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Diff {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Comment {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Edit {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum GhIssueCommands {
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    View {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug)]
pub enum GhRunCommands {
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    View {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    #[command(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Debug, Subcommand)]
pub enum AwsCommands {
    /// AWS STS – Security Token Service
    Sts {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// AWS S3 – Simple Storage Service
    S3 {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// AWS EC2 – Elastic Compute Cloud
    Ec2 {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// AWS ECS – Elastic Container Service
    Ecs {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// AWS RDS – Relational Database Service
    Rds {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// AWS CloudFormation
    Cloudformation {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Other AWS service (passthrough)
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand, Debug)]
pub enum TerraformCommands {
    /// terraform plan with compact output (≥70% token reduction)
    Plan {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// terraform apply with compact output (≥30% token reduction)
    Apply {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// terraform init (strips progress bars)
    Init {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported terraform subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Subcommand, Debug)]
pub enum AtmosCommands {
    /// `atmos terraform ...` with compact plan/apply/init output
    Terraform {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `atmos describe ...` with truncated structured output
    Describe {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `atmos validate ...` with filtered validation issues
    Validate {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `atmos workflow ...` with truncated structured output
    Workflow {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// `atmos version`
    Version {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Passthrough: runs any unsupported atmos subcommand directly
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Debug, Subcommand)]
pub enum PipCommands {
    /// List installed packages
    List {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show outdated packages
    Outdated {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Install packages
    Install {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Uninstall packages (passthrough)
    Uninstall {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Show package info (passthrough)
    Show {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Other pip subcommand
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

#[derive(Debug, Subcommand)]
pub enum RuffCommands {
    /// Run ruff linter (default mode, forces --output-format=json)
    Check {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run ruff formatter
    Format {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Other ruff subcommand (version, rule, etc.) — also handles `ruff .` default check
    #[command(external_subcommand)]
    Other(Vec<OsString>),
}

/// Mycelium-only subcommands that should never fall back to raw execution.
/// If Clap fails to parse these, show the Clap error directly.
pub const MYCELIUM_META_COMMANDS: &[&str] = &[
    "gain",
    "discover",
    "learn",
    "context",
    "init",
    "config",
    "proxy",
    "invoke",
    "hook-audit",
    "economics",
    "cc-economics",
    "doctor",
    "self-update",
    "parse-health",
];
