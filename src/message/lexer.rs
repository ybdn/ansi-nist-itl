//! Lexer of tagged records (`parse_record`, SPEC §10.7) and the patterns of
//! `nistitl` (`RE_TAG_LEN`, `RE_TAG_BEGIN`, `RE_TAG_CONTENT`).

use crate::error::{Error, Result};
use crate::py::{decimal_value, decode_latin1};
use crate::{FS, GS, RS, US};

/// Receiver of the events of [`parse_record`], the callbacks of `nistitl`.
///
/// ```
/// use ansi_nist_itl::{RecordVisitor, Result, parse_record};
///
/// #[derive(Default)]
/// struct Log(Vec<String>);
///
/// impl RecordVisitor for Log {
///     fn push_record(&mut self) -> Result<()> {
///         self.0.push("record".into());
///         Ok(())
///     }
///     fn push_field(&mut self, tag: &str, value: Option<&[u8]>) -> Result<()> {
///         self.0.push(format!("field {tag} {value:?}"));
///         Ok(())
///     }
///     fn push_subfield(&mut self, value: Option<&[u8]>) -> Result<()> {
///         self.0.push(format!("subfield {value:?}"));
///         Ok(())
///     }
///     fn push_value(&mut self, value: &[u8]) -> Result<()> {
///         self.0.push(format!("value {value:?}"));
///         Ok(())
///     }
/// }
///
/// let mut log = Log::default();
/// parse_record(b"2.001:20\x1d2.003:a\x1fb\x1ec\x1c", &mut log)?;
/// assert_eq!(
///     log.0,
///     [
///         "field 2.001: Some([50, 48])",
///         "value [97]",
///         "value [98]",
///         "subfield None",
///         "subfield Some([99])",
///         "field 2.003: None",
///         "record",
///     ]
/// );
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
pub trait RecordVisitor {
    /// End of a record.
    fn push_record(&mut self) -> Result<()>;

    /// End of a field: `tag` is its label, such as `"2.003:"`; `value` is
    /// its content if it has neither subfields nor items, `None` otherwise.
    fn push_field(&mut self, tag: &str, value: Option<&[u8]>) -> Result<()>;

    /// End of a subfield: `value` is its content if it has no items, `None`
    /// otherwise.
    fn push_subfield(&mut self, value: Option<&[u8]>) -> Result<()>;

    /// An item of the current subfield.
    fn push_value(&mut self, value: &[u8]) -> Result<()>;
}

/// Splits tagged records into fields, subfields and items, and reports them
/// to `visitor` (`parse_record` of `nistitl`).
///
/// A final FS is dropped; the buffer is then split on FS, and an empty
/// record stops the parsing. Each field must start with a label
/// `<digits>.<digits>:`, otherwise `BAD_RECORD`. An error of the visitor
/// stops the parsing; its position gets the offset of the field, if it has
/// none.
///
/// ```
/// use ansi_nist_itl::{RecordVisitor, Result, parse_record};
///
/// struct Count(usize);
///
/// impl RecordVisitor for Count {
///     fn push_record(&mut self) -> Result<()> { Ok(()) }
///     fn push_field(&mut self, _: &str, _: Option<&[u8]>) -> Result<()> {
///         self.0 += 1;
///         Ok(())
///     }
///     fn push_subfield(&mut self, _: Option<&[u8]>) -> Result<()> { Ok(()) }
///     fn push_value(&mut self, _: &[u8]) -> Result<()> { Ok(()) }
/// }
///
/// let mut count = Count(0);
/// let err = parse_record(b"2.001:9\x1dbad\x1c", &mut count).unwrap_err();
/// assert_eq!(err.to_string(), "BAD_RECORD: Invalid record data");
/// assert_eq!(err.position().offset, Some(8));
/// assert_eq!(count.0, 1);
/// ```
pub fn parse_record<V: RecordVisitor + ?Sized>(buffer: &[u8], visitor: &mut V) -> Result<()> {
    lex(buffer, 0, &mut Plain(visitor))
}

/// Receiver of the lexer inside the crate: a visitor that also learns where
/// each field is, and may skip a field without label (tolerant mode).
pub(crate) trait Lexed: RecordVisitor {
    /// A field at `start..end` of the input, its label ending at
    /// `label_end`, about to be visited.
    fn field_span(&mut self, start: usize, label_end: usize, end: usize) {
        let _ = (start, label_end, end);
    }

    /// A field without label (`BAD_RECORD`): an error, unless the receiver
    /// skips the field.
    fn bad_field(&mut self, error: Error) -> Result<()> {
        Err(error)
    }
}

/// A public visitor, with the default behaviour of [`Lexed`].
struct Plain<'a, V: ?Sized>(&'a mut V);

impl<V: RecordVisitor + ?Sized> RecordVisitor for Plain<'_, V> {
    fn push_record(&mut self) -> Result<()> {
        self.0.push_record()
    }

    fn push_field(&mut self, tag: &str, value: Option<&[u8]>) -> Result<()> {
        self.0.push_field(tag, value)
    }

    fn push_subfield(&mut self, value: Option<&[u8]>) -> Result<()> {
        self.0.push_subfield(value)
    }

    fn push_value(&mut self, value: &[u8]) -> Result<()> {
        self.0.push_value(value)
    }
}

