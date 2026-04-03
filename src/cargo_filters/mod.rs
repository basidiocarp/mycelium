//! Output filters for cargo subcommands (build, test, clippy, install, nextest).

mod build;
mod install;
mod nextest;
mod shared;
mod test;

pub(crate) use build::{filter_cargo_build, filter_cargo_clippy};
pub(crate) use install::filter_cargo_install;
pub(crate) use nextest::filter_cargo_nextest;

/// Check whether raw output looks like cargo output.
///
/// Returns true if any line matches cargo-specific patterns (Compiling, Checking,
/// Finished, error[E, warning:, test result:, etc.). Used to set quality to
/// Passthrough when the filter doesn't understand the input format.
pub(crate) fn looks_like_cargo_output(output: &str) -> bool {
    output.lines().any(|line| {
        let l = line.trim_start();
        l.starts_with("Compiling")
            || l.starts_with("Checking")
            || l.starts_with("Finished")
            || l.starts_with("Downloading")
            || l.starts_with("error[E")
            || l.starts_with("error:")
            || l.starts_with("warning:")
            || l.starts_with("test result:")
            || l.starts_with("running ")
            || l.starts_with("STARTING")
            || l.starts_with("Installing")
            || l.starts_with("Installed")
    })
}
