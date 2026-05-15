use clap::Args;

pub use super::subcommands::{CargoCommands, GoCommands, RuffCommands, VitestCommands};

/// Cargo commands with compact output
#[derive(Args)]
pub struct Cargo {
    #[command(subcommand)]
    pub command: CargoCommands,
}

/// TypeScript compiler with grouped error output
#[derive(Args)]
pub struct Tsc {
    /// TypeScript compiler arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Next.js build with compact output
#[derive(Args)]
pub struct Next {
    /// Next.js build arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Go commands with compact output
#[derive(Args)]
pub struct Go {
    #[command(subcommand)]
    pub command: GoCommands,
}

/// ESLint with grouped rule violations
#[derive(Args)]
pub struct Lint {
    /// Linter arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Prettier format checker with compact output
#[derive(Args)]
pub struct Prettier {
    /// Prettier arguments (e.g., --check, --write)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Auto-detects formatter (prettier, black, ruff) from project files
#[derive(Args)]
pub struct Format {
    /// Formatter arguments (auto-detects formatter from project files)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Ruff linter/formatter with compact output
#[derive(Args)]
pub struct Ruff {
    #[command(subcommand)]
    pub command: RuffCommands,
}

/// Mypy type checker with grouped error output
#[derive(Args)]
pub struct Mypy {
    /// Mypy arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// golangci-lint with compact output
#[derive(Args)]
#[command(name = "golangci-lint")]
pub struct GolangciLint {
    /// golangci-lint arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Vitest commands with compact output
#[derive(Args)]
pub struct Vitest {
    #[command(subcommand)]
    pub command: VitestCommands,
}

/// Playwright E2E tests with compact output
#[derive(Args)]
pub struct Playwright {
    /// Playwright arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Pytest test runner with compact output
#[derive(Args)]
pub struct Pytest {
    /// Pytest arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Run tests and show only failures (generic wrapper — use specific runners when available)
#[derive(Args)]
pub struct Test {
    /// Test command (e.g. cargo test)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}
