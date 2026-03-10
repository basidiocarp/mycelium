//! AWS STS – Security Token Service handler.

use crate::tracking;
use anyhow::Result;
use serde_json::Value;

use super::generic::run_aws_json;

/// AWS STS – Security Token Service.
///
/// `args` is the operation and its flags, e.g. `["get-caller-identity"]`.
pub fn run_sts(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("get-caller-identity") => run_sts_identity(&args[1..], verbose),
        _ => super::generic::run_generic("sts", args, verbose),
    }
}

fn run_sts_identity(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) = run_aws_json(&["sts", "get-caller-identity"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws sts get-caller-identity",
            "mycelium aws sts get-caller-identity",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_sts_identity(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws sts get-caller-identity",
        "mycelium aws sts get-caller-identity",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_sts_identity(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let account = v["Account"].as_str().unwrap_or("?");
    let arn = v["Arn"].as_str().unwrap_or("?");
    Some(format!("AWS: {} {}", account, arn))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_sts_identity() {
        let json = r#"{
    "UserId": "AIDAEXAMPLEUSERID1234",
    "Account": "123456789012",
    "Arn": "arn:aws:iam::123456789012:user/dev-user"
}"#;
        let result = filter_sts_identity(json).unwrap();
        assert_eq!(
            result,
            "AWS: 123456789012 arn:aws:iam::123456789012:user/dev-user"
        );
    }

    #[test]
    fn test_filter_sts_identity() {
        let json = r#"{
            "UserId": "AIDAEXAMPLE",
            "Account": "123456789012",
            "Arn": "arn:aws:iam::123456789012:user/dev"
        }"#;
        let result = filter_sts_identity(json).unwrap();
        assert_eq!(
            result,
            "AWS: 123456789012 arn:aws:iam::123456789012:user/dev"
        );
    }

    #[test]
    fn test_filter_sts_identity_missing_fields() {
        let json = r#"{}"#;
        let result = filter_sts_identity(json).unwrap();
        assert_eq!(result, "AWS: ? ?");
    }

    #[test]
    fn test_filter_sts_identity_invalid_json() {
        let result = filter_sts_identity("not json");
        assert!(result.is_none());
    }

    fn count_tokens(text: &str) -> usize {
        text.split_whitespace().count()
    }

    #[test]
    fn test_sts_token_savings() {
        let json = r#"{
    "UserId": "AIDAEXAMPLEUSERID1234",
    "Account": "123456789012",
    "Arn": "arn:aws:iam::123456789012:user/dev-user"
}"#;
        let result = filter_sts_identity(json).unwrap();
        let input_tokens = count_tokens(json);
        let output_tokens = count_tokens(&result);
        let savings = 100.0 - (output_tokens as f64 / input_tokens as f64 * 100.0);
        assert!(
            savings >= 60.0,
            "STS identity filter: expected >=60% savings, got {:.1}%",
            savings
        );
    }
}
