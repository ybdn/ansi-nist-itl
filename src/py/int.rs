//! `int(str)` in base 10, as CPython 3.12 implements it (SPEC §3.1).
//!
//! CPython first maps the string to ASCII (`_PyUnicode_TransformDecimalAndSpaceToASCII`):
//! non-ASCII white space becomes a space, non-ASCII decimal digits become
//! their ASCII digit, and the first other non-ASCII character becomes `?`,
//! which ends the string. It then parses the result as a C string
//! (`PyLong_FromString`), where only ASCII white space is skipped and a NUL
//! byte acts as a terminator. Both steps are reproduced here, so that, for
//! example, `"\x1c1"` is rejected while `"\xa01"` is accepted.

use std::fmt;

use super::repr::repr_str_limited;
use crate::value::PyInt;

/// Default `sys.get_int_max_str_digits()` of CPython 3.12.
pub(crate) const MAX_STR_DIGITS: usize = 4300;

/// Maximum length, in characters, of the `repr` quoted in an `int()` error (`%.200R`).
const REPR_LIMIT: usize = 200;

/// Failure of [`py_int`], with the exact message of the Python `ValueError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IntError {
    /// Syntax error; holds the `repr` of the input, truncated to 200 characters.
    InvalidLiteral(String),
    /// Valid syntax, but more than [`MAX_STR_DIGITS`] digits; holds the digit count.
    DigitsLimit(usize),
}

impl fmt::Display for IntError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntError::InvalidLiteral(repr) => {
                write!(f, "invalid literal for int() with base 10: {repr}")
            }
            IntError::DigitsLimit(n) => write!(
                f,
                "Exceeds the limit ({MAX_STR_DIGITS} digits) for integer string conversion: \
                 value has {n} digits; use sys.set_int_max_str_digits() to increase the limit"
            ),
        }
    }
}

/// `int(s)`: parses a base-10 integer with the rules of CPython 3.12.
pub(crate) fn py_int(s: &str) -> Result<PyInt, IntError> {
    let ascii = to_ascii(s);
    match scan(&ascii) {
        Scan::Int(n, end) if end == ascii.len() => Ok(n),
        Scan::TooManyDigits(n) => Err(IntError::DigitsLimit(n)),
        Scan::Int(..) | Scan::Syntax => {
            Err(IntError::InvalidLiteral(repr_str_limited(s, REPR_LIMIT)))
        }
    }
}

/// `_PyUnicode_TransformDecimalAndSpaceToASCII`.
fn to_ascii(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c as u8);
        } else if super::is_space(c) {
            out.push(b' ');
        } else if let Some(d) = super::decimal_value(c) {
            out.push(b'0' + d);
        } else {
            out.push(b'?');
            break;
        }
    }
    out
}

/// Outcome of [`scan`].
enum Scan {
    /// Parsed value and position where parsing stopped (a NUL byte or the end).
    Int(PyInt, usize),
    Syntax,
    TooManyDigits(usize),
}

