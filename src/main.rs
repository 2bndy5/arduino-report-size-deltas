//! This is the main CLI application code.
//! Use `cargo run [-- <Args>...]` to run this locally from source.
use anyhow::{Context, Result, anyhow};
use arduino_report_size_deltas::{COMMENT_MARKER, generate_comment};
use clap::Parser;
use colored::Colorize;
use git_bot_feedback::{
    CommentPolicy, RestApiClient, ThreadCommentOptions, client::GithubApiClient,
};
use log::{Level, LevelFilter, Metadata, Record};
use std::{
    env,
    io::{Write, stdout},
    path::PathBuf,
};

/// This is a CI tool to compliment the artifacts created by arduino/compile-sketches action.
#[derive(Parser, Debug)]
#[command(name = "report-size-deltas", version, about, long_about)]
pub struct Args {
    /// The path to the folder containing sketches' reports (JSON files)
    #[arg(
        short,
        long,
        default_value = "sketches-reports",
        env = "SKETCHES_REPORTS_SOURCE"
    )]
    sketches_reports_source: PathBuf,

    /// The GitHub access token used to post comments on the PR thread
    #[arg(short, long, env = "GITHUB_TOKEN")]
    token: Option<String>,
}

struct Logger;

impl Logger {
    fn level_color(level: &Level) -> String {
        let name = format!("{:>5}", level.as_str().to_uppercase());
        match level {
            Level::Error => name.red().bold().to_string(),
            Level::Warn => name.yellow().bold().to_string(),
            Level::Info => name.green().bold().to_string(),
            Level::Debug => name.blue().bold().to_string(),
            Level::Trace => name.magenta().bold().to_string(),
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        let mut stdout = stdout().lock();
        if record.target() == "CI_LOG_GROUPING" {
            // this log is meant to manipulate a CI workflow's log grouping
            writeln!(stdout, "{}", record.args()).expect("Failed to write log command to stdout");
            stdout
                .flush()
                .expect("Failed to flush log command to stdout");
        } else if self.enabled(record.metadata()) {
            let module = record.module_path();
            if module.is_none_or(|v| {
                v.starts_with("arduino_report_size_deltas") || v.starts_with("report_size_deltas")
            }) {
                writeln!(
                    stdout,
                    "[{}]: {}",
                    Self::level_color(&record.level()),
                    record.args()
                )
                .expect("Failed to write log message to stdout");
            } else {
                writeln!(
                    stdout,
                    "[{}]{{{}:{}}}: {}",
                    Self::level_color(&record.level()),
                    module.unwrap(), // safe to unwrap here because the None case is caught above
                    record.line().unwrap_or_default(),
                    record.args()
                )
                .expect("Failed to write detailed log message to stdout");
            }
            stdout
                .flush()
                .expect("Failed to flush log message to stdout");
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

pub fn logger_init() {
    let _ = log::set_logger(&LOGGER);
}

async fn run(args: &[String]) -> Result<()> {
    let args = Args::parse_from(args);
    logger_init();
    let client =
        GithubApiClient::new().with_context(|| "Failed to instantiate GitHub REST API client")?;
    log::set_max_level(if client.debug_enabled {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });

    GithubApiClient::start_log_group("Generating comment from JSON files");
    let comment = generate_comment(&args.sketches_reports_source);
    GithubApiClient::end_log_group();

    match comment {
        Ok(comment) => {
            log::info!("Posting comment");
            client
                .post_thread_comment(ThreadCommentOptions {
                    comment,
                    marker: COMMENT_MARKER.to_string(),
                    policy: CommentPolicy::Update,
                    ..Default::default()
                })
                .await
                .with_context(|| "Failed to post comment")?;
            Ok(())
        }
        Err(e) => Err(anyhow!("Failed to assemble comment:, {e}")),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run(&env::args().collect::<Vec<String>>()).await
}

#[cfg(test)]
mod test {
    use arduino_report_size_deltas::CommentAssemblyError;
    use mockito::{Matcher, Server};
    use std::{env, fs, io::Write};
    use tempfile::NamedTempFile;

    use crate::run;

    const REPO: &str = "2bndy5/arduino-report-size-deltas";
    const PR: &str = "22";
    const TOKEN: &str = "123456";

    #[derive(Debug, Default)]
    struct TestParams {
        no_report_data: bool,
    }

    async fn setup_test(test_params: TestParams) {
        let mut server = Server::new_async().await;
        let mut event_payload_path = NamedTempFile::new().unwrap();
        event_payload_path
            .write_all(format!("{{\"number\": {PR}}}").as_bytes())
            .unwrap();

        unsafe {
            env::set_var("GITHUB_API_URL", server.url());
            env::set_var("GITHUB_REPOSITORY", REPO);
            env::set_var("GITHUB_SHA", "deadbeef");
            env::set_var("GITHUB_TOKEN", TOKEN);
            env::set_var("GITHUB_EVENT_NAME", "pull_request");
            env::set_var("GITHUB_EVENT_PATH", event_payload_path.path());
            env::set_var(
                "SKETCHES_REPORTS_SOURCE",
                format!(
                    "tests/size-deltas-reports-{}",
                    if test_params.no_report_data {
                        "old"
                    } else {
                        "new"
                    }
                )
                .as_str(),
            );
            if !test_params.no_report_data && env::var("ACTIONS_STEP_DEBUG").is_err() {
                env::set_var("ACTIONS_STEP_DEBUG", "true");
            }
        }

        if test_params.no_report_data {
            let result = run(&[]).await;
            assert!(result.is_err_and(|e| {
                e.to_string()
                    .contains(&CommentAssemblyError::NotFound.to_string())
            }));
            return;
        }

        let mut mocks = vec![];
        mocks.push(
            server
                .mock(
                    "GET",
                    format!("/repos/{REPO}/issues/{PR}/comments").as_str(),
                )
                .match_query(Matcher::Any)
                .match_header("Accept", "application/vnd.github.raw+json")
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .match_body(Matcher::Any)
                .with_body("[]")
                .create(),
        );

        let expected_comment = fs::read_to_string("tests/size-deltas-reports-new/out.md").unwrap();
        let expect_payload = format!(r#"{{"body":"{}"}}"#, expected_comment.escape_debug());
        mocks.push(
            server
                .mock(
                    "POST",
                    format!("/repos/{REPO}/issues/{PR}/comments").as_str(),
                )
                .match_body(Matcher::Exact(expect_payload))
                .match_header("Accept", "application/vnd.github.raw+json")
                .match_header("Authorization", format!("token {TOKEN}").as_str())
                .with_body("{}")
                .match_header(
                    "user-agent",
                    Matcher::Regex(r"^git_bot_feedback/\d+\.\d+\.\d+".to_string()),
                )
                .create(),
        );

        run(&[]).await.unwrap();
        for mock in mocks {
            mock.assert();
        }
    }

    #[tokio::test]
    async fn normal() {
        setup_test(TestParams::default()).await;

        // just for dummy coverage of log output functions
        log::error!("No errors should be encountered");
        log::set_max_level(log::LevelFilter::Trace);
        log::trace!("No backtraces enabled");
        log::logger().flush();
    }

    #[tokio::test]
    async fn with_no_data() {
        setup_test(TestParams {
            no_report_data: true,
        })
        .await;
    }
}
