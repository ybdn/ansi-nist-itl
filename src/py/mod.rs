//! Python 3.12 conversions that `nistitl` relies on (SPEC §3).
//!
//! `nistitl` reads numbers with `int()`, writes values with `str()`, builds
//! error messages and summaries with `repr()` and string formatting. Matching
//! its output character for character requires reproducing CPython's rules,
//! Unicode tables included. Everything here is internal to the crate.

#[cfg(test)]
mod cpython_vectors;
mod encode;
mod format;
mod int;
mod repr;
mod unicode_tables;

pub(crate) use encode::{EncodeError, encode_latin1};
pub(crate) use format::{BadFormat, PrintfTemplate};
pub(crate) use int::{IntError, py_int};
pub(crate) use repr::{repr_bytes, repr_str};

// Consumed by the record summaries (SPEC §8.9, §10.6) and the `struct` values
// (§9.6), which come in the next development steps.
#[allow(unused_imports)]
pub(crate) use format::{pad_str, thousands};
#[allow(unused_imports)]
pub(crate) use repr::float_repr;

/// `str(bool)`.
pub(crate) fn bool_str(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

/// `str(None)`.
pub(crate) const NONE_STR: &str = "None";

/// Returns `true` if `c` falls in one of the sorted, disjoint `ranges`.
fn in_ranges(ranges: &[(u32, u32)], c: char) -> bool {
    let c = u32::from(c);
    ranges
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// `str.isspace()`.
pub(crate) fn is_space(c: char) -> bool {
    in_ranges(unicode_tables::SPACE, c)
}

/// `str.isprintable()`.
pub(crate) fn is_printable(c: char) -> bool {
    in_ranges(unicode_tables::PRINTABLE, c)
}

/// Value of a decimal digit (`str.isdecimal()`), any script.
pub(crate) fn decimal_value(c: char) -> Option<u8> {
    let c = u32::from(c);
    let table = unicode_tables::DECIMAL;
    let i = table.partition_point(|&(_, hi)| hi < c);
    let &(lo, hi) = table.get(i)?;
    if lo <= c && c <= hi {
        u8::try_from(c - lo).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_latin1() {
        let spaces: Vec<u32> = (0..=0xFFu32)
            .filter_map(char::from_u32)
            .filter(|&c| is_space(c))
            .map(u32::from)
            .collect();
        assert_eq!(
            spaces,
            [
                0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x85, 0xA0
            ]
        );
        assert!(is_space('\u{3000}'));
        assert!(!is_space('\u{200B}'));
    }

    #[test]
    fn printable_latin1() {
        let non_printable: Vec<u32> = (0..=0xFFu32)
            .filter_map(char::from_u32)
            .filter(|&c| !is_printable(c))
            .map(u32::from)
            .collect();
        let expected: Vec<u32> = (0x00..=0x1F).chain(0x7F..=0xA0).chain([0xAD]).collect();
        assert_eq!(non_printable, expected);
        assert!(is_printable('€'));
        assert!(!is_printable('\u{10FFFF}'));
    }

    #[test]
    fn decimal_digits() {
        assert_eq!(decimal_value('0'), Some(0));
        assert_eq!(decimal_value('9'), Some(9));
        assert_eq!(decimal_value('a'), None);
        assert_eq!(decimal_value('\u{0661}'), Some(1)); // ARABIC-INDIC DIGIT ONE
        assert_eq!(decimal_value('\u{FF19}'), Some(9)); // FULLWIDTH DIGIT NINE
        assert_eq!(decimal_value('\u{1D7FF}'), Some(9)); // MATHEMATICAL MONOSPACE DIGIT NINE
        assert_eq!(decimal_value('²'), None); // No, not Nd
        assert_eq!(decimal_value('\u{2160}'), None); // ROMAN NUMERAL ONE, Nl
    }

    #[test]
    fn str_of_constants() {
        assert_eq!(bool_str(true), "True");
        assert_eq!(bool_str(false), "False");
        assert_eq!(NONE_STR, "None");
    }
}
