//! A module for API related to parsing of JSON data from CI artifacts.
//! Additionally, there's a convenient [`parse_json()`] function for parsing of older
//! JSON formats produced by the [arduino/compile-sketches] action.
//!
//! [arduino/compile-sketches]: https://github.com/arduino/compile-sketches
use crate::error::JsonError;
use std::{fs, path::Path};
pub mod structs;
use structs::{Report, ReportOld};

/// Deserialize a JSON file at the given `path` into a [`Report`].
///
/// This will automatically try to parsing old JSON formats when
/// parsing the newer format fails syntactically.
pub fn parse_json<P: AsRef<Path>>(path: P) -> Result<Report, JsonError> {
    let asset = fs::read_to_string(path)?;
    match serde_json::from_str::<Report>(&asset) {
        Ok(report) => Ok(report),
        Err(e) => {
            if e.is_data() {
                // if parsing the new format fails (for typing reasons),
                // then try the old format and convert it.
                match serde_json::from_str::<ReportOld>(&asset) {
                    Ok(report) => Ok(report.into()),
                    Err(e_old) => {
                        eprintln!("Parsing old format failed: {e_old}");
                        Err(JsonError::Serde(e))
                    }
                }
            } else {
                Err(JsonError::Serde(e))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use super::{JsonError, parse_json};
    use tempfile::NamedTempFile;

    /// Test parsing of JSON report in newer format
    #[test]
    fn parse_new() {
        for entry in std::fs::read_dir("tests/size-deltas-reports-new").unwrap() {
            let path = entry.unwrap().path();
            if path.extension().unwrap().to_string_lossy() == "json" {
                println!("Parsing {path:?}");
                let report = parse_json(&path).unwrap();
                assert!(!report.boards.is_empty());
                assert!(report.is_valid());
            } else {
                println!("Skipped parsing non-JSON file: {}", path.to_string_lossy());
            }
        }
    }

    /// Test parsing of JSON report in newer format
    #[test]
    fn parse_old() {
        for entry in std::fs::read_dir("tests/size-deltas-reports-old").unwrap() {
            let path = entry.unwrap().path();
            println!("Parsing {path:?}");
            let report = parse_json(path).unwrap();
            assert!(!report.boards.is_empty());
            assert!(!report.is_valid());
        }
    }

    #[test]
    fn absent_file() {
        let result = parse_json("not-a-file.json");
        assert!(result.is_err_and(|e| matches!(e, JsonError::FileReadFail(_))));
    }

    #[test]
    fn bad_json() {
        let bad_asset = NamedTempFile::new().unwrap();
        let result = parse_json(&bad_asset);
        assert!(result.is_err_and(|e| matches!(e, JsonError::Serde(_))));
    }

    #[test]
    fn bad_report() {
        let mut bad_asset = NamedTempFile::new().unwrap();
        bad_asset.write_all("{}".as_bytes()).unwrap();
        let result = parse_json(&bad_asset);
        assert!(result.is_err_and(|e| matches!(e, JsonError::Serde(_))));
    }
}
