//! Data structures for (de)serializing GitHub's GraphQL and REST API payloads.
use arduino_report_size_deltas::COMMENT_MARKER;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct GraphQlResponse<T> {
    pub data: Option<T>,
    #[serde(default)]
    pub errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
pub struct GraphQlError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenPullRequestsData {
    pub repository: Repository,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub pull_requests: Connection<PullRequest>,
}

#[derive(Debug, Deserialize)]
pub struct Connection<T> {
    pub nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Nodes<T> {
    pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub locked: bool,
    pub head_ref_oid: String,
    pub comments: Nodes<Comment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub database_id: u64,
    pub body: String,
}

impl PullRequest {
    /// The most recent comment posted by this tool (if any).
    pub fn report_comment(&self) -> Option<&Comment> {
        self.comments
            .nodes
            .iter()
            .rev()
            .find(|c| c.body.starts_with(COMMENT_MARKER))
    }

    /// Does an existing report comment already describe the PR's latest commit?
    pub fn is_up_to_date(&self) -> bool {
        self.report_comment()
            .is_some_and(|c| c.body.contains(&format!("@ {}", self.head_ref_oid)))
    }
}

#[derive(Debug, Deserialize)]
pub struct ArtifactList {
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Deserialize)]
pub struct Artifact {
    pub expired: bool,
    pub archive_download_url: String,
    pub workflow_run: Option<WorkflowRun>,
}

#[derive(Debug, Deserialize)]
pub struct WorkflowRun {
    pub head_sha: String,
}

#[cfg(test)]
mod test {
    use super::*;

    fn pr(body: &str) -> PullRequest {
        PullRequest {
            number: 1,
            locked: false,
            head_ref_oid: "abc123".to_string(),
            comments: Nodes {
                nodes: vec![Comment {
                    database_id: 7,
                    body: body.to_string(),
                }],
            },
        }
    }

    #[test]
    fn up_to_date() {
        let body = format!("{COMMENT_MARKER}### Memory usage change @ abc123\n\n");
        assert!(pr(&body).is_up_to_date());
    }

    #[test]
    fn outdated() {
        let body = format!("{COMMENT_MARKER}### Memory usage change @ def456\n\n");
        let pr = pr(&body);
        assert!(!pr.is_up_to_date());
        assert!(pr.report_comment().is_some());
    }

    #[test]
    fn foreign_comment() {
        assert!(pr("hello @ abc123").report_comment().is_none());
    }
}
