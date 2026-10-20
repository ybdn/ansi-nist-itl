//! Values stored in fields, subfields and items (SPEC §2).

use std::cmp::Ordering;
use std::fmt;

/// Arbitrary-precision integer, the equivalent of a Python `int`.
///
/// `nistitl` reads tag numbers, LEN and IDC with Python's `int()`, which has
/// no upper bound: a tag of 20 digits is valid. `PyInt` keeps the decimal
/// digits in canonical form (no leading zeros, `0` is never negative) and
/// only supports what the crate needs: comparison, display and conversion
/// from and to machine integers.
///
/// ```
/// use ansi_nist_itl::PyInt;
///
/// let n = PyInt::from(-42i64);
/// assert_eq!(n.to_string(), "-42");
/// assert_eq!(n.to_i64(), Some(-42));
/// assert!(PyInt::from(7u64) > n);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyInt {
    negative: bool,
    /// ASCII decimal digits, without leading zeros (`"0"` for zero).
    digits: String,
}

impl PyInt {
    /// Builds a `PyInt` from ASCII decimal digits, leading zeros allowed.
    ///
    /// The caller guarantees that `digits` only contains `0`–`9`.
    pub(crate) fn from_ascii_digits(negative: bool, digits: &[u8]) -> Self {
        let first = digits.iter().position(|&d| d != b'0');
        let digits = match first {
            Some(i) => digits.get(i..).unwrap_or_default(),
            None => b"0",
        };
        let digits: String = digits.iter().map(|&d| char::from(d)).collect();
        let negative = negative && digits != "0";
        PyInt { negative, digits }
    }

    /// Returns `true` if the value is zero (a "false" value in Python).
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert!(PyInt::from(0u64).is_zero());
    /// assert!(!PyInt::from(-1i64).is_zero());
    /// ```
    pub fn is_zero(&self) -> bool {
        self.digits == "0"
    }

    /// Returns `true` if the value is strictly negative.
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert!(PyInt::from(-1i64).is_negative());
    /// assert!(!PyInt::from(0i64).is_negative());
    /// ```
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// Decimal digits of the absolute value, without leading zeros.
    pub(crate) fn digits(&self) -> &str {
        &self.digits
    }

    /// Converts to `i64`, or `None` if the value does not fit.
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert_eq!(PyInt::from(i64::MIN).to_i64(), Some(i64::MIN));
    /// assert_eq!(PyInt::from(u64::MAX).to_i64(), None);
    /// ```
    pub fn to_i64(&self) -> Option<i64> {
        let magnitude: u64 = self.digits.parse().ok()?;
        if self.negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }
}

impl From<u64> for PyInt {
    fn from(n: u64) -> Self {
        PyInt {
            negative: false,
            digits: n.to_string(),
        }
    }
}

impl From<i64> for PyInt {
    fn from(n: i64) -> Self {
        PyInt {
            negative: n < 0,
            digits: n.unsigned_abs().to_string(),
        }
    }
}

impl fmt::Display for PyInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        f.write_str(&self.digits)
    }
}

impl Ord for PyInt {
    fn cmp(&self, other: &Self) -> Ordering {
        // Canonical digits: a longer magnitude is a larger magnitude.
        let magnitude = || {
            self.digits
                .len()
                .cmp(&other.digits.len())
                .then_with(|| self.digits.cmp(&other.digits))
        };
        match (self.negative, other.negative) {
            (false, false) => magnitude(),
            (true, true) => magnitude().reverse(),
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
        }
    }
}

impl PartialOrd for PyInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_form() {
        let n = PyInt::from_ascii_digits(false, b"007");
        assert_eq!(n, PyInt::from(7u64));
        let z = PyInt::from_ascii_digits(true, b"000");
        assert_eq!(z, PyInt::from(0i64));
        assert!(!z.is_negative());
        assert_eq!(z.to_string(), "0");
    }

    #[test]
    fn ordering() {
        let mut v: Vec<PyInt> = [10i64, -3, 0, 2, -20, 999]
            .into_iter()
            .map(PyInt::from)
            .collect();
        v.push(PyInt::from_ascii_digits(
            false,
            b"123456789012345678901234567890",
        ));
        v.push(PyInt::from_ascii_digits(
            true,
            b"123456789012345678901234567890",
        ));
        v.sort();
        let s: Vec<String> = v.iter().map(ToString::to_string).collect();
        assert_eq!(
            s,
            [
                "-123456789012345678901234567890",
                "-20",
                "-3",
                "0",
                "2",
                "10",
                "999",
                "123456789012345678901234567890"
            ]
        );
    }

    #[test]
    fn to_i64_bounds() {
        assert_eq!(PyInt::from(i64::MAX).to_i64(), Some(i64::MAX));
        assert_eq!(
            PyInt::from_ascii_digits(true, b"9223372036854775809").to_i64(),
            None
        );
        assert_eq!(PyInt::from(i64::MIN).to_string(), "-9223372036854775808");
    }
}
