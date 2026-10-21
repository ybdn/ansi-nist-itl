//! `str.encode('latin-1')`, as CPython 3.12 implements it (SPEC §5.7).

use std::fmt;

/// `UnicodeEncodeError` of the Latin-1 codec.
///
/// CPython reports the first run of consecutive characters above U+00FF,
/// with positions counted in characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodeError {
    /// First character of the run.
    first: char,
    /// Position of the run, in characters, end excluded.
    start: usize,
    end: usize,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("'latin-1' codec can't encode ")?;
        if self.end - self.start == 1 {
            let c = u32::from(self.first);
            // Characters above U+00FF only: `\x` never applies.
            if c <= 0xFFFF {
                write!(f, "character '\\u{c:04x}'")?;
            } else {
                write!(f, "character '\\U{c:08x}'")?;
            }
            write!(f, " in position {}", self.start)?;
        } else {
            write!(f, "characters in position {}-{}", self.start, self.end - 1)?;
        }
        f.write_str(": ordinal not in range(256)")
    }
}

/// `s.encode('latin-1')`.
pub(crate) fn encode_latin1(s: &str) -> Result<Vec<u8>, EncodeError> {
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.chars().enumerate().peekable();
    while let Some((i, c)) = chars.next() {
        match u8::try_from(c) {
            Ok(b) => out.push(b),
            Err(_) => {
                let mut end = i + 1;
                while chars.next_if(|&(_, c)| u8::try_from(c).is_err()).is_some() {
                    end += 1;
                }
                return Err(EncodeError {
                    first: c,
                    start: i,
                    end,
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(s: &str) -> String {
        encode_latin1(s).unwrap_err().to_string()
    }

    #[test]
    fn latin1() {
        assert_eq!(encode_latin1("aé\u{ff}\u{0}").unwrap(), b"a\xe9\xff\x00");
        assert_eq!(encode_latin1("").unwrap(), b"");
    }

    /// Messages printed by CPython 3.12.
    #[test]
    fn messages() {
        assert_eq!(
            err("2.003:\u{20ac}"),
            "'latin-1' codec can't encode character '\\u20ac' in position 6: ordinal not in range(256)"
        );
        assert_eq!(
            err("a\u{20ac}\u{20ac}b"),
            "'latin-1' codec can't encode characters in position 1-2: ordinal not in range(256)"
        );
        assert_eq!(
            err("a\u{20ac}b\u{20ac}"),
            "'latin-1' codec can't encode character '\\u20ac' in position 1: ordinal not in range(256)"
        );
        assert_eq!(
            err("\u{1F600}"),
            "'latin-1' codec can't encode character '\\U0001f600' in position 0: ordinal not in range(256)"
        );
        assert_eq!(
            err("x\u{100}"),
            "'latin-1' codec can't encode character '\\u0100' in position 1: ordinal not in range(256)"
        );
        // Positions count characters, not UTF-8 bytes.
        assert_eq!(
            err("éé\u{100}\u{1F600}"),
            "'latin-1' codec can't encode characters in position 2-3: ordinal not in range(256)"
        );
    }
}
