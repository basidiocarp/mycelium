//! Generates shell completion scripts for bash, zsh, and fish.
use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{
    generate,
    shells::{Bash, Fish, Zsh},
};

use crate::commands::Cli;

/// Generate and print shell completion scripts for the given shell (bash, zsh, or fish).
pub fn run(shell: &str) -> Result<()> {
    let mut cmd = Cli::command();
    let mut stdout = std::io::stdout();
    match shell {
        "bash" => generate(Bash, &mut cmd, "mycelium", &mut stdout),
        "zsh" => generate(Zsh, &mut cmd, "mycelium", &mut stdout),
        "fish" => generate(Fish, &mut cmd, "mycelium", &mut stdout),
        other => anyhow::bail!("Unknown shell: '{}'. Supported: bash, zsh, fish", other),
    }
    Ok(())
}