impl<V: RecordVisitor + ?Sized> Lexed for Plain<'_, V> {}

/// [`parse_record`] on a buffer found at `base` in the input.
pub(crate) fn lex<V: Lexed + ?Sized>(buffer: &[u8], base: usize, visitor: &mut V) -> Result<()> {
    let buffer = match buffer.split_last() {
        Some((&FS, rest)) => rest,
        _ => buffer,
    };
    let mut record_start = base;
    for record in buffer.split(|&b| b == FS) {
        if record.is_empty() {
            break;
        }
        let mut field_start = record_start;
        for field in record.split(|&b| b == GS) {
            let at = |mut e: Error| {
                let position = e.position_mut();
                position.offset = position.offset.or(Some(field_start));
                e
            };
            match match_tag_content(field) {
                Some(label) => {
                    let end = field_start + field.len();
                    visitor.field_span(field_start, field_start + label, end);
                    lex_field(field, label, visitor).map_err(at)?;
                }
                None => visitor.bad_field(at(Error::bad_record()))?,
            }
            field_start += field.len() + 1;
        }
        visitor.push_record()?;
        record_start += record.len() + 1;
    }
    Ok(())
}

fn lex_field<V: RecordVisitor + ?Sized>(field: &[u8], label: usize, visitor: &mut V) -> Result<()> {
    let (tag, content) = field.split_at(label);
    let tag = decode_latin1(tag);
    let subfields: Vec<&[u8]> = content.split(|&b| b == RS).collect();
    let mut last_items = 0;
    for subfield in &subfields {
        let items: Vec<&[u8]> = subfield.split(|&b| b == US).collect();
        last_items = items.len();
        match items[..] {
            [item] if subfields.len() == 1 => {
                visitor.push_field(&tag, Some(item))?;
                break;
            }
            [item] => visitor.push_subfield(Some(item))?,
            _ => {
                for item in items {
                    visitor.push_value(item)?;
                }
                visitor.push_subfield(None)?;
            }
        }
    }
    if subfields.len() != 1 || last_items != 1 {
        visitor.push_field(&tag, None)?;
    }
    Ok(())
}

/// Length of the run of ASCII digits at the start of `b`.
fn digits(b: &[u8]) -> usize {
    b.iter().take_while(|c| c.is_ascii_digit()).count()
}

/// `\d+\.\d+:` at the start of `b` (bytes pattern: ASCII digits): the length
/// of the label.
fn match_label(b: &[u8]) -> Option<usize> {
    let record = digits(b);
    if record == 0 || b.get(record) != Some(&b'.') {
        return None;
    }
    let tag = digits(b.get(record + 1..)?);
    let end = record + 1 + tag;
    (tag > 0 && b.get(end) == Some(&b':')).then_some(end + 1)
}

/// `RE_TAG_CONTENT.match(field)`: the length of the label.
pub(crate) fn match_tag_content(field: &[u8]) -> Option<usize> {
    match_label(field)
}

/// `RE_TAG_LEN.match(buffer)`: the digits of the record number and of the
/// value, as in `1.001:123`.
pub(crate) fn match_tag_len(buffer: &[u8]) -> Option<(&[u8], &[u8])> {
    let label = match_label(buffer)?;
    let record = digits(buffer);
    let value_len = digits(buffer.get(label..)?);
    if value_len == 0 {
        return None;
    }
    Some((buffer.get(..record)?, buffer.get(label..label + value_len)?))
}

/// `RE_TAG_BEGIN.match(text)` (text pattern: `\d` is any decimal digit): the
/// record number and the tag, as text.
pub(crate) fn match_tag_begin(text: &str) -> Option<(&str, &str)> {
    let digits = |s: &str| {
        s.char_indices()
            .find(|&(_, c)| decimal_value(c).is_none())
            .map_or(s.len(), |(i, _)| i)
    };
    let record = digits(text);
    let rest = text.get(record..)?.strip_prefix('.')?;
    let tag = digits(rest);
    if record == 0 || tag == 0 || !rest.get(tag..)?.starts_with(':') {
        return None;
    }
    Some((text.get(..record)?, rest.get(..tag)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(match_tag_content(b"2.003:abc"), Some(6));
        assert_eq!(match_tag_content(b"002.0002:"), Some(9));
        assert_eq!(match_tag_content(b"2.:a"), None);
        assert_eq!(match_tag_content(b".3:a"), None);
        assert_eq!(match_tag_content(b"2.3"), None);
        assert_eq!(match_tag_content(b""), None);
        assert_eq!(
            match_tag_len(b"10.001:40\x1d"),
            Some((&b"10"[..], &b"40"[..]))
        );
        assert_eq!(match_tag_len(b"10.001:\x1d"), None);
        assert_eq!(match_tag_len(b"10.001:"), None);
        assert_eq!(match_tag_begin("14.999:x"), Some(("14", "999")));
        assert_eq!(match_tag_begin("1\u{661}.2:"), Some(("1\u{661}", "2")));
        assert_eq!(match_tag_begin("X999:"), None);
        assert_eq!(match_tag_begin("1.2"), None);
    }
}
