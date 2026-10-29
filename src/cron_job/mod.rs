//! Support for scheduled CI runs (aka "cron jobs").
//!
//! Open PRs are listed with a single GraphQL query (including their comments).
//! The repository's artifacts are then listed once (paginated) instead of
//! traversing each PR's commits and workflow runs.
//! After that, only 2 requests are needed per PR that lacks an up-to-date report
//! (1 to download the artifact, 1 to post/update the comment).
use arduino_report_size_deltas::{CronJobError, generate_comment};
use git_bot_feedback::{RestApiClient, RestApiRateLimitHeaders, client::USER_AGENT};
use reqwest::{
    Client, Method, Response, Url,
    header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde_json::json;
use std::{collections::HashMap, env, io::Cursor};

mod structs;
use structs::{Artifact, ArtifactList, GraphQlResponse, OpenPullRequestsData, PullRequest};

type Result<T> = std::result::Result<T, CronJobError>;

fn http_err(context: &'static str) -> impl FnOnce(reqwest::Error) -> CronJobError {
    move |source| CronJobError::Http { context, source }
}

const QUERY_OPEN_PRS: &str = r#"query($owner: String!, $name: String!, $after: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(states: OPEN, first: 50, after: $after) {
      nodes {
        number
        locked
        headRefOid
        comments(last: 100) {
          nodes {
            databaseId
            body
          }
        }
      }
      pageInfo {
        hasNextPage
        endCursor
      }
    }
  }
}"#;

const ARTIFACTS_PER_PAGE: usize = 100;

/// The maximum number of artifact list pages to probe.
///
/// Artifacts are listed newest first, and probing stops early only once every pending
/// commit has a matching artifact. A PR without an applicable artifact would otherwise
/// cause every page to be probed, so this bounds the requests spent on such PRs.
///
/// Only the newest `MAX_ARTIFACT_PAGES * ARTIFACTS_PER_PAGE` artifacts are considered.
/// A PR whose artifact is older than that is treated as having no artifact.
const MAX_ARTIFACT_PAGES: u32 = 10;

struct GithubApi<'a> {
    bot: &'a (dyn RestApiClient + Send + Sync),
    http: Client,
    rate_limit: RestApiRateLimitHeaders,
    api_url: Url,
    graphql_url: Url,
    repo: String,
}

impl<'a> GithubApi<'a> {
    fn new(bot: &'a (dyn RestApiClient + Send + Sync), token: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );
        let mut auth = HeaderValue::from_str(&format!("Bearer {token}"))?;
        auth.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth);

