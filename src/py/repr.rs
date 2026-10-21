//! `repr()` of `str`, `bytes` and `float` (SPEC §3.3, §3.4).

use std::fmt::Write;

/// Delimiter chosen by CPython: `"` if the text has a `'` and no `"`, else `'`.
fn quote(has_single: bool, has_double: bool) -> char {
    if has_single && !has_double { '"' } else { '\'' }
}

/// `repr(s)` for a `str`.
pub(crate) fn repr_str(s: &str) -> String {
    repr_str_limited(s, usize::MAX)
}

/// `'%.<limit>R' % s`: `repr(s)` cut after `limit` characters.
///
/// Only the needed prefix of `s` is escaped, so the cost stays bounded by
/// `limit` (plus one scan for the delimiter) whatever the size of `s`.
pub(crate) fn repr_str_limited(s: &str, limit: usize) -> String {
    let q = quote(s.contains('\''), s.contains('"'));
    let mut out = String::new();
    let mut count = 0usize;
    let mut push = |out: &mut String, piece: &str| {
        for c in piece.chars() {
            if count == limit {
                return false;
            }
            out.push(c);
            count += 1;
        }
        true
    };
    let mut buf = String::new();
    if !push(&mut out, q.encode_utf8(&mut [0; 4])) {
        return out;
    }
    for c in s.chars() {
        buf.clear();
        match c {
            '\\' => buf.push_str("\\\\"),
            '\t' => buf.push_str("\\t"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            c if c == q => {
                buf.push('\\');
                buf.push(c);
            }
            c if super::is_printable(c) => buf.push(c),
            c => {
                let n = u32::from(c);
                // Writing to a String cannot fail.
                let _ = if n <= 0xFF {
                    write!(buf, "\\x{n:02x}")
                } else if n <= 0xFFFF {
                    write!(buf, "\\u{n:04x}")
                } else {
                    write!(buf, "\\U{n:08x}")
                };
            }
        }
        if !push(&mut out, &buf) {
            return out;
        }
    }
    push(&mut out, q.encode_utf8(&mut [0; 4]));
    out
}

/// `repr(b)` for a `bytes`, `b` prefix included.
pub(crate) fn repr_bytes(b: &[u8]) -> String {
    let q = quote(b.contains(&b'\''), b.contains(&b'"'));
    let mut out = String::with_capacity(b.len() + 3);
    out.push('b');
    out.push(q);
    for &byte in b {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            _ if char::from(byte) == q => {
                out.push('\\');
                out.push(q);
            }
            0x20..0x7F => out.push(char::from(byte)),
            _ => {
                let _ = write!(out, "\\x{byte:02x}");
            }
        }
    }
    out.push(q);
    out
}

/// Shortest digits that round-trip to `f`, in Rust's `{:e}` layout (`-1.5e-5`).
///
/// Like CPython (`dtoa` mode 0), among the shortest candidates, picks the one
/// closest to the exact value of `f`. Rust's `{:e}` gives the shortest length
/// but not always that candidate (`-8969414945958.063` instead of `.062`);
/// the correctly rounded value at that length, when it round-trips, is it.
fn shortest_sci(f: f64) -> String {
    let shortest = format!("{f:e}");
    let mantissa = shortest.split_once('e').map_or(&*shortest, |(m, _)| m);
    let len = mantissa.bytes().filter(u8::is_ascii_digit).count();
    let rounded = format!("{:.*e}", len.saturating_sub(1), f);
    if rounded.parse::<f64>() == Ok(f) {
        rounded
    } else {
        shortest
    }
}

/// `repr(f)` (and `str(f)`) for a `float`.
#[allow(dead_code)] // Not used yet, see `py/mod.rs`.
pub(crate) fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_owned();
    }
    if f.is_infinite() {
        return if f < 0.0 { "-inf" } else { "inf" }.to_owned();
    }
    let sci = shortest_sci(f);
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    if !(-4..16).contains(&exp) {
        let exp_sign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{mantissa}e{exp_sign}{:02}", exp.unsigned_abs());
    }
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    if exp < 0 {
        let zeros = "0".repeat(usize::try_from(-exp - 1).unwrap_or(0));
        return format!("{sign}0.{zeros}{digits}");
    }
    let int_len = usize::try_from(exp).unwrap_or(0) + 1;
    if digits.len() <= int_len {
        let zeros = "0".repeat(int_len - digits.len());
        format!("{sign}{digits}{zeros}.0")
    } else {
        let (int, frac) = digits.split_at(int_len);
        format!("{sign}{int}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_quotes() {
        assert_eq!(repr_str(""), "''");
        assert_eq!(repr_str("abc"), "'abc'");
        assert_eq!(repr_str("a'b"), "\"a'b\"");
        assert_eq!(repr_str("c\"d"), "'c\"d'");
        assert_eq!(repr_str("a'b\"c"), "'a\\'b\"c'");
    }

    #[test]
    fn str_escapes() {
        // SPEC §3.3 / C84: 'x\ty\xa0\xad\\'
        assert_eq!(repr_str("x\ty\u{a0}\u{ad}\\"), "'x\\ty\\xa0\\xad\\\\'");
        assert_eq!(repr_str("\n\r\x00\x1c\x7f"), "'\\n\\r\\x00\\x1c\\x7f'");
        assert_eq!(repr_str("é€ Ã©"), "'é€ Ã©'");
        assert_eq!(repr_str("\u{200b}\u{10ffff}"), "'\\u200b\\U0010ffff'");
        assert_eq!(repr_str("\u{e000}"), "'\\ue000'");
    }

    #[test]
    fn str_limited() {
        assert_eq!(repr_str_limited("abc", 200), "'abc'");
        assert_eq!(repr_str_limited("abc", 5), "'abc'");
        assert_eq!(repr_str_limited("abc", 4), "'abc");
        assert_eq!(repr_str_limited("\x00\x00", 4), "'\\x0");
        assert_eq!(repr_str_limited("abc", 0), "");
    }

    #[test]
    fn bytes() {
        assert_eq!(repr_bytes(b""), "b''");
        assert_eq!(repr_bytes(b"IMG"), "b'IMG'");
        assert_eq!(repr_bytes(b"a'b"), "b\"a'b\"");
        assert_eq!(repr_bytes(b"a'b\"c"), "b'a\\'b\"c'");
        assert_eq!(
            repr_bytes(b"\\\t\n\r\x00\x1f\x7f\x80\xff ~"),
            "b'\\\\\\t\\n\\r\\x00\\x1f\\x7f\\x80\\xff ~'"
        );
    }

    #[test]
    fn floats() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (-2.5, "-2.5"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-5, "1.5e-05"),
            (123.456, "123.456"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.2345e16, "1.2345e+16"),
            (9999999999999998.0, "9999999999999998.0"),
            (1e100, "1e+100"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (0.1 + 0.2, "0.30000000000000004"),
            (100.0, "100.0"),
            // Two shortest candidates: CPython keeps the closest one.
            (-8969414945958.062, "-8969414945958.062"),
            (1299025745705376.2, "1299025745705376.2"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ];
        for &(f, expected) in cases {
            assert_eq!(float_repr(f), expected, "{f:e}");
        }
    }
}
