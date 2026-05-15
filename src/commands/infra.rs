use clap::Args;

pub use super::subcommands::{DockerCommands, KubectlCommands, TerraformCommands, AwsCommands, AtmosCommands};

/// Docker commands with compact output
#[derive(Args)]
pub struct Docker {
    #[command(subcommand)]
    pub command: DockerCommands,
}

/// Kubectl commands with compact output
#[derive(Args)]
pub struct Kubectl {
    #[command(subcommand)]
    pub command: KubectlCommands,
}

/// Terraform with compact plan/apply output
#[derive(Args)]
pub struct Terraform {
    #[command(subcommand)]
    pub command: TerraformCommands,
}

/// AWS CLI with compact output (force JSON, compress)
#[derive(Args)]
pub struct Aws {
    #[command(subcommand)]
    pub command: AwsCommands,
}

/// Atmos orchestration with compact output for common flows
#[derive(Args)]
pub struct Atmos {
    #[command(subcommand)]
    pub command: AtmosCommands,
}

/// PostgreSQL client with compact output (strip borders, compress tables)
#[derive(Args)]
pub struct Psql {
    /// psql arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Prisma commands with compact output (no ASCII art)
#[derive(Args)]
pub struct Prisma {
    #[command(subcommand)]
    pub command: super::subcommands::PrismaCommands,
}

/// Curl with auto-JSON detection and schema output
#[derive(Args)]
pub struct Curl {
    /// Curl arguments (URL + options)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// Download with compact output (strips progress bars)
#[derive(Args)]
pub struct Wget {
    /// URL to download
    pub url: String,
    /// Output to stdout instead of file
    #[arg(short = 'O', long)]
    pub stdout: bool,
    /// Additional wget arguments
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}
