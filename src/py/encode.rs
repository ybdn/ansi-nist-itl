//! `str.encode('latin-1')` and `str.encode('ascii')`, as CPython 3.12
//! implements them (SPEC §5.7, §9.5).

use std::fmt;

/// `UnicodeEncodeError` of the Latin-1 or ASCII codec.
///
/// CPython reports the first run of consecutive characters that the codec
/// cannot encode, with positions counted in characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EncodeError {
    /// Name of the codec.
    codec: &'static str,
    /// First code point the codec cannot encode.
    limit: u32,
    /// First character of the run.
    first: char,
    /// Position of the run, in characters, end excluded.
    start: usize,
    end: usize,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' codec can't encode ", self.codec)?;
        if self.end - self.start == 1 {
            let c = u32::from(self.first);
            if c <= 0xFF {
                write!(f, "character '\\x{c:02x}'")?;
            } else if c <= 0xFFFF {
                write!(f, "character '\\u{c:04x}'")?;
            } else {
                write!(f, "character '\\U{c:08x}'")?;
            }
            write!(f, " in position {}", self.start)?;
        } else {
            write!(f, "characters in position {}-{}", self.start, self.end - 1)?;
        }
        write!(f, ": ordinal not in range({})", self.limit)
    }
}

/// `s.encode(codec)` for a codec mapping each code point below `limit` to
/// one byte.
fn encode(s: &str, codec: &'static str, limit: u32) -> Result<Vec<u8>, EncodeError> {
    let fits = |c: char| u32::from(c) < limit;
    let mut out = Vec::with_capacity(s.len());
    let mut chars = s.chars().enumerate().peekable();
    while let Some((i, c)) = chars.next() {
        match u8::try_from(c) {
            Ok(b) if fits(c) => out.push(b),
            _ => {
                let mut end = i + 1;
                while chars.next_if(|&(_, c)| !fits(c)).is_some() {
                    end += 1;
                }
                return Err(EncodeError {
                    codec,
                    limit,
                    first: c,
                    start: i,
                    end,
                });
            }
        }
    }
    Ok(out)
}

/// `s.encode('latin-1')`.
pub(crate) fn encode_latin1(s: &str) -> Result<Vec<u8>, EncodeError> {
    encode(s, "latin-1", 0x100)
}

/// `b.decode('latin-1')`: each byte is the code point of its value.
pub(crate) fn decode_latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

/// `s.encode('ascii')`.
pub(crate) fn encode_ascii(s: &str) -> Result<Vec<u8>, EncodeError> {
    encode(s, "ascii", 0x80)
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
            encode_ascii("bé€b").unwrap_err().to_string(),
            "'ascii' codec can't encode characters in position 1-2: ordinal not in range(128)"
        );
        assert_eq!(
            encode_ascii("\0é").unwrap_err().to_string(),
            "'ascii' codec can't encode character '\\xe9' in position 1: ordinal not in range(128)"
        );
        assert_eq!(encode_ascii("<4sB").unwrap(), b"<4sB");
        assert_eq!(
            err("éé\u{100}\u{1F600}"),
            "'latin-1' codec can't encode characters in position 2-3: ordinal not in range(256)"
        );
    }
}
