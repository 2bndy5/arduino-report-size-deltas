//! A module that declares the data structures used for parsing JSON data.
//!
//! [arduino/compile-sketches]: https://github.com/arduino/compile-sketches
//! [arduino/report-size-deltas]: https://github.com/arduino/report-size-deltas
//!
//! All these structures were crafted from observations in
//!
//! - python source code (including tests) for
//!   [arduino/report-size-deltas] and [arduino/compile-sketches]
//! - actual artifacts produced via [arduino/compile-sketches].
//!
//! There doesn't seem to be a documented schema for the JSON data being parsed.'
//! All python code producing the JSON data is partially typed, so it hard to
//! discern a proper schema.
use serde::Deserialize;

/// The root structure that describes a report about compilation.
#[derive(Debug, Deserialize, Default)]
pub struct Report {
    /// The boards targeted when compiling sketches.
    pub boards: Vec<Board>,
    /// The SHA hash of the commit from which compilation was performed.
    pub commit_hash: String,
    /// The URL of the commit referenced by the [`Report::commit_hash`].
    pub commit_url: String,
}

impl Report {
    /// Ensure all needed data is present.
    ///
    /// [`parse_artifacts()`][fn@crate::parse_artifacts] supports parsing of
    /// old/outdated JSON formats previously produced by the `arduino/compile-sketches`
    /// action. Use this function to ensure enough data is present to form a [`Report`].
    pub fn is_valid(&self) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        for board in &self.boards {
            if board.sizes.is_none() || board.sizes.as_ref().is_some_and(|v| v.is_empty()) {
                return false;
            }
            // unwrap() yields Some value because of the check above
            for size in board.sizes.as_ref().unwrap() {
                if !size.has_maximum() {
                    return false;
                }
            }
        }
        true
    }
}

/// A intermediate structure used to translate olf JSON formats into the newer format.
#[derive(Debug, Deserialize)]
pub(super) struct ReportOld {
    pub board: String,
    pub commit_hash: String,
    pub commit_url: String,
    pub sketches: Vec<Sketch>,
    pub sizes: Option<Vec<BoardSize>>,
}

impl From<ReportOld> for Report {
    /// Convert a [`ReportOld`] instance into a [`Report`] instance.
    fn from(value: ReportOld) -> Self {
        let board = Board {
            board: value.board,
            sketches: value.sketches,
            sizes: value.sizes,
        };
        Self {
            boards: vec![board],
            commit_hash: value.commit_hash,
            commit_url: value.commit_url,
        }
    }
}

/// A data structure to describe the target [`Board::board`] and compilation context.
///
/// Includes it's  ([`Board::sizes`]), and which [`Board::sketches`] were compiled.
#[derive(Debug, Deserialize, Default)]
pub struct Board {
    /// The board's "Fully Qualified Board Name" (FQBN).
    ///
    /// A board-specific ID used by Arduino CLI tool.
    pub board: String,

    /// The list of compiled [`Sketch`]es.
    pub sketches: Vec<Sketch>,

    /// The board's maximum capacity of memory and flash.
    pub sizes: Option<Vec<BoardSize>>,
}

/// A data structure used to describe a compiled sketch.
#[derive(Debug, Deserialize, Default)]
pub struct Sketch {
    /// The relative path to the sketch compiled.
    ///
    /// Often relative to the project's root directory.
    pub name: String,

    /// Was sketch successfully compiled?
    pub compilation_success: bool,

    /// The compile size of the sketch.
    ///
    /// This [`Vec`] typically includes details about
    /// [`SketchSizeKind::Flash`] and [`SketchSizeKind::Ram`].
    pub sizes: Vec<SketchSizeKind>,

    /// The number of compilation warnings (if any).
    ///
    /// This information is only included in the report artifacts when the
    /// `enable-warnings-report` option is enabled for `arduino/compile-sketches`.
    pub warnings: Option<SketchWarnings>,
}

/// The number of warnings about a particular sketch's compilation.
#[derive(Debug, Deserialize, Default)]
pub struct SketchWarnings {
    /// The current number of warnings from latest compilation.
    pub current: AbsCount,

    /// The previous number of warnings from latest compilation.
    pub previous: AbsCount,

    /// The change in the number of warnings from [`SketchWarnings::previous`] to [`SketchWarnings::current`].
    pub delta: AbsCount,
}

/// An absolute count used for the values of [`SketchWarnings`].
#[derive(Debug, Deserialize, Default)]
pub struct AbsCount {
    /// The absolute 32-bit integer value.
    ///
    /// "Absolute" as in "not relative", meaning this value can be negative.
    pub absolute: i32,
}

