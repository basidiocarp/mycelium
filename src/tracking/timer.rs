//! Timed execution tracking for commands.
//!
//! Provides the [`TimedExecution`] struct for measuring command execution time
//! and automatically recording token savings.

use std::time::Instant;

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
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        let input_tokens = estimate_tokens(input);
        let output_tokens = estimate_tokens(output);

        if let Ok(tracker) = Tracker::new() {
            let _ = tracker.record(
                original_cmd,
                mycelium_cmd,
                input_tokens,
                output_tokens,
                elapsed_ms,
            );
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
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        // input_tokens=0, output_tokens=0 won't dilute savings statistics
        if let Ok(tracker) = Tracker::new() {
            let _ = tracker.record(original_cmd, mycelium_cmd, 0, 0, elapsed_ms);
        }
    }

    /// Track the command with parse tier and format mode.
    ///
    /// Use for commands that use the OutputParser framework.
    /// Records parse degradation data for the `parse-health` diagnostic command.
    ///
    /// # Arguments
    ///
    /// - `parse_tier`: 1=Full, 2=Degraded, 3=Passthrough
    /// - `format_mode`: "compact", "verbose", or "ultra"
    #[allow(dead_code)]
    pub fn track_with_parse_info(
        &self,
        original_cmd: &str,
        mycelium_cmd: &str,
        input: &str,
        output: &str,
        parse_tier: u8,
        format_mode: &str,
    ) {
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        let input_tokens = estimate_tokens(input);
        let output_tokens = estimate_tokens(output);

        if let Ok(tracker) = Tracker::new() {
            let _ = tracker.record_with_parse_info(
                original_cmd,
                mycelium_cmd,
                input_tokens,
                output_tokens,
                elapsed_ms,
                parse_tier,
                format_mode,
            );
        }
    }
}
