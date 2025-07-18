//! A module to define functions that generate a markdown comment.
mod helpers;
use std::path::Path;
mod structs;
use crate::{CommentAssemblyError, reports::parse_artifacts};
pub use helpers::COMMENT_MARKER;
use helpers::{generate_detailed_table, generate_general_table};

/// Generate the comment to be posted for a PR.
///
/// The given `sketches_path` shall point to the directory containing JSON files.
/// The comment is generated from the parsed JSON data.
///
/// When successful, this returns a [`String`] in markdown syntax.
pub fn generate_comment<P: AsRef<Path>>(sketches_path: P) -> Result<String, CommentAssemblyError> {
    let mut reports = parse_artifacts(&sketches_path)?;
    if reports.is_empty() {
        log::error!(
            "No delta size data found in the PR's artifacts (in path {}). \
            Ensure the `enable-size-deltas-report` input for `arduino/compile-sketches` action is enabled.",
            sketches_path.as_ref().to_string_lossy()
        );
        return Err(CommentAssemblyError::NotFound);
    }
    reports.sort_by_key(|k| k.boards[0].board.clone());

    let mut comment = String::from(COMMENT_MARKER);
    comment.push_str(format!("### Memory usage change @ {}\n\n", reports[0].commit_hash).as_str());

    generate_general_table(&reports, &mut comment);
    generate_detailed_table(&reports, &mut comment);

    Ok(comment)
}

#[cfg(test)]
mod test {
    use super::{CommentAssemblyError, generate_comment};
    use std::fs;

    #[test]
    fn use_new_test_assets() {
        let comment = generate_comment("tests/size-deltas-reports-new").unwrap();
        fs::write("tests/size-deltas-reports-new/out.md", comment).unwrap();
    }

    #[test]
    fn use_old_test_assets() {
        assert!(matches!(
            generate_comment("tests/size-deltas-reports-old"),
            Err(CommentAssemblyError::NotFound)
        ));
    }

    #[test]
    fn use_actual_assets() {
        let comment = generate_comment("tests/test_assets").unwrap();
        fs::write("tests/test_assets/out.md", comment).unwrap();
    }
}
