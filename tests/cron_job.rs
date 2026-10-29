//! Runs the CLI binary as a scheduled CI job against a mocked GitHub API.
use arduino_report_size_deltas::COMMENT_MARKER;
use mockito::{Matcher, Mock, Server, ServerGuard};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    process::{Command, Output},
};

const ASSETS: &str = "tests/cron_job_assets";
const REPORTS: &str = "tests/size-deltas-reports-new";
const ARTIFACT_NAME: &str = "sketches-reports";

fn asset(name: &str, server: &ServerGuard) -> String {
    fs::read_to_string(format!("{ASSETS}/{name}"))
        .unwrap()
        .replace("{{SERVER}}", &server.url())
        .replace("{{MARKER}}", &COMMENT_MARKER.escape_debug().to_string())
}

/// Zip the sketch reports like `actions/upload-artifact` would.
fn zip_reports() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(vec![]));
    for entry in fs::read_dir(REPORTS).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            writer
                .start_file(
                    path.file_name().unwrap().to_string_lossy(),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(&fs::read(&path).unwrap()).unwrap();
        }
    }
    writer.finish().unwrap().into_inner()
}

fn mock_listings(server: &mut ServerGuard) -> Vec<Mock> {
    vec![
        server
            .mock("POST", "/graphql")
            .with_body(asset("open-prs.json", server))
            .expect(1)
            .create(),
        server
            .mock("GET", "/repos/owner/repo/actions/artifacts")
            .match_query(Matcher::UrlEncoded("name".into(), ARTIFACT_NAME.into()))
            .match_header("authorization", "Bearer 123456")
            .with_body(asset("artifacts.json", server))
            .expect(1)
            .create(),
    ]
}

fn run_cli(server: &ServerGuard) -> Output {
    Command::new(env!("CARGO_BIN_EXE_report-size-deltas"))
        .env("GITHUB_ACTIONS", "true")
        .env("GITHUB_EVENT_NAME", "schedule")
        .env("GITHUB_REPOSITORY", "owner/repo")
        .env("GITHUB_SHA", "deadbeef")
        .env("GITHUB_TOKEN", "123456")
        .env("GITHUB_API_URL", server.url())
        .env("GITHUB_GRAPHQL_URL", format!("{}/graphql", server.url()))
        .env("SKETCHES_REPORTS_SOURCE", ARTIFACT_NAME)
        .output()
        .unwrap()
}

fn mock_report_post(server: &mut ServerGuard, method: &str, path: &str) -> Mock {
    server
        .mock(method, path)
        .match_body(Matcher::Regex("Memory usage change @".to_string()))
        .with_body("{}")
        .expect(1)
        .create()
}

