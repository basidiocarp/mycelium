//! Go subcommand handlers for test (NDJSON), build, and vet with token-efficient output.
pub mod filters;

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::process::Command;

pub use filters::{filter_go_build, filter_go_test_json, filter_go_vet};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct GoTestEvent {
    #[serde(rename = "Time")]
    pub time: Option<String>,
    #[serde(rename = "Action")]
    pub action: String,
    #[serde(rename = "Package")]
    pub package: Option<String>,
    #[serde(rename = "Test")]
    pub test: Option<String>,
    #[serde(rename = "Output")]
    pub output: Option<String>,
    #[serde(rename = "Elapsed")]
    pub elapsed: Option<f64>,
    #[serde(rename = "ImportPath")]
    pub import_path: Option<String>,
    #[serde(rename = "FailedBuild")]
    pub failed_build: Option<String>,
}

#[derive(Debug, Default)]
pub struct PackageResult {
    pub pass: usize,
    pub fail: usize,
    pub skip: usize,
    pub build_failed: bool,
    pub build_errors: Vec<String>,
    pub failed_tests: Vec<(String, Vec<String>)>, // (test_name, output_lines)
}

/// Execute `go test` with NDJSON parsing and token-efficient summary output.
pub fn run_test(args: &[String], verbose: u8) -> Result<()> {
    // Build the args list with -json flag injected.
    let mut go_args: Vec<&str> = vec!["test"];
    let use_json = !args.iter().any(|a| a == "-json");
    if use_json {
        go_args.push("-json");
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    go_args.extend_from_slice(&arg_refs);

    if verbose > 0 {
        eprintln!("Running: go {}", go_args.join(" "));
    }

    // Stream NDJSON lines silently — go test JSON must be batched for parsing,
    // so we suppress per-line output and print the summary once all lines are in.
    let stream = crate::streaming::execute_streaming("go", &go_args, |_| None)
        .context("Failed to run go test. Is Go installed?")?;

    // The raw field contains stdout lines joined; extract the JSON portion for filtering.
    let filtered = filter_go_test_json(&stream.raw);

    if let Some(hint) = crate::tee::tee_and_hint(&stream.raw, "go_test", stream.exit_code) {
        println!("{}\n{}", filtered, hint);
    } else {
        println!("{}", filtered);
    }

    if stream.exit_code != 0 {
        std::process::exit(stream.exit_code);
    }

    Ok(())
}

/// Execute `go build` and filter output to show only errors.
pub fn run_build(args: &[String], verbose: u8) -> Result<()> {
    let mut cmd = Command::new("go");
    cmd.arg("build");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: go build {}", args.join(" "));
    }

    let output = cmd
        .output()
        .context("Failed to run go build. Is Go installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let exit_code = output
        .status
        .code()
        .unwrap_or(if output.status.success() { 0 } else { 1 });
    let filtered = filter_go_build(&raw);

    if let Some(hint) = crate::tee::tee_and_hint(&raw, "go_build", exit_code) {
        if !filtered.is_empty() {
            println!("{}\n{}", filtered, hint);
        } else {
            println!("{}", hint);
        }
    } else if !filtered.is_empty() {
        println!("{}", filtered);
    }

    // Preserve exit code for CI/CD
    if !output.status.success() {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Execute `go vet` and filter output to show only issues.
pub fn run_vet(args: &[String], verbose: u8) -> Result<()> {
    let mut cmd = Command::new("go");
    cmd.arg("vet");

    for arg in args {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: go vet {}", args.join(" "));
    }

    let output = cmd
        .output()
        .context("Failed to run go vet. Is Go installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let raw = format!("{}\n{}", stdout, stderr);

    let exit_code = output
        .status
        .code()
        .unwrap_or(if output.status.success() { 0 } else { 1 });
    let filtered = filter_go_vet(&raw);

    if let Some(hint) = crate::tee::tee_and_hint(&raw, "go_vet", exit_code) {
        if !filtered.is_empty() {
            println!("{}\n{}", filtered, hint);
        } else {
            println!("{}", hint);
        }
    } else if !filtered.is_empty() {
        println!("{}", filtered);
    }

    // Preserve exit code for CI/CD
    if !output.status.success() {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Execute an unrecognized `go` subcommand as a passthrough.
pub fn run_other(args: &[OsString], verbose: u8) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("go: no subcommand specified");
    }

    let subcommand = args[0].to_string_lossy();
    let mut cmd = Command::new("go");
    cmd.arg(&*subcommand);

    for arg in &args[1..] {
        cmd.arg(arg);
    }

    if verbose > 0 {
        eprintln!("Running: go {} ...", subcommand);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run go {}", subcommand))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _raw = format!("{}\n{}", stdout, stderr);

    print!("{}", stdout);
    eprint!("{}", stderr);

    // Preserve exit code
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}
