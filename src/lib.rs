#![doc = include_str!("../README.md")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod error;
mod reports;
pub use reports::structs as report_structs;
mod summarize;
pub use error::{CommentAssemblyError, JsonError};
pub use reports::parse_artifacts;
pub use summarize::{COMMENT_MARKER, generate_comment};
