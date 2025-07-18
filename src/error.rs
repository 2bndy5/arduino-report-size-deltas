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
