use anyhow::{Context, Result};

use crate::{rewrite_cmd, tracking};

/// Maximum bytes to capture from command stdout (64 MB).
/// This constant is imported from exec.rs via super:: reference.
use super::exec::MAX_STDOUT_CAPTURE;
use super::exec::MAX_STDERR_CAPTURE;

pub(super) fn run_spawned_command(
    mut command: std::process::Command,
    tracked_input: &str,
    tracked_output: &str,
    timer: tracking::TimedExecution,
) -> Result<()> {
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::thread;

    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute command")?;

    let stdout_pipe = child
        .stdout
        .take()
        .context("Failed to capture child stdout")?;
    let stderr_pipe = child
        .stderr
        .take()
        .context("Failed to capture child stderr")?;

    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stdout_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < MAX_STDOUT_CAPTURE {
                let remaining = MAX_STDOUT_CAPTURE - captured.len();
                captured.extend_from_slice(&buf[..count.min(remaining)]);
            }
            // Continue draining stdout even after cap is hit, but discard bytes past the cap
        }

        Ok(captured)
    });

    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stderr_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < MAX_STDERR_CAPTURE {
                let remaining = MAX_STDERR_CAPTURE - captured.len();
                captured.extend_from_slice(&buf[..count.min(remaining)]);
            }
            // Continue draining stderr even after cap is hit, but discard bytes past the cap
            let mut err = std::io::stderr().lock();
            let _ = err.write_all(&buf[..count]);
            let _ = err.flush();
        }

        Ok(captured)
    });

    let status = child.wait().context("Failed waiting for command")?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
    // Join the stderr thread to avoid leaving it detached; bytes are unused because
    // stderr is delivered in real-time by the passthrough thread and excluded from filtering.
    let _stderr_bytes = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;

    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let full_output = stdout.to_string();

    // Route raw stdout through hyphae for chunked storage before any ContentRouter filtering.
    // This ensures hyphae stores the complete unfiltered command output.
    let hyphae_output = if crate::hyphae::is_available() {
        let result = crate::hyphae::route_or_filter(&tracked_input, &stdout, |r| {
            crate::filter::FilterResult::full(r, r.to_string())
        });
        result.output
    } else {
        // Hyphae not available — use raw output for display if no ContentRouter filter applies
        stdout.to_string()
    };

    // Apply content-aware routing to stdout for display filtering.
    // Built-in (compiled) filter runs first and always takes precedence.
    let router = super::content_router::ContentRouter::default();
    let routed_stdout = router.route(&stdout);

    // Apply TOML declarative filter as a fallback when the built-in filter
    // produced no transformation. Load filters once here to avoid repeated
    // disk reads inside tight loops; cwd is captured at command-dispatch time.
    let (filtered_for_display, matched_filter_name) = if routed_stdout == stdout.as_ref() {
        // Built-in filter was a no-op — consult TOML filters.
        let (project_filters, user_filters) = crate::filters::load_all_declarative_filters();
        if let Some(matched) =
            crate::filters::find_matching_filter(tracked_input, &project_filters, &user_filters)
        {
            let filter_name = matched.command.clone();
            let toml_result = matched.apply(&stdout);
            (toml_result.output, Some(filter_name))
        } else {
            (routed_stdout, None)
        }
    } else {
        (routed_stdout, None)
    };

    // Choose what to print: if hyphae handled it, use hyphae's summary; otherwise use the
    // ContentRouter-filtered text for display.
    let output_to_print = if crate::hyphae::is_available() {
        use tracing::debug;
        debug!(source = "hyphae", "using hyphae summary as display output");
        hyphae_output.clone()
    } else {
        use tracing::debug;
        debug!(
            source = "content_router",
            matched_filter = matched_filter_name.as_deref(),
            input_bytes = stdout.len(),
            output_bytes = filtered_for_display.len(),
            "using content-router output"
        );
        filtered_for_display.to_string()
    };

    // When hyphae is active, update filtered_for_display to match output_to_print
    // so token savings tracking uses the actual output sent to the model.
    let filtered_for_display = if crate::hyphae::is_available() {
        hyphae_output
    } else {
        filtered_for_display.to_string()
    };

    // Print the final output after all filtering is complete.
    // Stderr is already streamed live by the capture thread above.
    print!("{output_to_print}");

    // Append MYCELIUM_EXPLAIN annotation if enabled and command was rewritten
    if std::env::var("MYCELIUM_EXPLAIN").is_ok() {
        let resolution = rewrite_cmd::resolve_runtime_command(tracked_input);
        if resolution.rewritten {
            if !output_to_print.is_empty() {
                println!();
            }
            print!(
                "[mycelium] filter: {} | savings: ~{:.0}%",
                resolution.source,
                resolution.estimated_savings_pct.unwrap_or(0.0)
            );
        }
    }

    // Track using the actual output that was displayed (hyphae summary if active, else ContentRouter-filtered).
    // This ensures token savings are computed against the real output users see.
    // Stderr is excluded from both sides — it is streamed live and not part of the filtered display output.
    timer.track(tracked_input, tracked_output, &full_output, &filtered_for_display);

    if !status.success() {
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