/// `Py_ISSPACE`: ASCII white space only.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// `PyLong_FromString(buf, &end, 10)`, reading `buf` as a NUL-terminated C string.
fn scan(buf: &[u8]) -> Scan {
    let at = |i: usize| buf.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while at(i) != 0 && is_c_space(at(i)) {
        i += 1;
    }
    let negative = match at(i) {
        b'-' => {
            i += 1;
            true
        }
        b'+' => {
            i += 1;
            false
        }
        _ => false,
    };
    let start = i;
    if at(i) == b'_' {
        return Scan::Syntax;
    }
    let mut prev = 0u8;
    let mut digits = 0usize;
    while at(i).is_ascii_digit() || at(i) == b'_' {
        if at(i) == b'_' {
            if prev == b'_' {
                return Scan::Syntax;
            }
        } else {
            digits += 1;
        }
        prev = at(i);
        i += 1;
    }
    if prev == b'_' || i == start {
        return Scan::Syntax;
    }
    let end = i;
    while at(i) != 0 && is_c_space(at(i)) {
        i += 1;
    }
    if at(i) != 0 {
        return Scan::Syntax;
    }
    if digits > MAX_STR_DIGITS {
        return Scan::TooManyDigits(digits);
    }
    let body: Vec<u8> = buf
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .copied()
        .filter(|&b| b != b'_')
        .collect();
    Scan::Int(PyInt::from_ascii_digits(negative, &body), i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> String {
        py_int(s).unwrap().to_string()
    }

    fn err(s: &str) -> String {
        py_int(s).unwrap_err().to_string()
    }

    #[test]
    fn spec_examples() {
        assert_eq!(ok(" 1 "), "1");
        assert_eq!(ok("+1"), "1");
        assert_eq!(ok("0_1"), "1");
        assert_eq!(ok("1\u{a0}"), "1");
        assert_eq!(ok("-1"), "-1");
        assert_eq!(ok("007"), "7");
        assert_eq!(err(""), "invalid literal for int() with base 10: ''");
        assert_eq!(err("A"), "invalid literal for int() with base 10: 'A'");
        assert_eq!(ok(&"0".repeat(4300)), "0");
        assert_eq!(py_int(&"0".repeat(4301)), Err(IntError::DigitsLimit(4301)));
        let underscores = format!("{}1", "1_".repeat(2150));
        assert_eq!(underscores.len(), 4301);
        assert_eq!(ok(&underscores).len(), 2151);
    }

    #[test]
    fn edge_cases_c01_c11() {
        assert_eq!(ok("-1"), "-1");
        assert_eq!(
            ok("123456789012345678901234567890"),
            "123456789012345678901234567890"
        );
    }

    #[test]
    fn white_space() {
        // ASCII: only Py_ISSPACE is skipped; the separators FS..US are not.
        assert_eq!(ok("\t\n\x0b\x0c\r 1 \r\n"), "1");
        for s in ["\x1c1", "1\x1d", "\x1e1", "1\x1f"] {
            assert_eq!(
                py_int(s),
                Err(IntError::InvalidLiteral(repr_str_limited(s, 200)))
            );
        }
        // Non-ASCII white space becomes a space.
        assert_eq!(ok("\u{85}1"), "1");
        assert_eq!(ok("\u{2003}-1\u{3000}"), "-1");
        assert!(py_int("+\u{a0}1").is_err());
        assert!(py_int("- 1").is_err());
        assert!(py_int("1\u{2028}1").is_err());
    }

    #[test]
    fn underscores() {
        for s in ["_1", "1_", "1__0", "-_1", "1_\x00"] {
            assert!(py_int(s).is_err(), "{s:?}");
        }
        assert_eq!(ok("1_000_000"), "1000000");
    }

    #[test]
    fn unicode_digits() {
        assert_eq!(ok("\u{661}\u{662}"), "12");
        assert_eq!(ok("\u{ff11}"), "1");
        assert!(py_int("²").is_err());
    }

    #[test]
    fn message_repr() {
        assert_eq!(
            err("1\x00"),
            "invalid literal for int() with base 10: '1\\x00'"
        );
        assert_eq!(
            err("a'b"),
            "invalid literal for int() with base 10: \"a'b\""
        );
        // %.200R: the repr is cut at 200 characters, closing quote included.
        let msg = err(&"A".repeat(300));
        assert_eq!(
            msg,
            format!(
                "invalid literal for int() with base 10: '{}",
                "A".repeat(199)
            )
        );
        let msg = err(&"\x00".repeat(100));
        let repr = msg
            .strip_prefix("invalid literal for int() with base 10: ")
            .unwrap();
        assert_eq!(repr.chars().count(), 200);
        assert!(repr.ends_with("\\x00\\x0"));
        let msg = err(&format!("{}A", "é".repeat(199)));
        assert!(msg.ends_with(&format!("'{}", "é".repeat(199))));
    }

    #[test]
    fn digits_limit() {
        let ones = "1".repeat(4301);
        // Syntax is checked first.
        assert!(matches!(
            py_int(&format!("{ones}A")),
            Err(IntError::InvalidLiteral(_))
        ));
        assert!(matches!(
            py_int(&format!("A{ones}")),
            Err(IntError::InvalidLiteral(_))
        ));
        assert!(matches!(
            py_int(&format!("{ones}é")),
            Err(IntError::InvalidLiteral(_))
        ));
        assert!(matches!(
            py_int(&format!("-{ones}_\x00")),
            Err(IntError::InvalidLiteral(_))
        ));
        // Sign, spaces and underscores are not counted; non-ASCII digits are.
        assert_eq!(
            py_int(&format!("-{ones}")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            py_int(&format!(" {ones}\u{a0}")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            py_int(&"\u{661}".repeat(4301)),
            Err(IntError::DigitsLimit(4301))
        );
        // A NUL byte ends the C string: what follows escapes the syntax check.
        assert_eq!(
            py_int(&format!("{ones}\x00")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            py_int(&format!("{ones}\x00A")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            py_int(&format!("{ones}\x00é")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            py_int(&format!("{ones} \x00")),
            Err(IntError::DigitsLimit(4301))
        );
        assert_eq!(
            err(&"0".repeat(4301)),
            "Exceeds the limit (4300 digits) for integer string conversion: value has 4301 \
             digits; use sys.set_int_max_str_digits() to increase the limit"
        );
    }
}