#[test]
fn reports_on_pending_prs_only() {
    let mut server = Server::new();
    let zip = zip_reports();
    let mut mocks = mock_listings(&mut server);
    // PR 1 (no report) and PR 2 (outdated report) are downloaded.
    // PR 3 (up-to-date), PR 4 (locked), and PR 5 (expired artifact) are not.
    for id in [1, 2] {
        mocks.push(
            server
                .mock("GET", format!("/download/{id}").as_str())
                .with_body(&zip)
                .expect(1)
                .create(),
        );
    }
    for id in [3, 4] {
        mocks.push(
            server
                .mock("GET", format!("/download/{id}").as_str())
                .expect(0)
                .create(),
        );
    }
    mocks.push(mock_report_post(
        &mut server,
        "POST",
        "/repos/owner/repo/issues/1/comments",
    ));
    mocks.push(mock_report_post(
        &mut server,
        "PATCH",
        "/repos/owner/repo/issues/comments/77",
    ));

    let output = run_cli(&server);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn continues_after_a_failed_download() {
    let mut server = Server::new();
    let zip = zip_reports();
    let mut mocks = mock_listings(&mut server);
    mocks.push(
        server
            .mock("GET", "/download/1")
            .with_status(404)
            .expect(1)
            .create(),
    );
    mocks.push(
        server
            .mock("GET", "/download/2")
            .with_body(&zip)
            .expect(1)
            .create(),
    );
    mocks.push(mock_report_post(
        &mut server,
        "PATCH",
        "/repos/owner/repo/issues/comments/77",
    ));

    let output = run_cli(&server);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Failed to report on PR #1")
            && stderr.contains("Failed to report on 1 PR(s)"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn follows_paginated_listings() {
    let mut server = Server::new();
    let url = server.url();
    let zip = zip_reports();
    let up_to_date = format!("{COMMENT_MARKER}### Memory usage change @ ccc");
    let page = |nodes: Value, next: Option<&str>| {
        json!({"data": {"repository": {"pullRequests": {
            "nodes": nodes,
            "pageInfo": {"hasNextPage": next.is_some(), "endCursor": next},
        }}}})
        .to_string()
    };
    let pr = |number: u64, sha: &str, comments: Value| {
        json!({
            "number": number,
            "locked": false,
            "headRefOid": sha,
            "comments": {"nodes": comments},
        })
    };
    let artifact = |sha: Option<&str>, expired: bool, id: u64| {
        let mut value = json!({
            "expired": expired,
            "archive_download_url": format!("{url}/download/{id}"),
        });
        if let Some(sha) = sha {
            value["workflow_run"] = json!({"head_sha": sha});
        }
        value
    };

    let mut mocks = vec![
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex(r#""after":null"#.to_string()))
            .with_body(page(json!([pr(1, "aaa", json!([]))]), Some("cursor1")))
            .expect(1)
            .create(),
        server
            .mock("POST", "/graphql")
            .match_body(Matcher::Regex(r#""after":"cursor1""#.to_string()))
            .with_body(page(
                json!([pr(2, "ccc", json!([{"databaseId": 9, "body": up_to_date}]))]),
                None,
            ))
            .expect(1)
            .create(),
    ];

    // The first page is full, but has nothing usable: it forces a second page to be probed.
    let mut first_page = vec![artifact(Some("aaa"), true, 8), artifact(None, false, 9)];
    first_page.extend((0..98).map(|_| artifact(Some("zzz"), false, 10)));
    let artifacts_query = |page: &str| {
        Matcher::AllOf(vec![
            Matcher::UrlEncoded("name".into(), ARTIFACT_NAME.into()),
            Matcher::UrlEncoded("page".into(), page.into()),
        ])
    };
    mocks.push(
        server
            .mock("GET", "/repos/owner/repo/actions/artifacts")
            .match_query(artifacts_query("1"))
            .with_body(json!({"artifacts": first_page}).to_string())
            .expect(1)
            .create(),
    );
    mocks.push(
        server
            .mock("GET", "/repos/owner/repo/actions/artifacts")
            .match_query(artifacts_query("2"))
            .with_body(json!({"artifacts": [artifact(Some("aaa"), false, 1)]}).to_string())
            .expect(1)
            .create(),
    );
    mocks.push(
        server
            .mock("GET", "/download/1")
            .with_body(&zip)
            .expect(1)
            .create(),
    );
    mocks.push(mock_report_post(
        &mut server,
        "POST",
        "/repos/owner/repo/issues/1/comments",
    ));

    let output = run_cli(&server);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for mock in mocks {
        mock.assert();
    }
}

#[test]
fn nothing_to_do_without_artifacts() {
    let mut server = Server::new();
    let graphql = server
        .mock("POST", "/graphql")
        .with_body(
            r#"{"data":{"repository":{"pullRequests":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#,
        )
        .expect(1)
        .create();
    let artifacts = server
        .mock("GET", "/repos/owner/repo/actions/artifacts")
        .match_query(Matcher::Any)
        .expect(0)
        .create();

    assert!(run_cli(&server).status.success());
    graphql.assert();
    artifacts.assert();
}
