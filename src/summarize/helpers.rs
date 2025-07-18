//! A module that holds various helper API for generating a markdown comment.
//! See [`crate::summarize::generate_comment()`] for API meant for public consumption.
use crate::{
    reports::structs::{Report, SketchSizeKind},
    summarize::structs::{SizeKind, SizeSummary},
};
use std::collections::BTreeMap;

/// A prefix to identify bot comments from markdown text.
pub const COMMENT_MARKER: &str = "<!-- 2bndy5/arduino-report-size-deltas -->\n";

/// The maximum comment length (in bytes). This limitation is imposed by GitHub REST API.
const MAX_COMMENT_LEN: usize = 65536;

/// The text used as a header row in a 5-column markdown table.
const GENERAL_HEADER: [&str; 5] = ["Board", "Flash", "%", "RAM for global variables", "%"];

/// A reusable divider for constructing 5-column markdown tables.
const TABLE_DIVIDER: &str = "|---|---|---|---|---|\n";

/// The start of a collapsed detailed report.
const START_DETAILS: &str = "\n<details><summary>Click for full report per board</summary>\n";

/// The end of a collapsed detailed report.
const END_DETAILS: &str = "\n</details>\n";

/// A function used to ensure the given `len_limit` is respected when
/// appending the specified `new_data` text to the mutably referenced `existing_comment`.
///
/// The `usize` returned is the new `len_limit` after altering the `existing_comment`.
/// If the `existing_comment` is not altered, then the given `len_limit` is returned.
fn append_to_comment(existing_comment: &mut String, new_data: &str, len_limit: usize) -> usize {
    let new_len = new_data.len();
    if existing_comment.len() + new_len < len_limit {
        existing_comment.push_str(new_data);
        return len_limit - new_len;
    }
    len_limit
}

/// Create board summary table.
///
/// This is the short overview table that summarizes the changes in memory size.
pub(super) fn generate_general_table(reports: &Vec<Report>, comment: &mut String) {
    let mut board_summary = BTreeMap::new();
    for report in reports {
        for board in &report.boards {
            let mut size_summary = SizeSummary::default();
            for sketch in &board.sketches {
                for size in &sketch.sizes {
                    size_summary.add(size);
                }
            }
            let board_name = board.board.clone();
            board_summary.insert(board_name, size_summary);
        }
    }

    let mut len_limit = MAX_COMMENT_LEN - comment.len();
    len_limit = append_to_comment(
        comment,
        format!("| {} |\n{TABLE_DIVIDER}", GENERAL_HEADER.join(" | ")).as_str(),
        len_limit,
    );
    for (board, summary) in board_summary {
        let row = [
            board,
            summary.flash.summarize_absolute(),
            summary.flash.summarize_relative(),
            summary.ram.summarize_absolute(),
            summary.ram.summarize_relative(),
        ];
        let line = row.join(" | ");
        let new_limit = append_to_comment(comment, format!("| {line} |\n").as_str(), len_limit);
        if len_limit == new_limit {
            break;
        } else {
            len_limit = new_limit;
        }
    }
}

/// Create sketch summaries per board
pub(super) fn generate_detailed_table(reports: &Vec<Report>, comment: &mut String) {
    let mut len_limit = MAX_COMMENT_LEN - comment.len();
    if len_limit > (START_DETAILS.len() + END_DETAILS.len()) {
        len_limit = append_to_comment(comment, START_DETAILS, len_limit) - END_DETAILS.len();
        for report in reports {
            for board in &report.boards {
                len_limit = append_to_comment(
                    comment,
                    format!("\n### `{}`\n\n", board.board).as_str(),
                    len_limit,
                );
                let mut header = vec!["Sketch"];
                header.extend_from_slice(&GENERAL_HEADER[1..]);
                len_limit = append_to_comment(
                    comment,
                    format!("| {} |\n{TABLE_DIVIDER}", header.join(" | ")).as_str(),
                    len_limit,
                );
                for sketch in &board.sketches {
                    let mut row = vec![String::new(); 5];
                    row[0] = sketch.name.clone();
                    for size in &sketch.sizes {
                        match size {
                            SketchSizeKind::Ram { size } => {
                                let delta = size.get_delta();
                                row[3] = SizeKind::fmt(&delta.absolute);
                                if let Some(v) = &delta.relative {
                                    row[4] = SizeKind::fmt(v);
                                }
                            }
                            SketchSizeKind::Flash { size } => {
                                let delta = size.get_delta();
                                row[1] = SizeKind::fmt(&delta.absolute);
                                if let Some(v) = &delta.relative {
                                    row[2] = SizeKind::fmt(v);
                                }
                            }
                        }
                    }
                    let new_limit = append_to_comment(
                        comment,
                        format!("| {} |\n", row.join(" | ")).as_str(),
                        len_limit,
                    );
                    if new_limit == len_limit {
                        break;
                    } else {
                        len_limit = new_limit;
                    }
                }
            }
        }
        comment.push_str(END_DETAILS);
    }
}

#[cfg(test)]
mod test {
    use crate::{
        reports::{parse_json, structs::Report},
        summarize::helpers::{END_DETAILS, GENERAL_HEADER, START_DETAILS},
    };

    use super::{MAX_COMMENT_LEN, generate_detailed_table, generate_general_table};

    fn get_report() -> Report {
        parse_json("tests/size-deltas-reports-new/arduino-avr-nano.json").unwrap()
    }

    #[test]
    fn max_len_general() {
        let mut comment = String::new();
        for _ in 0..MAX_COMMENT_LEN {
            comment.push('.');
        }
        let reports = vec![get_report()];
        generate_general_table(&reports, &mut comment);
    }

    fn detail_comment_maxed(already_full: bool) {
        let mut comment = String::new();
        let test_max = if already_full {
            MAX_COMMENT_LEN
        } else {
            let empty_space = format!("| {} |", GENERAL_HEADER.join(" | ")).len() * 2;
            MAX_COMMENT_LEN - END_DETAILS.len() - START_DETAILS.len() - empty_space
        };
        for _ in 0..test_max {
            comment.push('.');
        }
        let reports = vec![get_report()];
        generate_detailed_table(&reports, &mut comment);
    }

    #[test]
    fn max_detail_len() {
        detail_comment_maxed(true);
        detail_comment_maxed(false);
    }
}
