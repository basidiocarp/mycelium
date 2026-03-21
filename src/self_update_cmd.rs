//! Self-update command that checks GitHub releases and downloads the latest binary.

use anyhow::Result;

/// Check for updates and optionally download the latest Mycelium release from GitHub.
pub fn run(check_only: bool) -> Result<()> {
    spore::self_update::run(
        "mycelium",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_REPOSITORY"),
        check_only,
    )
}
