//! Timed execution tracking for commands.
//!
//! Provides the [`TimedExecution`] struct for measuring command execution time
//! and automatically recording token savings.

use std::time::Instant;

use spore::logging::{SpanContext, tool_span};

use super::{Tracker, estimate_tokens};

/// Helper for timing command execution and tracking results.
///
/// Preferred API for tracking commands. Automatically measures execution time
/// and records token savings. Use instead of manual `Tracker::record` calls.
///
/// # Examples
///
/// ```no_run
/// use mycelium::tracking::TimedExecution;
///
/// let timer = TimedExecution::start();
/// let input = "long raw output";
/// let output = "filtered output";
/// timer.track("ls -la", "mycelium ls", input, output);
/// ```
pub struct TimedExecution {
    start: Instant,
}

impl TimedExecution {
    /// Start timing a command execution.
    ///
    /// Creates a new timer that starts measuring elapsed time immediately.
    /// Call [`track`](Self::track) or [`track_passthrough`](Self::track_passthrough)
    /// when the command completes.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::TimedExecution;
    ///
    /// let timer = TimedExecution::start();
    /// // ... execute command ...
    /// timer.track("cmd", "mycelium cmd", "input", "output");
    /// ```
    #[must_use] 
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Track the command with elapsed time and token counts.
    ///
    /// Records the command execution with:
    /// - Elapsed time since [`start`](Self::start)
    /// - Token counts estimated from input/output strings
    /// - Calculated savings metrics
    ///
    /// # Arguments
    ///
    /// - `original_cmd`: Standard command (e.g., "ls -la")
    /// - `mycelium_cmd`: Mycelium command used (e.g., "mycelium ls")
    /// - `input`: Standard command output (for token estimation)
    /// - `output`: Mycelium command output (for token estimation)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::TimedExecution;
    ///
    /// let timer = TimedExecution::start();
    /// let input = "long output...";
    /// let output = "short output";
    /// timer.track("ls -la", "mycelium ls", input, output);
    /// ```
    pub fn track(&self, original_cmd: &str, mycelium_cmd: &str, input: &str, output: &str) {
        let _tool_span = tool_span("tracking_record", &span_context(original_cmd)).entered();
        let elapsed_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let input_tokens = estimate_tokens(input);
        let output_tokens = estimate_tokens(output);

        match Tracker::new() {
            Ok(tracker) => {
                if let Err(e) = tracker.record(
                    original_cmd,
                    mycelium_cmd,
                    input_tokens,
                    output_tokens,
                    elapsed_ms,
                ) {
                    tracing::warn!(error = %e, "tracker.record failed");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "tracker init failed");
            }
        }
    }

    /// Track passthrough commands (timing-only, no token counting).
    ///
    /// For commands that stream output or run interactively where output
    /// cannot be captured. Records execution time but sets tokens to 0
    /// (does not dilute savings statistics).
    ///
    /// # Arguments
    ///
    /// - `original_cmd`: Standard command (e.g., "git tag --list")
    /// - `mycelium_cmd`: Mycelium command used (e.g., "mycelium git tag --list")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mycelium::tracking::TimedExecution;
    ///
    /// let timer = TimedExecution::start();
    /// // ... execute streaming command ...
    /// timer.track_passthrough("git tag", "mycelium git tag");
    /// ```
    pub fn track_passthrough(&self, original_cmd: &str, mycelium_cmd: &str) {
        let _tool_span =
            tool_span("tracking_record_passthrough", &span_context(original_cmd)).entered();
        let elapsed_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        match Tracker::new() {
            Ok(tracker) => {
                if let Err(e) = tracker.record_passthrough(original_cmd, mycelium_cmd, elapsed_ms) {
                    tracing::warn!(error = %e, "tracker.record_passthrough failed");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "tracker init failed");
            }
        }
    }

    /// Track the command with parse tier and format mode.
    ///
    /// Use for commands that use the `OutputParser` framework.
    /// Records parse degradation data for the `parse-health` diagnostic command.
    ///
    /// # Arguments
    ///
    /// - `parse_tier`: 1=Full, 2=Degraded, 3=Passthrough
    /// - `format_mode`: "compact", "verbose", or "ultra"
    pub fn track_with_parse_info(
        &self,
        original_cmd: &str,
        mycelium_cmd: &str,
        input: &str,
        output: &str,
        parse_tier: u8,
        format_mode: &str,
    ) {
        let _tool_span =
            tool_span("tracking_record_parse_info", &span_context(original_cmd)).entered();
        let elapsed_ms = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        let input_tokens = estimate_tokens(input);
        let output_tokens = estimate_tokens(output);

        match Tracker::new() {
            Ok(tracker) => {
                if let Err(e) = tracker.record_with_parse_info(
                    original_cmd,
                    mycelium_cmd,
                    input_tokens,
                    output_tokens,
                    elapsed_ms,
                    parse_tier,
                    format_mode,
                ) {
                    tracing::warn!(error = %e, "tracker.record_with_parse_info failed");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "tracker init failed");
            }
        }
    }
}

fn span_context(command: &str) -> SpanContext {
    let context = SpanContext::for_app("mycelium").with_tool(command.to_string());
    match std::env::current_dir() {
        Ok(path) => context.with_workspace_root(path.display().to_string()),
        Err(_) => context,
    }
}