        let api_url = env::var("GITHUB_API_URL").unwrap_or("https://api.github.com".to_string());
        let graphql_url =
            env::var("GITHUB_GRAPHQL_URL").unwrap_or("https://api.github.com/graphql".to_string());
        Ok(Self {
            bot,
            http: Client::builder()
                .default_headers(headers)
                .user_agent(USER_AGENT)
                .build()
                .map_err(http_err("build the HTTP client"))?,
            rate_limit: RestApiRateLimitHeaders {
                reset: "x-ratelimit-reset".to_string(),
                remaining: "x-ratelimit-remaining".to_string(),
                retry: "retry-after".to_string(),
            },
            api_url: Url::parse(&format!("{}/", api_url.trim_end_matches('/')))?,
            graphql_url: Url::parse(&graphql_url)?,
            repo: env::var("GITHUB_REPOSITORY")
                .map_err(|_| CronJobError::MissingEnv("GITHUB_REPOSITORY"))?,
        })
    }

    async fn send(
        &self,
        context: &'static str,
        method: Method,
        url: Url,
        body: Option<String>,
    ) -> Result<Response> {
        let request = self
            .bot
            .make_api_request(&self.http, url, method, body, None)?;
        let response = self
            .bot
            .send_api_request(&self.http, request, &self.rate_limit)
            .await?;
        response.error_for_status().map_err(http_err(context))
    }

    /// Get all open PRs (with their comments) in as few requests as possible.
    async fn open_pull_requests(&self) -> Result<Vec<PullRequest>> {
        let (owner, name) = self
            .repo
            .split_once('/')
            .ok_or_else(|| CronJobError::MalformedRepo(self.repo.clone()))?;
        let mut found = vec![];
        let mut after: Option<String> = None;
        loop {
            let payload = json!({
                "query": QUERY_OPEN_PRS,
                "variables": {"owner": owner, "name": name, "after": after},
            });
            let context = "list open pull requests";
            let text = self
                .send(
                    context,
                    Method::POST,
                    self.graphql_url.clone(),
                    Some(payload.to_string()),
                )
                .await?
                .text()
                .await
                .map_err(http_err(context))?;
            let response: GraphQlResponse<OpenPullRequestsData> = serde_json::from_str(&text)
                .map_err(|source| CronJobError::Json {
                    context: "list of open pull requests",
                    source,
                })?;
            let Some(data) = response.data else {
                let messages: Vec<_> = response.errors.iter().map(|e| e.message.as_str()).collect();
                return Err(CronJobError::GraphQl(messages.join("; ")));
            };
            let prs = data.repository.pull_requests;
            found.extend(prs.nodes);
            if !prs.page_info.has_next_page {
                return Ok(found);
            }
            after = prs.page_info.end_cursor;
        }
    }

    /// Find the newest unexpired artifact for each of the given commit SHAs.
    async fn find_artifacts(
        &self,
        artifact_name: &str,
        shas: &[&str],
    ) -> Result<HashMap<String, Artifact>> {
        let mut found = HashMap::new();
        for page in 1..=MAX_ARTIFACT_PAGES {
            log::debug!("Probing page {page} of artifacts list");
            let mut url = self
                .api_url
                .join(&format!("repos/{}/actions/artifacts", self.repo))?;
            url.query_pairs_mut()
                .append_pair("name", artifact_name)
                .append_pair("per_page", &ARTIFACTS_PER_PAGE.to_string())
                .append_pair("page", &page.to_string());
            let context = "list artifacts";
            let text = self
                .send(context, Method::GET, url, None)
                .await?
                .text()
                .await
                .map_err(http_err(context))?;
            let list: ArtifactList =
                serde_json::from_str(&text).map_err(|source| CronJobError::Json {
                    context: "list of artifacts",
                    source,
                })?;
            let count = list.artifacts.len();
            for artifact in list.artifacts {
                if artifact.expired {
                    continue;
                }
                let Some(sha) = artifact.workflow_run.as_ref().map(|r| r.head_sha.clone()) else {
                    continue;
                };
                if shas.contains(&sha.as_str()) {
                    found.entry(sha).or_insert(artifact);
                }
            }
            if found.len() == shas.len() || count < ARTIFACTS_PER_PAGE {
                break;
            }
        }
        Ok(found)
    }

    /// Download an artifact's ZIP archive and generate a comment from the JSON files in it.
    async fn comment_from_artifact(&self, artifact: &Artifact) -> Result<String> {
        let context = "download artifact";
        let bytes = self
            .send(
                context,
                Method::GET,
                Url::parse(&artifact.archive_download_url)?,
                None,
            )
            .await?
            .bytes()
            .await
            .map_err(http_err(context))?;
        let dir = tempfile::tempdir().map_err(CronJobError::TempDir)?;
        zip::ZipArchive::new(Cursor::new(bytes))?.extract(dir.path())?;
        Ok(generate_comment(dir.path())?)
    }

    async fn post_comment(&self, pr: &PullRequest, comment: &str) -> Result<()> {
        let (method, path) = match pr.report_comment() {
            Some(existing) => (
                Method::PATCH,
                format!(
                    "repos/{}/issues/comments/{}",
                    self.repo, existing.database_id
                ),
            ),
            None => (
                Method::POST,
                format!("repos/{}/issues/{}/comments", self.repo, pr.number),
            ),
        };
        self.send(
            "post report comment",
            method,
            self.api_url.join(&path)?,
            Some(json!({"body": comment}).to_string()),
        )
        .await?;
        Ok(())
    }
}

