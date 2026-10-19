//! Parsing and normalization for HTTP byte-range values.
//!
//! Parsing preserves open-ended and suffix ranges until the file size is known. Normalization
//! then resolves them into inclusive byte offsets and discards unsatisfiable ranges.

use std::str::FromStr;

/// Maximum number of ranges accepted from one header value.
pub(super) const MAX_RANGES: usize = 8;

/// One parsed byte-range specification before the file size is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RangeSpec {
    /// `start-end`, with inclusive byte offsets.
    Bounded { start: u64, end: u64 },
    /// `start-`, from the given offset through the end of the file.
    OpenEnded { start: u64 },
    /// `-length`, the final `length` bytes of the file.
    Suffix { length: u64 },
}

/// Why a byte-range specification could not be parsed.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ParseRangeError {
    MissingHyphen,
    EmptyRange,
    InvalidStart,
    InvalidEnd,
    InvalidSuffixLength,
    EndBeforeStart,
}

impl FromStr for RangeSpec {
    type Err = ParseRangeError;

    fn from_str(range: &str) -> Result<Self, Self::Err> {
        let (start, end) = range
            .trim()
            .split_once('-')
            .ok_or(ParseRangeError::MissingHyphen)?;
        let (start, end) = (start.trim(), end.trim());

        let parser = |base: &str, err: Self::Err| base.parse::<u64>().map_err(|_| err);
        match (start, end) {
            ("", "") => Err(ParseRangeError::EmptyRange),
            ("", end) => Ok(Self::Suffix {
                length: parser(end, ParseRangeError::InvalidSuffixLength)?,
            }),
            (start, "") => Ok(Self::OpenEnded {
                start: parser(start, ParseRangeError::InvalidStart)?,
            }),
            (start, end) => {
                let start = parser(start, ParseRangeError::InvalidStart)?;
                let end = parser(end, ParseRangeError::InvalidEnd)?;

                if end < start {
                    return Err(ParseRangeError::EndBeforeStart);
                }
                Ok(Self::Bounded { start, end })
            }
        }
    }
}

/// A fixed-capacity collection of parsed byte-range specifications.
///
/// The array avoids a per-request allocation, while `len` tracks its populated prefix. Parsing
/// skips empty list elements and fails on malformed specifications among the first
/// [`MAX_RANGES`] non-empty entries. It examines at most twice that many list elements,
/// ignoring the rest to bound parsing work.
#[derive(Default)]
pub(super) struct RangeSpecs {
    ranges: [Option<RangeSpec>; MAX_RANGES],
    len: usize,
}

impl RangeSpecs {
    fn iter(&self) -> impl Iterator<Item = RangeSpec> + '_ {
        self.ranges[..self.len].iter().copied().flatten()
    }
}

impl FromStr for RangeSpecs {
    type Err = ParseRangeError;

    fn from_str(ranges: &str) -> Result<Self, Self::Err> {
        let mut parsed = Self::default();

        for range in ranges
            .split(',')
            .take(MAX_RANGES * 2)
            .filter(|range| !range.trim().is_empty())
            .take(MAX_RANGES)
        {
            parsed.ranges[parsed.len] = Some(range.parse()?);
            parsed.len += 1;
        }

        Ok(parsed)
    }
}

/// A fixed-capacity collection of inclusive byte offsets resolved against a file's size.
#[derive(Default)]
pub(super) struct NormalizedRanges {
    ranges: [(u64, u64); MAX_RANGES],
    len: usize,
}

impl NormalizedRanges {
    /// Returns only the populated ranges, excluding unused capacity.
    pub(super) fn as_slice(&self) -> &[(u64, u64)] {
        &self.ranges[..self.len]
    }
}

/// Resolves parsed specifications against `total_size`.
///
/// Explicit end offsets are clamped to the last byte. Unsatisfiable ranges are omitted and
/// order is preserved.
pub(super) fn normalize_range_specs(range_specs: &RangeSpecs, total_size: u64) -> NormalizedRanges {
    let mut ranges = NormalizedRanges::default();
    if total_size == 0 {
        return ranges;
    }

    let last = total_size - 1;
    for range in range_specs.iter() {
        let range = match range {
            RangeSpec::Bounded { start, end } if start < total_size => Some((start, end.min(last))),
            RangeSpec::OpenEnded { start } if start < total_size => Some((start, last)),
            RangeSpec::Suffix { length } if length > 0 => {
                Some((total_size.saturating_sub(length), last))
            }
            _ => None,
        };

        if let Some(range) = range {
            ranges.ranges[ranges.len] = range;
            ranges.len += 1;
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::{ParseRangeError, RangeSpec};

    #[test]
    fn parse_ranges() {
        for (input, expected) in [
            ("0-3", RangeSpec::Bounded { start: 0, end: 3 }),
            ("-1", RangeSpec::Suffix { length: 1 }),
            ("1-", RangeSpec::OpenEnded { start: 1 }),
        ] {
            assert_eq!(input.parse(), Ok(expected), "{input}");
        }
    }

    #[test]
    fn parse_errors() {
        for (input, expected) in [
            ("0", ParseRangeError::MissingHyphen),
            ("-", ParseRangeError::EmptyRange),
            ("x-", ParseRangeError::InvalidStart),
            ("0-x", ParseRangeError::InvalidEnd),
            ("-x", ParseRangeError::InvalidSuffixLength),
            ("5-3", ParseRangeError::EndBeforeStart),
            ("18446744073709551616-", ParseRangeError::InvalidStart),
            ("0-18446744073709551616", ParseRangeError::InvalidEnd),
            (
                "-18446744073709551616",
                ParseRangeError::InvalidSuffixLength,
            ),
            ("18446744073709551616x-", ParseRangeError::InvalidStart),
            ("0-18446744073709551616x", ParseRangeError::InvalidEnd),
            (
                "-18446744073709551616x",
                ParseRangeError::InvalidSuffixLength,
            ),
        ] {
            assert_eq!(input.parse::<RangeSpec>().err(), Some(expected));
        }
    }
}
