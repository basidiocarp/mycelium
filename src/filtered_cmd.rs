use anyhow::{Context, Result};
use std::process::Command;

/// Builder for the standard mycelium filter-track-exit pattern.
///
/// Abstracts the boilerplate shared by 20+ command modules:
/// timer → run command → strip ANSI (optional) → filter → tee → print → track → exit on failure.
///
/// # Example
/// ```rust,ignore
/// FilteredCommand::new("prettier")
///     .args(args)
///     .verbose(verbose)
///     .filter(|raw| filter_prettier_output(raw))
///     .run()
/// ```
pub struct FilteredCommand {
    tool_name: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    verbose: u8,
    tee_slug: Option<String>,
    mycelium_label: Option<String>,
    filter_fn: Box<dyn Fn(&str) -> String>,
    do_strip_ansi: bool,
}

impl FilteredCommand {
    /// Create a new builder for the given tool.
    /// `tool_name` is used for: `Command::new()`, verbose prefix, default tee slug,
    /// and default tracking labels.
    pub fn new(tool_name: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            args: Vec::new(),
            envs: Vec::new(),
            verbose: 0,
            tee_slug: None,
            mycelium_label: None,
            filter_fn: Box::new(|s: &str| s.to_string()),
            do_strip_ansi: false,
        }
    }

    /// Set all args at once (replaces any previously set args).
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Append a single arg.
    #[cfg(test)]
    pub fn arg<S: Into<String>>(mut self, arg: S) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set an environment variable for the spawned process.
    pub fn env(mut self, key: &str, val: &str) -> Self {
        self.envs.push((key.to_string(), val.to_string()));
        self
    }

    /// Set verbosity level (0 = silent, 1+ = print "Running: tool args" to stderr).
    pub fn verbose(mut self, level: u8) -> Self {
        self.verbose = level;
        self
    }

    /// Override the tee slug (default: tool_name).
    /// Used for compound commands: e.g., "cargo_test" instead of "cargo".
    pub fn tee_slug(mut self, slug: &str) -> Self {
        self.tee_slug = Some(slug.to_string());
        self
    }

    /// Override the "mycelium ..." label used in token tracking.
    pub fn mycelium_label(mut self, label: &str) -> Self {
        self.mycelium_label = Some(label.to_string());
        self
    }

    /// Set the filter function applied to raw output.
    /// Default: identity (returns raw unchanged).
    pub fn filter<F: Fn(&str) -> String + 'static>(mut self, f: F) -> Self {
        self.filter_fn = Box::new(f);
        self
    }

    /// Strip ANSI codes from raw output before filtering (default: false).
    pub fn strip_ansi(mut self, yes: bool) -> Self {
        self.do_strip_ansi = yes;
        self
    }

    /// Execute: timer → run command → filter → tee → print → track → exit on failure.
    pub fn run(self) -> Result<()> {
        use crate::{tee, tracking, utils};

        let timer = tracking::TimedExecution::start();

        if self.verbose > 0 {
            eprintln!("Running: {} {}", self.tool_name, self.args.join(" "));
        }

        let output = Command::new(&self.tool_name)
            .args(&self.args)
            .envs(self.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .with_context(|| format!("Failed to run {}", self.tool_name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let raw = if self.do_strip_ansi {
            utils::strip_ansi(&combined)
        } else {
            combined
        };

        let filtered = (self.filter_fn)(&raw);
        let exit_code = utils::exit_code(&output.status);

        let raw_label = format!("{} {}", self.tool_name, self.args.join(" "));
        let mycelium_label = self
            .mycelium_label
            .unwrap_or_else(|| format!("mycelium {} {}", self.tool_name, self.args.join(" ")));
        let slug = self.tee_slug.unwrap_or_else(|| self.tool_name.clone());

        if let Some(hint) = tee::tee_and_hint(&raw, &slug, exit_code) {
            println!("{}\n{}", filtered, hint);
        } else {
            println!("{}", filtered);
        }

        timer.track(&raw_label, &mycelium_label, &raw, &filtered);

        if exit_code != 0 {
            std::process::exit(exit_code);
        }

        Ok(())
    }

    /// Execute and return (raw, filtered) instead of printing.
    /// Does NOT call `std::process::exit` — caller handles exit code.
    /// Does NOT call tee or print output.
    #[cfg(test)]
    pub fn run_capturing(self) -> Result<(String, String)> {
        use crate::{tracking, utils};

        let timer = tracking::TimedExecution::start();

        if self.verbose > 0 {
            eprintln!("Running: {} {}", self.tool_name, self.args.join(" "));
        }

        let output = Command::new(&self.tool_name)
            .args(&self.args)
            .envs(self.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .with_context(|| format!("Failed to run {}", self.tool_name))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}{}", stdout, stderr);

        let raw = if self.do_strip_ansi {
            utils::strip_ansi(&combined)
        } else {
            combined
        };

        let filtered = (self.filter_fn)(&raw);

        let raw_label = format!("{} {}", self.tool_name, self.args.join(" "));
        let mycelium_label = self
            .mycelium_label
            .unwrap_or_else(|| format!("mycelium {} {}", self.tool_name, self.args.join(" ")));

        timer.track(&raw_label, &mycelium_label, &raw, &filtered);

        Ok((raw, filtered))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default_filter_is_identity() {
        let cmd = FilteredCommand::new("echo");
        let result = (cmd.filter_fn)("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_builder_custom_filter_applied() {
        let cmd = FilteredCommand::new("echo").filter(|s| s.to_uppercase());
        let result = (cmd.filter_fn)("hello");
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_builder_args_set() {
        let cmd = FilteredCommand::new("echo").args(["--flag", "value"]);
        assert_eq!(cmd.args, vec!["--flag", "value"]);
    }

    #[test]
    fn test_builder_arg_appended() {
        let cmd = FilteredCommand::new("echo").arg("first").arg("second");
        assert_eq!(cmd.args, vec!["first", "second"]);
    }

    #[test]
    fn test_run_capturing_with_echo() {
        let (raw, filtered) = FilteredCommand::new("echo")
            .arg("hello")
            .filter(|s| s.trim().to_uppercase())
            .run_capturing()
            .unwrap();
        assert!(raw.contains("hello"));
        assert!(filtered.contains("HELLO"));
    }
}
