//! A module to declare the data structures used to aggregate data from [`crate::reports::structs`].
use crate::reports::structs::{SizeValue, SketchSizeKind};
use std::fmt::Display;

/// A data structure to represent absolute or relative changes in memory size.
#[derive(Debug, Default)]
pub struct SizeKind {
    /// The absolute value of memory size.
    ///
    /// Typically, this should not exceed a board's maximum memory capacity.
    pub absolute: SizeValue<i64>,

    /// The relative value of memory size.
    ///
    /// This is considered relative to a board's maximum memory capacity
    /// (as determined by [arduino/compile-sketches](https://github.com/arduino/compile-sketches)).
    pub relative: SizeValue<f32>,
}

impl SizeKind {
    /// Get a pretty [`String`] representation of a numeric `value`.
    ///
    /// This basically just adds a "+" to positive numbers.
    /// Negative or zero `value`s are simply converted to a [`String`].
    pub fn fmt<T: PartialOrd + Display + Default>(value: &SizeValue<T>) -> String {
        match value {
            SizeValue::Known(v) => {
                if *v > T::default() {
                    return format!("+{v}");
                }
                format!("{v}")
            }
            SizeValue::NotApplicable(v) => v.clone(),
        }
    }
}

/// A data structure to track the minimum and maximum ranges of any changes in memory size.
#[derive(Debug, Default)]
pub struct SizeDeltaRange {
    /// The minimum value
    pub minimum: SizeKind,
    pub maximum: SizeKind,
}

impl SizeDeltaRange {
    /// A short-code for the emoji to emphasize a decrease in memory size.
    const EMOJI_DECREASE: &str = ":green_heart:";
    /// A short-code for the emoji to emphasize an increase and decrease in memory size.
    const EMOJI_AMBIGUOUS: &str = ":grey_question:";
    /// A short-code for the emoji to emphasize an increase in memory size.
    const EMOJI_INCREASE: &str = ":small_red_triangle:";
    /// A placeholder to represent data that is not applicable.
    const NOT_APPLICABLE: &str = "N/A";

    /// Incorporate the given `size` into [`Self::minimum`] or
    /// [`Self::maximum`] [`SizeKind::absolute`] values.
    fn add_absolute(&mut self, size: i64) {
        match self.minimum.absolute {
            SizeValue::NotApplicable(_) => {
                self.minimum.absolute = SizeValue::Known(size);
            }
            SizeValue::Known(val) => {
                if size < val {
                    self.minimum.absolute = SizeValue::Known(size);
                }
            }
        }
        match self.maximum.absolute {
            SizeValue::NotApplicable(_) => {
                self.maximum.absolute = SizeValue::Known(size);
            }
            SizeValue::Known(val) => {
                if size > val {
                    self.maximum.absolute = SizeValue::Known(size);
                }
            }
        }
    }

    /// Incorporate the given `size` into [`Self::minimum`] or
    /// [`Self::maximum`] [`SizeKind::relative`] values.
    fn add_relative(&mut self, size: f32) {
        match self.minimum.relative {
            SizeValue::NotApplicable(_) => {
                self.minimum.relative = SizeValue::Known(size);
            }
            SizeValue::Known(val) => {
                if size < val {
                    self.minimum.relative = SizeValue::Known(size);
                }
            }
        }
        match self.maximum.relative {
            SizeValue::NotApplicable(_) => {
                self.maximum.relative = SizeValue::Known(size);
            }
            SizeValue::Known(val) => {
                if size > val {
                    self.maximum.relative = SizeValue::Known(size);
                }
            }
        }
    }

    /// Converts an instance of [`SizeDeltaRange`] into a [`String`] of
    /// [`SizeKind::relative`] values.
    pub(super) fn summarize_relative(&self) -> String {
        let min_rel = SizeKind::fmt(&self.minimum.relative);
        let max_rel = SizeKind::fmt(&self.maximum.relative);
        if [min_rel.as_str(), max_rel.as_str()].contains(&Self::NOT_APPLICABLE) {
            return Self::NOT_APPLICABLE.to_string();
        }
        format!("{min_rel} - {max_rel}")
    }

    /// Converts an instance of [`SizeDeltaRange`] into a [`String`] of
    /// [`SizeKind::absolute`] values.
    pub(super) fn summarize_absolute(&self) -> String {
        let emoji = {
            if let (SizeValue::Known(min), SizeValue::Known(max)) =
                (&self.minimum.absolute, &self.maximum.absolute)
            {
                if *min < 0 && *max <= 0 {
                    Self::EMOJI_DECREASE
                } else if *min == 0 && *max == 0 {
                    ""
                } else if *min >= 0 && *max > 0 {
                    Self::EMOJI_INCREASE
                } else {
                    Self::EMOJI_AMBIGUOUS
                }
            } else {
                ""
            }
        };
        let min_abs = SizeKind::fmt(&self.minimum.absolute);
        let max_abs = SizeKind::fmt(&self.maximum.absolute);
        if [min_abs.as_str(), max_abs.as_str()].contains(&Self::NOT_APPLICABLE) {
            return Self::NOT_APPLICABLE.to_string();
        }
        format!("{emoji} {min_abs} - {max_abs}")
    }
}

/// A struct to gather an overall summary of sketches' size deltas
#[derive(Debug, Default)]
pub struct SizeSummary {
    pub flash: SizeDeltaRange,
    pub ram: SizeDeltaRange,
}

impl SizeSummary {
    /// Incorporate the given `size` into the [`SizeSummary::flash`]/[`SizeSummary::ram`]
    /// [`SizeDeltaRange::maximum`]/[`SizeDeltaRange::minimum`].
    pub fn add(&mut self, size: &SketchSizeKind) {
        match size {
            SketchSizeKind::Ram { size } => {
                let delta = size.get_delta();
                if let SizeValue::Known(absolute) = delta.absolute {
                    self.ram.add_absolute(absolute);
                }
                if let Some(SizeValue::Known(relative)) = &delta.relative {
                    self.ram.add_relative(*relative);
                }
            }
            SketchSizeKind::Flash { size } => {
                let delta = size.get_delta();
                if let SizeValue::Known(absolute) = delta.absolute {
                    self.flash.add_absolute(absolute);
                }
                if let Some(SizeValue::Known(relative)) = &delta.relative {
                    self.flash.add_relative(*relative);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::reports::structs::SizeValue;

    use super::SizeKind;

    #[test]
    fn positive_has_plus() {
        assert_eq!(SizeKind::fmt(&SizeValue::Known(1)), "+1".to_string());
    }
}
