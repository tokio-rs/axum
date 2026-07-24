//! Parsing and normalization for HTTP byte-range values.
//!
//! Parsing preserves open-ended and suffix ranges until the file size is known. Normalization
//! then resolves them into inclusive byte offsets and discards unsatisfiable ranges.

/// Maximum number of ranges accepted from one header value.
pub(super) const MAX_RANGES: usize = 8;

/// One parsed byte-range specification before the file size is known.
///
/// The fields represent the three supported forms as follows:
///
/// - `start-end`: both values are inclusive byte offsets.
/// - `start-`: `end` is `None`.
/// - `-length`: `start` is `None` and `end` stores the suffix length.
///
/// A successfully parsed specification never has both fields set to `None`.
#[derive(Clone, Copy, Default)]
struct RangeSpec {
    start: Option<i64>,
    end: Option<i64>,
}

impl TryFrom<&str> for RangeSpec {
    type Error = ();

    fn try_from(range: &str) -> Result<Self, Self::Error> {
        let (start, end) = range.trim().split_once('-').ok_or(())?;
        let start = parse_range_value(start)?;
        let end = parse_range_value(end)?;

        match (start, end) {
            (None, None | Some(0)) => Err(()),
            (Some(start), Some(end)) if start > end => Err(()),
            _ => Ok(Self { start, end }),
        }
    }
}

/// A fixed-capacity collection of parsed byte-range specifications.
///
/// Only `ranges[..len]` contains parsed values. Parsing fails if any specification is malformed
/// within the first [`MAX_RANGES`] ranges. Any remaining ranges are ignored.
#[derive(Default)]
pub(super) struct RangeSpecs {
    ranges: [RangeSpec; MAX_RANGES],
    len: usize,
}

impl RangeSpecs {
    fn iter(&self) -> impl Iterator<Item = RangeSpec> + '_ {
        self.ranges[..self.len].iter().copied()
    }
}

impl TryFrom<&str> for RangeSpecs {
    type Error = ();

    fn try_from(ranges: &str) -> Result<Self, Self::Error> {
        let mut parsed = Self::default();

        for range in ranges.split(',').take(MAX_RANGES) {
            parsed.ranges[parsed.len] = RangeSpec::try_from(range)?;
            parsed.len += 1;
        }

        (parsed.len > 0).then_some(parsed).ok_or(())
    }
}

fn parse_range_value(value: &str) -> Result<Option<i64>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.parse::<i64>().map_err(|_| ())?;
    (value >= 0).then_some(Some(value)).ok_or(())
}

/// Resolves parsed specifications against `total_size`.
///
/// Explicit end offsets are clamped to the last byte. Unsatisfiable ranges are omitted, order is
/// preserved, and the returned length identifies the populated prefix of the returned array.
pub(super) fn normalize_range_specs(
    range_specs: &RangeSpecs,
    total_size: u64,
) -> ([(u64, u64); MAX_RANGES], usize) {
    let mut ranges = [(0, 0); MAX_RANGES];
    let mut len = 0;
    if total_size == 0 {
        return (ranges, len);
    }

    let last = total_size - 1;
    for range in range_specs.iter() {
        let range = match (range.start, range.end) {
            (Some(start), Some(end)) => {
                let start = start as u64;
                if start >= total_size {
                    None
                } else {
                    Some((start, (end as u64).min(last)))
                }
            }
            (Some(start), None) => {
                let start = start as u64;
                (start < total_size).then_some((start, last))
            }
            (None, Some(suffix_len)) => {
                let suffix_len = suffix_len as u64;
                Some(if suffix_len >= total_size {
                    (0, last)
                } else {
                    (total_size - suffix_len, last)
                })
            }
            (None, None) => None,
        };

        if let Some(range) = range {
            ranges[len] = range;
            len += 1;
        }
    }

    (ranges, len)
}
