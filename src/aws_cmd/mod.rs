//! AWS CLI output compression.
//!
//! Replaces verbose `--output table`/`text` with JSON, then compresses.
//! Specialized filters for high-frequency commands (STS, S3, EC2, ECS, RDS, CloudFormation).

mod cloudformation;
mod ec2;
mod ecs;
pub(crate) mod generic;
mod rds;
mod s3;
mod sts;

pub use cloudformation::run_cloudformation;
pub use ec2::run_ec2;
pub use ecs::run_ecs;
pub use generic::run_generic;
pub use rds::run_rds;
pub use s3::run_s3;
pub use sts::run_sts;

/// Run an AWS CLI command with token-optimized output.
///
/// Delegates to the per-service entry points below.
#[allow(dead_code)]
pub fn run(subcommand: &str, args: &[String], verbose: u8) -> anyhow::Result<()> {
    match subcommand {
        "sts" => run_sts(args, verbose),
        "s3" => run_s3(args, verbose),
        "ec2" => run_ec2(args, verbose),
        "ecs" => run_ecs(args, verbose),
        "rds" => run_rds(args, verbose),
        "cloudformation" => run_cloudformation(args, verbose),
        _ => run_generic(subcommand, args, verbose),
    }
}