/// A data structure to describe a compilation's size.
///
/// Used for [`SketchSizeKind::Ram`] and [`SketchSizeKind::Flash`].
#[derive(Debug, Deserialize, Default)]
pub struct SketchSize {
    /// The maximum size of something.
    ///
    /// Only present for compatibility with older JSON formats.
    /// This is not actually used in the generated report comment.
    /// Instead, maximum values are stored in [`Board::sizes`].
    pub maximum: Option<SizeValue<u64>>,

    /// The current compilation size.
    pub current: SketchDeltaSize,

    /// The previous compilation size.
    ///
    /// Can be [`None`] if no previous compilation was performed.
    pub previous: Option<SketchDeltaSize>,

    /// The change in compilation size from [SketchSize::previous] to [`SketchSize::current`].
    ///
    /// Can be [`None`] if no previous compilation was performed.
    pub delta: Option<SketchDeltaSize>,
}

impl SketchSize {
    /// A convenience function to get [`SketchSize::delta`].
    ///
    /// Falls back to [`SketchSize::current`] when [`SketchSize::delta`] is [`None`].
    pub fn get_delta(&self) -> &SketchDeltaSize {
        self.delta.as_ref().unwrap_or(&self.current)
    }
}

/// An enumeration of possible compilation size kinds.
#[derive(Debug, Deserialize)]
#[serde(tag = "name")]
pub enum SketchSizeKind {
    /// The compilation size of "Ram for global variables".
    #[serde(rename(deserialize = "RAM for global variables"))]
    Ram {
        #[serde(flatten)]
        size: SketchSize,
    },

    /// The compilation size of flash memory.
    #[serde(rename(deserialize = "flash"))]
    Flash {
        #[serde(flatten)]
        size: SketchSize,
    },
}

impl Default for SketchSizeKind {
    fn default() -> Self {
        Self::Flash {
            size: Default::default(),
        }
    }
}

/// An enumeration of the possible values used to describe a compilation's size.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SizeValue<T> {
    /// Represents a "Not Applicable" (N/A) value.
    NotApplicable(String),

    /// Represents a known value.
    Known(T),
}

impl<T> Default for SizeValue<T> {
    fn default() -> Self {
        SizeValue::NotApplicable(String::from("N/A"))
    }
}

/// A data structure to describe fields in [`SketchSize`].
#[derive(Debug, Deserialize, Default)]
pub struct SketchDeltaSize {
    /// The absolute compilation size value.
    ///
    /// "Absolute" as in "not relative", meaning this 64-bit integer can be negative.
    pub absolute: SizeValue<i64>,

    /// The relative compilation size.
    ///
    /// Often relative to a previous compilation size.
    /// This can be [`None`]if no previous compilation was preformed.
    pub relative: Option<SizeValue<f32>>,
}

/// An enumeration of a [`Board::sizes`].
#[derive(Debug, Deserialize)]
#[serde(tag = "name")]
pub enum BoardSize {
    /// The maximum size of "RAM for global variables".
    #[serde(rename(deserialize = "RAM for global variables"))]
    Ram { maximum: Option<SizeValue<u64>> },
    /// The maximum size of flash memory.
    #[serde(rename(deserialize = "flash"))]
    Flash { maximum: Option<SizeValue<u64>> },
}

impl Default for BoardSize {
    fn default() -> Self {
        BoardSize::Flash {
            maximum: Default::default(),
        }
    }
}

impl BoardSize {
    /// A convenience function to ensure the board's maximum sizes are defined.
    ///
    /// Primarily used by [`Report::is_valid()`].
    pub fn has_maximum(&self) -> bool {
        match self {
            BoardSize::Ram { maximum } => maximum.is_some(),
            BoardSize::Flash { maximum } => maximum.is_some(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::report_structs::BoardSize;

    use super::{Report, SketchSize, SketchSizeKind};

    #[test]
    fn no_boards() {
        let report = Report {
            boards: vec![],
            commit_hash: Default::default(),
            commit_url: Default::default(),
        };
        assert!(!report.is_valid());
    }

    #[test]
    fn default_enum() {
        let _size_default = SketchSize::default();
        assert!(matches!(
            SketchSizeKind::default(),
            SketchSizeKind::Flash {
                size: _size_default
            }
        ));

        assert!(matches!(
            BoardSize::default(),
            BoardSize::Flash { maximum: None }
        ));
    }
}
