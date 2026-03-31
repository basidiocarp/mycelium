//! AWS RDS – Relational Database Service handler.

use crate::tracking;
use crate::utils::join_with_overflow;
use anyhow::Result;
use serde_json::Value;

use super::generic::{MAX_ITEMS, run_aws_json};

/// AWS RDS – Relational Database Service.
///
/// `args` is the operation and its flags, e.g. `["describe-db-instances"]`.
pub fn run_rds(args: &[String], verbose: u8) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("describe-db-instances") => run_rds_describe(&args[1..], verbose),
        _ => super::generic::run_generic("rds", args, verbose),
    }
}

fn run_rds_describe(extra_args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();
    let (raw, stderr, status) =
        run_aws_json(&["rds", "describe-db-instances"], extra_args, verbose)?;

    if !status.success() {
        timer.track(
            "aws rds describe-db-instances",
            "mycelium aws rds describe-db-instances",
            &stderr,
            &stderr,
        );
        std::process::exit(status.code().unwrap_or(1));
    }

    let filtered = match filter_rds_instances(&raw) {
        Some(f) => f,
        None => raw.clone(),
    };
    println!("{}", filtered);

    timer.track(
        "aws rds describe-db-instances",
        "mycelium aws rds describe-db-instances",
        &raw,
        &filtered,
    );
    Ok(())
}

fn filter_rds_instances(json_str: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_str).ok()?;
    let dbs = v["DBInstances"].as_array()?;

    let mut result = Vec::new();
    let total = dbs.len();

    for db in dbs.iter().take(MAX_ITEMS) {
        let name = db["DBInstanceIdentifier"].as_str().unwrap_or("?");
        let engine = db["Engine"].as_str().unwrap_or("?");
        let version = db["EngineVersion"].as_str().unwrap_or("?");
        let class = db["DBInstanceClass"].as_str().unwrap_or("?");
        let status = db["DBInstanceStatus"].as_str().unwrap_or("?");
        result.push(format!(
            "{} {} {} {} {}",
            name, engine, version, class, status
        ));
    }

    Some(join_with_overflow(&result, total, MAX_ITEMS, "instances"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_rds_instances() {
        let json = r#"{
            "DBInstances": [{
                "DBInstanceIdentifier": "mydb",
                "Engine": "postgres",
                "EngineVersion": "15.4",
                "DBInstanceClass": "db.t3.micro",
                "DBInstanceStatus": "available"
            }]
        }"#;
        let result = filter_rds_instances(json).unwrap();
        assert_eq!(result, "mydb postgres 15.4 db.t3.micro available");
    }

    #[test]
    fn test_rds_overflow() {
        let mut dbs = Vec::new();
        for i in 1..=25 {
            dbs.push(format!(
                r#"{{"DBInstanceIdentifier": "db-{}", "Engine": "postgres", "EngineVersion": "15.4", "DBInstanceClass": "db.t3.micro", "DBInstanceStatus": "available"}}"#,
                i
            ));
        }
        let json = format!(r#"{{"DBInstances": [{}]}}"#, dbs.join(","));
        let result = filter_rds_instances(&json).unwrap();
        assert!(result.contains("... +5 more instances"));
    }

    #[test]
    fn test_filter_rds_instances_token_savings() {
        fn count_tokens(text: &str) -> usize {
            crate::tracking::estimate_tokens(text)
        }

        let input = r#"{
            "DBInstances": [
                {
                    "DBInstanceIdentifier": "production-db",
                    "Engine": "postgres",
                    "EngineVersion": "15.4",
                    "DBInstanceClass": "db.r6g.2xlarge",
                    "DBInstanceStatus": "available",
                    "BackupRetentionPeriod": 30,
                    "AllocatedStorage": 500,
                    "StorageType": "gp3",
                    "Endpoint": {"Address":"prod.abc123.us-east-1.rds.amazonaws.com","Port":5432}
                },
                {
                    "DBInstanceIdentifier": "staging-db",
                    "Engine": "mysql",
                    "EngineVersion": "8.0.35",
                    "DBInstanceClass": "db.t3.large",
                    "DBInstanceStatus": "available",
                    "BackupRetentionPeriod": 7,
                    "AllocatedStorage": 100,
                    "StorageType": "gp2",
                    "Endpoint": {"Address":"staging.xyz789.us-east-1.rds.amazonaws.com","Port":3306}
                },
                {
                    "DBInstanceIdentifier": "analytics-db",
                    "Engine": "postgres",
                    "EngineVersion": "14.7",
                    "DBInstanceClass": "db.r6g.4xlarge",
                    "DBInstanceStatus": "available",
                    "BackupRetentionPeriod": 90,
                    "AllocatedStorage": 1000,
                    "StorageType": "gp3",
                    "Endpoint": {"Address":"analytics.def456.us-east-1.rds.amazonaws.com","Port":5432}
                }
            ]
        }"#;

        let output = filter_rds_instances(input).unwrap();
        let savings = (count_tokens(input).saturating_sub(count_tokens(&output))) * 100
            / count_tokens(input).max(1);
        assert!(
            savings >= 60,
            "RDS describe-db-instances filter: expected >= 60% token savings, got {}%",
            savings
        );
    }
}
