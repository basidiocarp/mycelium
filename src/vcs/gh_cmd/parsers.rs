//! JSON deserialization structs and OutputParser implementations for GitHub CLI.

use super::markdown::filter_markdown_body;
use crate::parser::types::{
    GhIssueDetail, GhIssueItem, GhIssueList, GhRepoDetail, GhRunItem, GhRunList, GhRunViewSummary,
};
use crate::parser::{OutputParser, ParseResult};

// ---------------------------------------------------------------------------
// JSON deserialization structs (private to this module)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub(super) struct GhIssueJson {
    pub(super) number: i64,
    pub(super) title: String,
    pub(super) state: String,
    pub(super) author: GhAuthorJson,
}

#[derive(serde::Deserialize)]
pub(super) struct GhAuthorJson {
    pub(super) login: String,
}

#[derive(serde::Deserialize)]
pub(super) struct GhIssueDetailJson {
    pub(super) number: i64,
    pub(super) title: String,
    pub(super) state: String,
    pub(super) author: GhAuthorJson,
    pub(super) body: Option<String>,
    pub(super) url: String,
}

#[derive(serde::Deserialize)]
pub(super) struct GhRunJson {
    #[serde(rename = "databaseId")]
    pub(super) database_id: i64,
    pub(super) name: String,
    pub(super) status: String,
    pub(super) conclusion: Option<String>,
}

#[derive(serde::Deserialize)]
pub(super) struct GhRepoJson {
    pub(super) name: String,
    pub(super) owner: GhAuthorJson,
    pub(super) description: Option<String>,
    pub(super) url: String,
    #[serde(rename = "stargazerCount")]
    pub(super) stargazer_count: i64,
    #[serde(rename = "forkCount")]
    pub(super) fork_count: i64,
    #[serde(rename = "isPrivate")]
    pub(super) is_private: bool,
}

// ---------------------------------------------------------------------------
// OutputParser implementations
// ---------------------------------------------------------------------------

/// Parser for `gh issue list` JSON output.
pub struct GhIssueListParser;

impl OutputParser for GhIssueListParser {
    type Output = GhIssueList;

    fn parse(input: &str) -> ParseResult<GhIssueList> {
        match serde_json::from_str::<Vec<GhIssueJson>>(input) {
            Ok(items) => ParseResult::Full(GhIssueList {
                issues: items
                    .into_iter()
                    .map(|i| GhIssueItem {
                        number: i.number,
                        title: i.title,
                        state: i.state,
                        author: i.author.login,
                    })
                    .collect(),
            }),
            Err(_) => ParseResult::Passthrough(crate::parser::truncate_output(input, 2000)),
        }
    }
}

/// Parser for `gh issue view` JSON output.
pub struct GhIssueViewParser;

impl OutputParser for GhIssueViewParser {
    type Output = GhIssueDetail;

    fn parse(input: &str) -> ParseResult<GhIssueDetail> {
        match serde_json::from_str::<GhIssueDetailJson>(input) {
            Ok(j) => {
                let body = j.body.unwrap_or_default();
                let filtered_body = filter_markdown_body(&body);
                ParseResult::Full(GhIssueDetail {
                    number: j.number,
                    title: j.title,
                    state: j.state,
                    author: j.author.login,
                    url: j.url,
                    body: filtered_body,
                })
            }
            Err(_) => ParseResult::Passthrough(crate::parser::truncate_output(input, 2000)),
        }
    }
}

/// Parser for `gh run list` JSON output.
pub struct GhRunListParser;

impl OutputParser for GhRunListParser {
    type Output = GhRunList;

    fn parse(input: &str) -> ParseResult<GhRunList> {
        match serde_json::from_str::<Vec<GhRunJson>>(input) {
            Ok(items) => ParseResult::Full(GhRunList {
                runs: items
                    .into_iter()
                    .map(|r| GhRunItem {
                        id: r.database_id,
                        name: r.name,
                        status: r.status,
                        conclusion: r.conclusion,
                    })
                    .collect(),
            }),
            Err(_) => ParseResult::Passthrough(crate::parser::truncate_output(input, 2000)),
        }
    }
}

/// Parser for `gh run view` text output.
pub struct GhRunViewParser;

impl OutputParser for GhRunViewParser {
    type Output = GhRunViewSummary;

    fn parse(input: &str) -> ParseResult<GhRunViewSummary> {
        let mut status = None;
        let mut conclusion = None;
        let mut failed_jobs = Vec::new();
        let mut in_jobs = false;
        let mut saw_useful_line = false;

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(value) = trimmed.strip_prefix("Status:") {
                status = Some(value.trim().to_string());
                saw_useful_line = true;
                continue;
            }
            if let Some(value) = trimmed.strip_prefix("Conclusion:") {
                conclusion = Some(value.trim().to_string());
                saw_useful_line = true;
                continue;
            }
            if trimmed == "JOBS" {
                in_jobs = true;
                saw_useful_line = true;
                continue;
            }
            if in_jobs
                && (trimmed.contains('✗') || trimmed.contains('X') || trimmed.contains("fail"))
            {
                failed_jobs.push(trimmed.to_string());
                saw_useful_line = true;
            }
        }

        if !saw_useful_line {
            return ParseResult::Passthrough(crate::parser::truncate_output(input, 2000));
        }

        ParseResult::Full(GhRunViewSummary {
            run_id: None,
            status,
            conclusion,
            failed_jobs,
        })
    }
}

/// Parser for `gh repo view` JSON output.
pub struct GhRepoViewParser;

impl OutputParser for GhRepoViewParser {
    type Output = GhRepoDetail;

    fn parse(input: &str) -> ParseResult<GhRepoDetail> {
        match serde_json::from_str::<GhRepoJson>(input) {
            Ok(j) => ParseResult::Full(GhRepoDetail {
                owner: j.owner.login,
                name: j.name,
                description: j.description.unwrap_or_default(),
                url: j.url,
                stars: j.stargazer_count,
                forks: j.fork_count,
                private: j.is_private,
            }),
            Err(_) => ParseResult::Passthrough(crate::parser::truncate_output(input, 2000)),
        }
    }
}
