use thiserror::Error;

/// The possible error types related to parsing JSON data.
#[derive(Debug, Error)]
pub enum JsonError {
    /// Errors emitted from (de)serializing JSON data.
    #[error("{0}")]
    Serde(#[from] serde_json::Error),

    /// Error emitted when failing to read a JSON file.
    #[error("{0:?}")]
    FileReadFail(#[from] std::io::Error),
}

/// The types of error that could be propagated from [`generate_comment()`][fn@crate::summarize::generate_comment].
#[derive(Debug, Error)]
pub enum CommentAssemblyError {
    /// Represents any [`std::io::Error`] encountered.
    #[error("{0:?}")]
    Io(#[from] std::io::Error),

    /// Represents any [`JsonError`] encountered.
    #[error("{0:?}")]
    Json(#[from] JsonError),

    /// Represents an error in which expected data is not found.
    ///
    /// Check stderr for the root cause of this kind of error because
    /// detailed log output is emitted in this situation.
    #[error("Found no applicable data to summarize")]
    NotFound,
}

/// The types of error that could be encountered when reporting on open PRs from a scheduled CI run.
#[cfg(feature = "bin")]
#[derive(Debug, Error)]
pub enum CronJobError {
    /// A required environment variable is not set.
    #[error("The environment variable {0} is not set")]
    MissingEnv(&'static str),

    /// The repository name is not in the form `owner/name`.
    #[error("Malformed repository name: {0}")]
    MalformedRepo(String),

    /// The GraphQL API responded with errors instead of data.
    #[error("GraphQL query failed: {0}")]
    GraphQl(String),

    /// A JSON payload from the API could not be deserialized.
    #[error("Failed to deserialize {context}: {source}")]
    Json {
        /// What was being deserialized.
        context: &'static str,
        /// The underlying error.
        source: serde_json::Error,
    },

    /// One or more PRs could not be reported on.
    #[error("Failed to report on {0} PR(s)")]
    ReportFailures(usize),

    /// Represents any error from the Git server client.
    #[error("{0}")]
    Client(#[from] git_bot_feedback::RestClientError),

    /// Represents any HTTP error encountered.
    /// An HTTP request failed (or its response could not be read).
    #[error("Failed to {context}: {source}")]
    Http {
        /// The action that was being attempted.
        context: &'static str,
        /// The underlying error.
        #[source]
        source: reqwest::Error,
    },

    /// Represents an invalid HTTP header value (eg. a malformed token).
    #[error("{0}")]
    Header(#[from] reqwest::header::InvalidHeaderValue),

    /// Represents an invalid URL.
    #[error("{0}")]
    Url(#[from] url::ParseError),

    /// Represents any error encountered while extracting an artifact.
    #[error("{0}")]
    Zip(#[from] zip::result::ZipError),

    /// Failed to create the temporary folder used to extract an artifact.
    #[error("Failed to create a temporary folder for the artifact: {0}")]
    TempDir(std::io::Error),

    /// Represents any [`CommentAssemblyError`] encountered.
    #[error("{0}")]
    Comment(#[from] CommentAssemblyError),
}
