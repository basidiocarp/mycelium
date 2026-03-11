//! AWS CloudFormation handler.

use crate::tracking;
use crate::utils::{join_with_overflow, truncate_iso_date};
use anyhow::Result;
use serde_json::Value;

use super::generic::{MAX_ITEMS, run_aws_json};

/// AWS CloudFormation.
///
/// `args` is the operation and its flags, e.g. `["list-stacks"]`.
pub fn run_cloudformation(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list-stacks") => run_cfn_list_stacks(&args[1..], verbose),
        Some("describe-stacks") => run_cfn_describe_stacks(&args[1..], verbose),
        _ => super::generic::run_generic("cloudformation", args, verbose),
    }
}

fn run_cfn_list_stacks(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) =
        run_aws_json(&["cloudformation", "list-stacks"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws cloudformation list-stacks",
            "mycelium aws cloudformation list-stacks",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_cfn_list_stacks(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws cloudformation list-stacks",
        "mycelium aws cloudformation list-stacks",
        &raw,
        &filtered,
    );
    Ok(())
}

fn run_cfn_describe_stacks(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) =
        run_aws_json(&["cloudformation", "describe-stacks"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws cloudformation describe-stacks",
            "mycelium aws cloudformation describe-stacks",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_cfn_describe_stacks(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws cloudformation describe-stacks",
        "mycelium aws cloudformation describe-stacks",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_cfn_list_stacks(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let stacks = v["StackSummaries"].as_array()?;

    let mut result = Vec::new();
    let total = stacks.len();

    for stack in stacks.iter().take(MAX_ITEMS) {
        let name = stack["StackName"].as_str().unwrap_or("?");
        let status = stack["StackStatus"].as_str().unwrap_or("?");
        let date = stack["LastUpdatedTime"]
            .as_str()
            .or_else(|| stack["CreationTime"].as_str())
            .unwrap_or("?");
        result.push(format!("{} {} {}", name, status, truncate_iso_date(date)));
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "stacks"))
}

fn filter_cfn_describe_stacks(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let stacks = v["Stacks"].as_array()?;

    let mut result = Vec::new();
    let total = stacks.len();

    for stack in stacks.iter().take(MAX_ITEMS) {
        let name = stack["StackName"].as_str().unwrap_or("?");
        let status = stack["StackStatus"].as_str().unwrap_or("?");
        let date = stack["LastUpdatedTime"]
            .as_str()
            .or_else(|| stack["CreationTime"].as_str())
            .unwrap_or("?");
        result.push(format!("{} {} {}", name, status, truncate_iso_date(date)));

        // Show outputs if present
        if let Some(outputs) = stack["Outputs"].as_array() {
            for out in outputs {
                let key = out["OutputKey"].as_str().unwrap_or("?");
                let val = out["OutputValue"].as_str().unwrap_or("?");
                result.push(format!("  {}={}", key, val));
            }
        }
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "stacks"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_cfn_list_stacks() {
        let json = r#"{
            "StackSummaries": [{
                "StackName": "my-stack",
                "StackStatus": "CREATE_COMPLETE",
                "CreationTime": "2024-01-15T10:30:00Z"
            }, {
                "StackName": "other-stack",
                "StackStatus": "UPDATE_COMPLETE",
                "LastUpdatedTime": "2024-02-20T14:00:00Z",
                "CreationTime": "2024-01-01T00:00:00Z"
            }]
        }"#;
        let result = filter_cfn_list_stacks(json).unwrap();
        assert!(result.contains("my-stack CREATE_COMPLETE 2024-01-15"));
        assert!(result.contains("other-stack UPDATE_COMPLETE 2024-02-20"));
    }

    #[test]
    fn test_filter_cfn_describe_stacks_with_outputs() {
        let json = r#"{
            "Stacks": [{
                "StackName": "my-stack",
                "StackStatus": "CREATE_COMPLETE",
                "CreationTime": "2024-01-15T10:30:00Z",
                "Outputs": [
                    {"OutputKey": "ApiUrl", "OutputValue": "https://api.example.com"},
                    {"OutputKey": "BucketName", "OutputValue": "my-bucket"}
                ]
            }]
        }"#;
        let result = filter_cfn_describe_stacks(json).unwrap();
        assert!(result.contains("my-stack CREATE_COMPLETE 2024-01-15"));
        assert!(result.contains("ApiUrl=https://api.example.com"));
        assert!(result.contains("BucketName=my-bucket"));
    }

    #[test]
    fn test_filter_cfn_describe_stacks_no_outputs() {
        let json = r#"{
            "Stacks": [{
                "StackName": "my-stack",
                "StackStatus": "CREATE_COMPLETE",
                "CreationTime": "2024-01-15T10:30:00Z"
            }]
        }"#;
        let result = filter_cfn_describe_stacks(json).unwrap();
        assert!(result.contains("my-stack CREATE_COMPLETE 2024-01-15"));
        assert!(!result.contains("="));
    }

    #[test]
    fn test_filter_cfn_list_stacks_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        let input = r#"{
            "StackSummaries": [
                {
                    "StackName": "api-infrastructure",
                    "StackStatus": "CREATE_COMPLETE",
                    "CreationTime": "2024-01-10T08:30:00Z",
                    "LastUpdatedTime": "2024-02-15T10:30:00Z",
                    "StackId": "arn:aws:cloudformation:us-east-1:123456789:stack/api-infrastructure/abc-def-ghi",
                    "TemplateDescription": "Production API infrastructure with ECS and RDS"
                },
                {
                    "StackName": "database-backup",
                    "StackStatus": "UPDATE_COMPLETE",
                    "CreationTime": "2024-01-05T14:20:00Z",
                    "LastUpdatedTime": "2024-02-20T14:00:00Z",
                    "StackId": "arn:aws:cloudformation:us-east-1:123456789:stack/database-backup/xyz-pqr-stu",
                    "TemplateDescription": "RDS database backup infrastructure"
                }
            ]
        }"#;

        let output = filter_cfn_list_stacks(input).unwrap();
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 60,
            "CloudFormation list-stacks filter: expected >= 60% token savings, got {}%",
            savings
        );
    }

    #[test]
    fn test_filter_cfn_describe_stacks_token_savings() {
        fn count_tokens(text: &str) -> usize {
            text.split_whitespace().count()
        }

        let input = r#"{
            "Stacks": [
                {
                    "StackName": "api-infrastructure",
                    "StackStatus": "CREATE_COMPLETE",
                    "CreationTime": "2024-01-10T08:30:00Z",
                    "LastUpdatedTime": "2024-02-15T10:30:00Z",
                    "StackId": "arn:aws:cloudformation:us-east-1:123456789:stack/api-infrastructure/abc",
                    "StackStatusReason": "Stack successfully created",
                    "Outputs": [
                        {"OutputKey": "ApiEndpoint", "OutputValue": "https://api.example.com", "Description": "API Gateway endpoint"},
                        {"OutputKey": "DatabaseHost", "OutputValue": "prod.abc123.us-east-1.rds.amazonaws.com", "Description": "RDS database endpoint"}
                    ],
                    "Parameters": [
                        {"ParameterKey": "Environment", "ParameterValue": "production"},
                        {"ParameterKey": "InstanceType", "ParameterValue": "t3.large"}
                    ]
                },
                {
                    "StackName": "database-backup",
                    "StackStatus": "UPDATE_COMPLETE",
                    "CreationTime": "2024-01-05T14:20:00Z",
                    "LastUpdatedTime": "2024-02-20T14:00:00Z",
                    "StackId": "arn:aws:cloudformation:us-east-1:123456789:stack/database-backup/xyz",
                    "StackStatusReason": "Stack update completed",
                    "Outputs": [
                        {"OutputKey": "BackupBucket", "OutputValue": "backup-prod-bucket-12345", "Description": "S3 bucket for backups"},
                        {"OutputKey": "BackupRole", "OutputValue": "arn:aws:iam::123456789:role/backup-role", "Description": "IAM role for backup"}
                    ]
                }
            ]
        }"#;

        let output = filter_cfn_describe_stacks(input).unwrap();
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 60,
            "CloudFormation describe-stacks filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