/// Post (or update) report comments on all open PRs that lack one for their latest commit.
pub async fn run(
    bot: &(dyn RestApiClient + Send + Sync),
    token: &str,
    artifact_name: &str,
) -> Result<()> {
    let api = GithubApi::new(bot, token)?;
    let prs = api.open_pull_requests().await?;
    let pending: Vec<_> = prs
        .iter()
        .filter(|pr| !pr.locked && !pr.is_up_to_date())
        .collect();
    log::info!(
        "Found {} open PR(s); {} need a report",
        prs.len(),
        pending.len()
    );
    if pending.is_empty() {
        return Ok(());
    }

    let shas: Vec<&str> = pending.iter().map(|pr| pr.head_ref_oid.as_str()).collect();
    let artifacts = api.find_artifacts(artifact_name, &shas).await?;

    let mut failures = 0;
    for pr in pending {
        let Some(artifact) = artifacts.get(&pr.head_ref_oid) else {
            log::info!("No artifacts found for PR #{}", pr.number);
            continue;
        };
        let result = async {
            let comment = api.comment_from_artifact(artifact).await?;
            api.post_comment(pr, &comment).await
        }
        .await;
        match result {
            Ok(()) => log::info!("Posted report on PR #{}", pr.number),
            Err(e) => {
                log::error!("Failed to report on PR #{}: {e}", pr.number);
                failures += 1;
            }
        }
    }
    if failures > 0 {
        return Err(CronJobError::ReportFailures(failures));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::run;
    use arduino_report_size_deltas::CronJobError;
    use git_bot_feedback::client::LocalClient;
    use mockito::Server;
    use serde_json::json;
    use std::env;

    fn set_env(server: &Server) {
        unsafe {
            env::set_var("GITHUB_API_URL", server.url());
            env::set_var("GITHUB_GRAPHQL_URL", format!("{}/graphql", server.url()));
            env::set_var("GITHUB_REPOSITORY", "owner/repo");
        }
    }

    #[tokio::test]
    async fn graphql_errors_are_reported() {
        let mut server = Server::new_async().await;
        set_env(&server);
        server
            .mock("POST", "/graphql")
            .with_body(json!({"errors": [{"message": "bad credentials"}]}).to_string())
            .create();
        let err = run(&LocalClient, "token", "sketches-reports")
            .await
            .unwrap_err();
        assert!(
            matches!(err, CronJobError::GraphQl(ref m) if m.contains("bad credentials")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn malformed_repo_name() {
        let server = Server::new_async().await;
        set_env(&server);
        unsafe { env::set_var("GITHUB_REPOSITORY", "no-slash") };
        let err = run(&LocalClient, "token", "sketches-reports")
            .await
            .unwrap_err();
        assert!(matches!(err, CronJobError::MalformedRepo(_)), "{err:?}");
    }

    #[tokio::test]
    async fn missing_repo_env() {
        let server = Server::new_async().await;
        set_env(&server);
        unsafe { env::remove_var("GITHUB_REPOSITORY") };
        let err = run(&LocalClient, "token", "sketches-reports")
            .await
            .unwrap_err();
        assert!(matches!(err, CronJobError::MissingEnv(_)), "{err:?}");
    }

    #[tokio::test]
    async fn invalid_graphql_payload() {
        let mut server = Server::new_async().await;
        set_env(&server);
        server.mock("POST", "/graphql").with_body("{}").create();
        let err = run(&LocalClient, "token", "sketches-reports")
            .await
            .unwrap_err();
        assert!(matches!(err, CronJobError::GraphQl(_)), "{err:?}");
    }
}
