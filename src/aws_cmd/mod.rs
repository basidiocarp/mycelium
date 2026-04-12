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
