//! `%` and `str.format` conversions used by `nistitl` (SPEC §3.5).

use std::fmt;

use super::repr::repr_str;
use crate::value::PyInt;

/// `'%<width>d'`, `'%0<width>d'`, `'{:<width>}'` and `'{:0<width>}'` for an `int`.
///
/// Right-aligned on `width` characters, with spaces, or with zeros placed
/// after the sign (`-7` on 3 digits gives `-07`). Never truncated.
pub(crate) fn pad_int(n: &PyInt, width: usize, zero: bool) -> String {
    let sign = if n.is_negative() { "-" } else { "" };
    let len = sign.len() + n.digits().len();
    let fill = width.saturating_sub(len);
    if zero {
        format!("{sign}{}{}", "0".repeat(fill), n.digits())
    } else {
        format!("{}{sign}{}", " ".repeat(fill), n.digits())
    }
}

/// `'{:<width>}'` for a `str`: left-aligned, padded with spaces, never truncated.
///
/// The width counts characters, not bytes.
pub(crate) fn pad_str(s: &str, width: usize) -> String {
    let fill = width.saturating_sub(s.chars().count());
    format!("{s}{}", " ".repeat(fill))
}

/// `'{:,}'` for a non-negative `int`: thousands separated by commas.
pub(crate) fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A `printf` template applied to `(record, tag)`, like `Field.format` (`'%d.%03d:'`).
///
/// Only literal text, `%%` and exactly two `%[0][width]d` conversions are
/// supported (SPEC §3.5, §11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintfTemplate {
    source: String,
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Literal(String),
    Int { zero: bool, width: usize },
}

/// A template outside the supported subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BadFormat(String);

impl fmt::Display for BadFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported tag format {}: expected text with exactly two %[0][width]d conversions",
            repr_str(&self.0)
        )
    }
}

impl PrintfTemplate {
    /// Parses `template`.
    pub(crate) fn parse(template: &str) -> Result<Self, BadFormat> {
        let bad = || BadFormat(template.to_owned());
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut conversions = 0;
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                literal.push(c);
                continue;
            }
            if chars.next_if_eq(&'%').is_some() {
                literal.push('%');
                continue;
            }
            let zero = chars.next_if_eq(&'0').is_some();
            let mut width = 0usize;
            while let Some(d) = chars.next_if(char::is_ascii_digit) {
                let d = d.to_digit(10).map(|d| d as usize).ok_or_else(bad)?;
                width = width
                    .checked_mul(10)
                    .and_then(|w| w.checked_add(d))
                    .ok_or_else(bad)?;
            }
            if chars.next() != Some('d') {
                return Err(bad());
            }
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(&mut literal)));
            }
            parts.push(Part::Int { zero, width });
            conversions += 1;
        }
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        if conversions != 2 {
            return Err(bad());
        }
        Ok(PrintfTemplate {
            source: template.to_owned(),
            parts,
        })
    }

    /// The template as given to [`PrintfTemplate::parse`].
    pub(crate) fn as_str(&self) -> &str {
        &self.source
    }

    /// `template % (a, b)`.
    pub(crate) fn render(&self, a: &PyInt, b: &PyInt) -> String {
        let mut args = [a, b].into_iter();
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(s) => out.push_str(s),
                Part::Int { zero, width } => {
                    if let Some(n) = args.next() {
                        out.push_str(&pad_int(n, *width, *zero));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(n: i64) -> PyInt {
        PyInt::from(n)
    }

    #[test]
    fn default_tag_format() {
        let t = PrintfTemplate::parse("%d.%03d:").unwrap();
        assert_eq!(t.as_str(), "%d.%03d:");
        assert_eq!(t.render(&int(2), &int(3)), "2.003:");
        assert_eq!(t.render(&int(2), &int(1000)), "2.1000:");
        assert_eq!(t.render(&int(2), &int(-7)), "2.-07:");
        assert_eq!(t.render(&int(-1), &int(0)), "-1.000:");
        let big = PyInt::from_ascii_digits(false, b"12345678901234567890");
        assert_eq!(t.render(&int(2), &big), "2.12345678901234567890:");
    }

    #[test]
    fn other_templates() {
        let t = PrintfTemplate::parse("%02d.%3d%%:").unwrap();
        assert_eq!(t.render(&int(2), &int(5)), "02.  5%:");
        let t = PrintfTemplate::parse("%%%d%d").unwrap();
        assert_eq!(t.render(&int(1), &int(-2)), "%1-2");
        let t = PrintfTemplate::parse("%0d.%d").unwrap();
        assert_eq!(t.render(&int(1), &int(2)), "1.2");
        let t = PrintfTemplate::parse("é%d.%003d€").unwrap();
        assert_eq!(t.render(&int(1), &int(2)), "é1.002€");
    }

    #[test]
    fn bad_templates() {
        for t in [
            "",
            "%d",
            "%d%d%d",
            "%s.%d",
            "%d.%-3d",
            "%d.%+d",
            "%d.% d",
            "%d.%i",
            "%d.%x",
            "%d.%",
            "%d%d%",
            "%d.%3",
            "%d.%99999999999999999999999d",
        ] {
            assert_eq!(
                PrintfTemplate::parse(t),
                Err(BadFormat(t.to_owned())),
                "{t:?}"
            );
        }
        assert_eq!(
            BadFormat("%s".to_owned()).to_string(),
            "unsupported tag format '%s': expected text with exactly two %[0][width]d conversions"
        );
    }

    #[test]
    fn padding() {
        // '{:2}'.format(int)
        assert_eq!(pad_int(&int(2), 2, false), " 2");
        assert_eq!(pad_int(&int(-2), 2, false), "-2");
        assert_eq!(pad_int(&int(123), 2, false), "123");
        // '{:03}'.format(int)
        assert_eq!(pad_int(&int(1), 3, true), "001");
        assert_eq!(pad_int(&int(-7), 3, true), "-07");
        assert_eq!(pad_int(&int(1234), 3, true), "1234");
        // '{:30}'.format(str)
        assert_eq!(pad_str("LEN", 6), "LEN   ");
        assert_eq!(pad_str("é", 3), "é  ");
        assert_eq!(pad_str("toolong", 3), "toolong");
        assert_eq!(pad_str("", 30).len(), 30);
    }

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(u64::MAX), "18,446,744,073,709,551,615");
    }
}
