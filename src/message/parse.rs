//! Parsing of a message (`Message.parse`, SPEC §10.8–10.10).

use super::lexer::{lex, match_tag_begin, match_tag_len};
use super::{Message, RecordVisitor, int_of, int_of_scalar};
use crate::alias;
use crate::bytes::Bytes;
use crate::error::{Error, ErrorKind, Position, Result, Warning};
use crate::field::{AnyField, BinaryField, Field};
use crate::py::{decode_latin1, py_int};
use crate::record::{AsciiRecord, BinaryRecord, FieldKey, Record};
use crate::subfield::SubField;
#[cfg(test)]
use crate::value::Value;
use crate::value::{Item, PyInt, Scalar, SubValue};
use crate::{FS, GS};

/// How [`Message::parse_with`] reads a buffer.
///
/// ```
/// use ansi_nist_itl::{Message, ParseOptions};
///
/// let options = ParseOptions::strict();
/// assert_eq!(options, ParseOptions::default());
/// assert!(Message::parse_with(b"", options).is_err());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    _private: (),
}

impl ParseOptions {
    /// The behaviour of `nistitl`: the first error stops the parsing (the
    /// default).
    ///
    /// ```
    /// use ansi_nist_itl::ParseOptions;
    ///
    /// assert_eq!(ParseOptions::strict(), ParseOptions::default());
    /// ```
    pub fn strict() -> Self {
        ParseOptions::default()
    }
}

/// Result of [`Message::parse_with`].
///
/// ```
/// use ansi_nist_itl::{Message, ParseOptions};
///
/// let bytes = Message::new().to_bytes()?;
/// let parsed = Message::parse_with(&bytes, ParseOptions::strict())?;
/// assert_eq!(parsed.message.len(), 1);
/// assert!(parsed.warnings.is_empty());
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Parsed {
    /// The message read.
    pub message: Message,
    /// Warnings raised while parsing.
    pub warnings: Vec<Warning>,
}

impl Message {
    /// Reads a message (`Message().parse(buffer)`), as `nistitl` does.
    ///
    /// The buffer is copied once into a shared buffer; binary data are views
    /// on it. Use [`Message::parse_shared`] to avoid the copy.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("IDC", 3)?;
    /// msg.push(r)?;
    /// let bytes = msg.to_bytes()?;
    ///
    /// let read = Message::parse(&bytes)?;
    /// assert_eq!(read.len(), 2);
    ///
    /// let err = Message::parse(&bytes[..bytes.len() - 1]).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 2"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn parse(buffer: &[u8]) -> Result<Message> {
        Message::parse_shared(buffer)
    }

    /// [`Message::parse`] on a shared buffer, without copying it.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use ansi_nist_itl::{Bytes, Message};
    ///
    /// let bytes: Arc<[u8]> = Message::new().to_bytes()?.into();
    /// let msg = Message::parse_shared(Bytes::from(bytes))?;
    /// assert_eq!(msg.len(), 1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn parse_shared(buffer: impl Into<Bytes>) -> Result<Message> {
        Parser::run(buffer.into()).map(|(message, _)| message)
    }

    /// [`Message::parse`], also returning the warnings that `nistitl` emits
    /// (`DeprecationWarning` for records of type 3, 5 and 6).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, BinaryRecord, Message, Warning};
    ///
    /// let mut msg = Message::new();
    /// msg.push(BinaryRecord::new(3))?;
    /// let (read, warnings) = Message::parse_with_warnings(&msg.to_bytes()?)?;
    /// assert_eq!(read.len(), 2);
    /// assert_eq!(warnings, [Warning::Deprecated { record_type: 3 }]);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn parse_with_warnings(buffer: &[u8]) -> Result<(Message, Vec<Warning>)> {
        Parser::run(Bytes::from(buffer))
    }

    /// [`Message::parse`] with options.
    ///
    /// ```
    /// use ansi_nist_itl::{Message, ParseOptions};
    ///
    /// let bytes = Message::new().to_bytes()?;
    /// let parsed = Message::parse_with(&bytes, ParseOptions::strict())?;
    /// assert_eq!(parsed.message.len(), 1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn parse_with(buffer: &[u8], options: ParseOptions) -> Result<Parsed> {
        let ParseOptions { _private: () } = options;
        let (message, warnings) = Parser::run(Bytes::from(buffer))?;
        Ok(Parsed { message, warnings })
    }
}

/// Fills the parts of the position of `e` that are not set yet.
fn locate(
    mut e: Error,
    record_index: Option<usize>,
    record_type: Option<u32>,
    offset: Option<usize>,
) -> Error {
    let p = e.position_mut();
    p.record_index = p.record_index.or(record_index);
    p.record_type = p.record_type.or(record_type);
    p.offset = p.offset.or(offset);
    e
}

/// `bytes.find(needle, start)`.
fn find(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    let rest = haystack.get(start..)?;
    if needle.is_empty() {
        return Some(start);
    }
    rest.windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + start)
}

fn to_u32(n: &PyInt) -> Option<u32> {
    n.to_i64().and_then(|n| u32::try_from(n).ok())
}

fn to_usize(n: &PyInt) -> Option<usize> {
    n.to_i64().and_then(|n| usize::try_from(n).ok())
}

/// `'%s' % record` for a record number.
fn digits_text(b: &[u8]) -> String {
    decode_latin1(b)
}

/// Visitor of `nistitl` (`_Parser`, SPEC §10.8): fills a tagged record.
struct TextParser<'a> {
    record: &'a mut AsciiRecord,
    subfields: Vec<SubField>,
    values: Vec<Item>,
    closed: bool,
}

impl<'a> TextParser<'a> {
    fn new(record: &'a mut AsciiRecord) -> Self {
        TextParser {
            record,
            subfields: Vec::new(),
            values: Vec::new(),
            closed: false,
        }
    }
}

impl RecordVisitor for TextParser<'_> {
    fn push_record(&mut self) -> Result<()> {
        self.closed = true;
        Ok(())
    }

    fn push_field(&mut self, label: &str, value: Option<&[u8]>) -> Result<()> {
        let record_type = self.record.record_type();
        let Some((record, tag)) = match_tag_begin(label) else {
            return Err(Error::illegal_tag_name(label));
        };
        let record = py_int(record)?;
        if to_u32(&record) != Some(record_type) {
            return Err(Error::illegal_record_number(label).with_position(Position {
                tag: py_int(tag).ok(),
                ..Position::default()
            }));
        }
        let tag = py_int(tag)?;
        let position = Position {
            record_type: Some(record_type),
            tag: Some(tag.clone()),
            ..Position::default()
        };
        let exists = self.record.field(&tag).is_some_and(AnyField::is_truthy);
        if !exists {
            self.record
                .add_field(Field::new(record_type, tag.clone(), "")?)?;
        }
        let field = match self.record.field_mut(FieldKey::Tag(tag.clone())) {
            Some(AnyField::Text(f)) => f,
            _ => return Err(Error::no_attribute("BinaryField", "reset").with_position(position)),
        };
        field.reset();
        match value.filter(|v| !v.is_empty()) {
            Some(value) => {
                let text = decode_latin1(value);
                let one = PyInt::from(1);
                let two = PyInt::from(2);
                // IDC and LEN are numeric.
                if (record_type != 1 && tag == two) || tag == one {
                    let n = py_int(&text).map_err(|e| Error::from(e).with_position(position))?;
                    field.set_value(n)?;
                } else {
                    field.set_value(text)?;
                }
            }
            None => field.add_subfields(std::mem::take(&mut self.subfields))?,
        }
        self.subfields.clear();
        self.values.clear();
        Ok(())
    }

    fn push_subfield(&mut self, value: Option<&[u8]>) -> Result<()> {
        let subfield = match value.filter(|v| !v.is_empty()) {
            Some(value) => SubField::new(decode_latin1(value), "SI")?,
            None => {
                let mut sf = SubField::default();
                sf.add_values(std::mem::take(&mut self.values))?;
                sf
            }
        };
        self.subfields.push(subfield);
        self.values.clear();
        Ok(())
    }

    fn push_value(&mut self, value: &[u8]) -> Result<()> {
        self.values.push(Item::from(decode_latin1(value)));
        Ok(())
    }
}

/// State of `Message.parse`.
struct Parser {
    buffer: Bytes,
    records: Vec<Record>,
    warnings: Vec<Warning>,
    /// Number of the last tagged record read (`record` in `nistitl`).
    last_tagged: Option<PyInt>,
    /// Last search of the data label: from where, for which label, and what
    /// it found. Offsets only grow, so a match after the new start stands.
    data_search: Option<(usize, Vec<u8>, Option<usize>)>,
    /// Characters of a text CNT, indexed by the binary records; dropped
    /// whenever the Type-1 is read again.
    cnt_chars: Option<Vec<char>>,
}

impl Parser {
    fn run(buffer: Bytes) -> Result<(Message, Vec<Warning>)> {
        let mut parser = Parser {
            buffer,
            records: Vec::new(),
            warnings: Vec::new(),
            last_tagged: None,
            data_search: None,
            cnt_chars: None,
        };
        // `reset(autocreate=False)`: an empty Type-1.
        parser
            .records
            .push(Record::Ascii(AsciiRecord::with(1, false, true)?));
        parser.parse()?;
        let message = Message {
            records: parser.records,
        };
        check_cnt(&message)?;
        Ok((message, parser.warnings))
    }

    fn parse(&mut self) -> Result<()> {
        let len = self.buffer.len();
        let mut offset = 0usize;
        while offset + 4 < len {
            let length = match match_tag_len(self.buffer.get(offset..).unwrap_or_default()) {
                Some((record, value)) => {
                    let (record, value) = (record.to_vec(), value.to_vec());
                    self.tagged(offset, &record, &value)?
                }
                None => self.binary(offset)?,
            };
            offset += length;
        }
        if offset < len {
            return Err(Error::nist_too_long().with_position(Position {
                offset: Some(offset),
                ..Position::default()
            }));
        }
        Ok(())
    }

    /// `_factory(t, autocreate, True)`.
    fn factory(&mut self, record_type: &PyInt, autocreate: bool) -> Result<Record> {
        let t = to_u32(record_type);
        if let Some(t @ (3 | 5 | 6)) = t {
            self.warnings.push(Warning::Deprecated { record_type: t });
        }
        match t {
            Some(t @ 3..=8) => Ok(Record::Binary(BinaryRecord::new(t))),
            Some(t) => Ok(Record::Ascii(AsciiRecord::with(t, autocreate, true)?)),
            None => Err(Error::unknown_record_type(record_type.clone())),
        }
    }

    /// A tagged record at `offset`; returns its length.
    fn tagged(&mut self, offset: usize, record_digits: &[u8], len_digits: &[u8]) -> Result<usize> {
        let len = self.buffer.len();
        let index = self.records.len();
        let at = |e: Error, t: Option<u32>| locate(e, Some(index), t, Some(offset));
        let record = py_int(&digits_text(record_digits)).map_err(|e| at(e.into(), None))?;
        let length = py_int(&digits_text(len_digits)).map_err(|e| at(e.into(), to_u32(&record)))?;
        self.last_tagged = Some(record.clone());
        let table = to_u32(&record).and_then(alias::table);
        let (Some(record_type), Some(table)) = (to_u32(&record), table) else {
            return Err(at(Error::unknown_record_type(record), None));
        };
        let at = |e: Error| locate(e, Some(index), Some(record_type), Some(offset));
        let data_tag = alias::tag_of(table, alias::DATA);
        let data_label = data_tag.map(|t| format!("{t}:").into_bytes());

        let mut future = None;
        let length = match to_usize(&length)
            .filter(|&l| offset.checked_add(l).is_some_and(|end| end <= len))
        {
            None => {
                future = Some(Error::nist_too_short(&record));
                len - offset
            }
            Some(length) => {
                let pos_data = data_label.as_ref().and_then(|l| self.find_label(l, offset));
                let pos_fs = find(&self.buffer, &[FS], offset);
                let first = [pos_fs, pos_data]
                    .into_iter()
                    .flatten()
                    .min()
                    .ok_or_else(|| at(Error::empty_sequence()))?;
                let new_length = first - offset;
                if new_length > length {
                    future = Some(Error::nist_too_long());
                    new_length
                } else {
                    length
                }
            }
        };
        let future = future.map(at);
        let record_buffer = self
            .buffer
            .slice(offset..offset + length)
            .unwrap_or_default();
        let pos = data_label.as_ref().and_then(|l| find(&record_buffer, l, 0));

        match pos {
            Some(pos) if pos > 0 => {
                // Tagged record with binary data: the text part, then the data.
                let pos_end = record_buffer
                    .get(..pos)
                    .and_then(|b| b.iter().rposition(|&c| c == GS));
                let header_start = pos_end.map_or(0, |p| p + 1);
                let header =
                    decode_latin1(record_buffer.get(header_start..pos + 4).unwrap_or_default());
                if match_tag_begin(&header).is_none() {
                    let tag = data_tag.map_or_else(String::new, |t| t.to_string());
                    return Err(locate(
                        Error::illegal_tag_format(tag, header),
                        Some(index),
                        Some(record_type),
                        Some(offset + header_start),
                    ));
                }
                // `record_buffer[:pos_end] + FS`, `pos_end` being -1 if no GS.
                let text_end = pos_end.unwrap_or(record_buffer.len().saturating_sub(1));
                let mut text = record_buffer.get(..text_end).unwrap_or_default().to_vec();
                text.push(FS);
                let record = self.factory(&PyInt::from(record_type), false).map_err(at)?;
                self.push(record, offset)?;
                self.lex_into(index, &text, offset, future.clone())?;
                let start = offset + pos + 4;
                let end = (offset + record_buffer.len()).saturating_sub(1);
                let data = self.buffer.slice(start..end.max(start)).unwrap_or_default();
                let located = |e| locate(e, Some(index), Some(record_type), Some(offset + pos));
                if let (Some(tag), Some(Record::Ascii(r))) = (data_tag, self.records.get_mut(index))
                {
                    let field = BinaryField::new(record_type, tag, data).map_err(located)?;
                    r.add_field(field).map_err(located)?;
                }
            }
            _ if record_type == 1 => {
                self.cnt_chars = None;
                self.lex_into(0, &record_buffer, offset, future.clone())?;
            }
            _ => {
                let record = self.factory(&PyInt::from(record_type), false).map_err(at)?;
                self.push(record, offset)?;
                self.lex_into(index, &record_buffer, offset, future.clone())?;
            }
        }
        match future {
            Some(e) => Err(e),
            None => Ok(length),
        }
    }

    /// `buffer.find(label, offset)`, reusing the previous search.
    fn find_label(&mut self, label: &[u8], offset: usize) -> Option<usize> {
        if let Some((start, last, found)) = &self.data_search {
            if *start <= offset && last == label && found.is_none_or(|p| p >= offset) {
                return *found;
            }
        }
        let found = find(&self.buffer, label, offset);
        self.data_search = Some((offset, label.to_vec(), found));
        found
    }

    /// `self += record`.
    fn push(&mut self, record: Record, offset: usize) -> Result<()> {
        if record.record_type() == 1 {
            return Err(locate(
                Error::cannot_add_type1(),
                Some(self.records.len()),
                Some(1),
                Some(offset),
            ));
        }
        self.records.push(record);
        Ok(())
    }

    /// Lexes `text` into the tagged record at `index`, with the rule of the
    /// `future` error (SPEC §10.9).
    fn lex_into(
        &mut self,
        index: usize,
        text: &[u8],
        offset: usize,
        future: Option<Error>,
    ) -> Result<()> {
        let Some(Record::Ascii(record)) = self.records.get_mut(index) else {
            return Err(Error::index_out_of_range());
        };
        let record_type = record.record_type();
        let mut parser = TextParser::new(record);
        if let Err(e) = lex(text, offset, &mut parser) {
            let e = locate(e, Some(index), Some(record_type), None);
            return Err(match (future, e.kind()) {
                (Some(future), ErrorKind::Nist(_)) => future.with_source(e),
                _ => e,
            });
        }
        if !parser.closed {
            return Err(future.unwrap_or_else(|| {
                locate(
                    Error::record_not_terminated(record_type),
                    Some(index),
                    Some(record_type),
                    Some(offset),
                )
            }));
        }
        Ok(())
    }

    /// A binary record at `offset`; returns its length.
    fn binary(&mut self, offset: usize) -> Result<usize> {
        let index = self.records.len();
        let at = |e: Error| locate(e, Some(index), None, Some(offset));
        // `struct.unpack("!LB", ...)`: the loop leaves at least 5 bytes.
        let head = self.buffer.get(offset..offset + 5).unwrap_or_default();
        let (Some(Ok(length)), Some(&idc)) = (head.get(..4).map(<[u8; 4]>::try_from), head.get(4))
        else {
            return Err(at(Error::unrecognized_binary_record()));
        };
        let length = u32::from_be_bytes(length);
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        let record_type = self.cnt_record_type(index).map_err(at)?;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.buffer.len())
        {
            let record = self
                .last_tagged
                .as_ref()
                .map_or_else(String::new, PyInt::to_string);
            return Err(at(Error::nist_too_short(record)));
        }
        let at = |e: Error| locate(e, Some(index), to_u32(&record_type), Some(offset));
        let mut record = self.factory(&record_type, true).map_err(at)?;
        let value = self
            .buffer
            .slice(offset + 5..offset + length)
            .unwrap_or_default();
        match &mut record {
            Record::Binary(r) => {
                r.set_idc(idc);
                r.set_value(value);
            }
            Record::Ascii(r) => {
                r.set(alias::IDC, u32::from(idc)).map_err(at)?;
                r.set_bytes("value", value).map_err(at)?;
            }
        }
        self.push(record, offset)?;
        Ok(length)
    }

    /// `int(self._records[0].CNT[k][0])`, an `IndexError` being a
    /// `BAD_CONTENT`.
    fn cnt_record_type(&mut self, k: usize) -> Result<PyInt> {
        let Some(Record::Ascii(type1)) = self.records.first() else {
            return Err(Error::index_out_of_range());
        };
        // `record.CNT`: the first field whose alias is CNT.
        let Some(field) = type1.fields().iter().find(|f| f.alias() == alias::CNT) else {
            return Err(Error::bad_attribute_get(alias::CNT, 1));
        };
        let field = match field {
            AnyField::Text(f) => f,
            // `bytes[k]` is an `int`.
            AnyField::Binary(f) => {
                return Err(match f.value().get(k) {
                    Some(_) => Error::not_subscriptable("int"),
                    None => bad_content(),
                });
            }
        };
        let first = if field.is_empty() {
            // `CNT` is a text: `CNT[k]` is its character `k`.
            match field.scalar() {
                Scalar::Text(s) => {
                    let chars = self.cnt_chars.get_or_insert_with(|| s.chars().collect());
                    match chars.get(k) {
                        Some(c) => Some(py_int(&c.to_string())?),
                        None => None,
                    }
                }
                other => return Err(Error::not_subscriptable(scalar_class(other))),
            }
        } else {
            match field.subfields().get(k) {
                Some(sf) => int_of_first(sf)?,
                None => None,
            }
        };
        first.ok_or_else(bad_content)
    }
}

/// `BAD_CONTENT` of an unrecognized binary record, caused by an `IndexError`.
fn bad_content() -> Error {
    Error::unrecognized_binary_record().with_source(Error::index_out_of_range())
}

fn scalar_class(s: &Scalar) -> &'static str {
    match s {
        Scalar::Null => "NoneType",
        Scalar::Bool(_) => "bool",
        Scalar::Int(_) => "int",
        Scalar::Text(_) => "str",
    }
}

/// `int(v[0])` for the value `v` of a subfield; `None` if `v` is empty
/// (`IndexError`).
fn int_of_first(sf: &SubField) -> Result<Option<PyInt>> {
    match sf.value() {
        SubValue::Items(items) => items.first().map(int_of_item).transpose(),
        SubValue::Scalar(Scalar::Text(s)) => s
            .chars()
            .next()
            .map(|c| py_int(&c.to_string()).map_err(Error::from))
            .transpose(),
        SubValue::Scalar(other) => Err(Error::not_subscriptable(scalar_class(&other))),
    }
}

/// `int(item)`.
fn int_of_item(item: &Item) -> Result<PyInt> {
    match item {
        Item::Scalar(s) => int_of_scalar(s),
        Item::List(_) => Err(Error::int_argument("list")),
    }
}

/// Values of the CNT subfield `i`, as Python sees `CNT.value[i]`: its length
/// and its elements, as texts or items.
enum CntEntry {
    Items(Vec<Item>),
    Chars(Vec<char>),
}

impl CntEntry {
    fn of(sf: &SubField) -> Result<CntEntry> {
        match sf.value() {
            SubValue::Items(items) => Ok(CntEntry::Items(items)),
            SubValue::Scalar(Scalar::Text(s)) => Ok(CntEntry::Chars(s.chars().collect())),
            SubValue::Scalar(other) => Err(Error::no_len(scalar_class(&other))),
        }
    }

    fn len(&self) -> usize {
        match self {
            CntEntry::Items(items) => items.len(),
            CntEntry::Chars(chars) => chars.len(),
        }
    }

    /// `int(v[i])`.
    fn int(&self, i: usize) -> Result<PyInt> {
        match self {
            CntEntry::Items(items) => items
                .get(i)
                .map_or_else(|| Err(Error::index_out_of_range()), int_of_item),
            CntEntry::Chars(chars) => chars.get(i).map_or_else(
                || Err(Error::index_out_of_range()),
                |c| py_int(&c.to_string()).map_err(Error::from),
            ),
        }
    }
}

/// Final check of the CNT (SPEC §10.10).
fn check_cnt(message: &Message) -> Result<()> {
    let at = |i: Option<usize>| {
        move |mut e: Error| {
            let p = e.position_mut();
            p.record_index = p.record_index.or(i);
            p.record_type = p.record_type.or(Some(1));
            p.tag = p.tag.clone().or(Some(PyInt::from(3)));
            e
        }
    };
    let cnt = match message.records.first() {
        Some(Record::Ascii(r)) => r.field(alias::CNT),
        _ => None,
    };
    let cnt = match cnt {
        Some(AnyField::Text(f)) => f,
        Some(AnyField::Binary(_)) => return Err(Error::no_len("BinaryField")),
        None => return Err(Error::no_len("NoneType")),
    };
    if cnt.len() != message.records.len() {
        return Err(at(None)(Error::cnt_record_count()));
    }
    for (i, (sf, record)) in cnt.subfields().iter().zip(&message.records).enumerate() {
        let at = at(Some(i));
        let entry = CntEntry::of(sf).map_err(at)?;
        if entry.len() != 2 {
            return Err(at(Error::cnt_value_count(i)));
        }
        let record_type = PyInt::from(record.record_type());
        let first = entry.int(0).map_err(at)?;
        if i == 0 {
            if first != record_type {
                return Err(at(Error::cnt_record_type(i)));
            }
        } else if first != record_type {
            // Python's `and` is lazy: the IDCs are read only now.
            let idc = entry.int(1).map_err(at)?;
            let own = record.idc().and_then(|v| int_of(&v)).map_err(at)?;
            if idc != own {
                return Err(at(Error::cnt_record_type_or_idc(i)));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn message_with(records: Vec<Record>) -> Vec<u8> {
        let mut msg = Message::new();
        for r in records {
            msg.push(r).unwrap();
        }
        msg.to_bytes().unwrap()
    }

    #[test]
    fn binary_data_are_views_on_the_buffer() {
        let mut r14 = AsciiRecord::new(14).unwrap();
        r14.set_bytes("DATA", b"image".to_vec()).unwrap();
        let mut r4 = BinaryRecord::new(4);
        r4.set_value(b"pixels".to_vec());
        let bytes: Arc<[u8]> = message_with(vec![r14.into(), r4.into()]).into();
        let range = bytes.as_ptr_range();
        let msg = Message::parse_shared(Bytes::from(Arc::clone(&bytes))).unwrap();
        let data = msg
            .record(1)
            .unwrap()
            .as_ascii()
            .unwrap()
            .get_bytes("DATA")
            .unwrap();
        let pixels = msg.record(2).unwrap().as_binary().unwrap().value();
        assert_eq!((data, pixels), (&b"image"[..], &b"pixels"[..]));
        assert!(range.contains(&data.as_ptr()));
        assert!(range.contains(&pixels.as_ptr()));
    }

    #[test]
    fn deprecated_types_warn_in_order() {
        let records = [5, 4, 3, 6, 6]
            .into_iter()
            .map(|t| Record::from(BinaryRecord::new(t)))
            .collect();
        let (msg, warnings) = Message::parse_with_warnings(&message_with(records)).unwrap();
        assert_eq!(msg.len(), 6);
        let types: Vec<u32> = warnings
            .iter()
            .map(|Warning::Deprecated { record_type }| *record_type)
            .collect();
        assert_eq!(types, [5, 3, 6, 6]);
    }

    #[test]
    fn round_trip() {
        let mut r2 = AsciiRecord::new(2).unwrap();
        r2.set("IDC", 1).unwrap();
        r2.set("_10", vec![vec!["a", "b"], vec!["c"]]).unwrap();
        let mut r10 = AsciiRecord::new(10).unwrap();
        r10.set("IDC", 2).unwrap();
        r10.set("SRC", "\u{e9}t\u{e9}").unwrap();
        r10.set_bytes("DATA", vec![0x1C, 0x1D, 0, 0xFF]).unwrap();
        let mut r8 = BinaryRecord::new(8);
        r8.set_idc(3);
        r8.pack(
            "!BBBHH",
            &[0.into(), 1.into(), 0.into(), 5.into(), 5.into()],
            b"sig",
        )
        .unwrap();
        let bytes = message_with(vec![r2.into(), r10.into(), r8.into()]);
        let mut msg = Message::parse(&bytes).unwrap();
        assert_eq!(msg.to_bytes().unwrap(), bytes);
        assert_eq!(
            msg.by_key(10, 2)
                .unwrap()
                .as_ascii()
                .unwrap()
                .get("SRC")
                .unwrap(),
            Value::from("\u{e9}t\u{e9}")
        );
    }

    #[test]
    fn no_type1_tag_in_binary_record() {
        // `CNT` announces a Type-1 for a binary record: the IDC fails first.
        let mut msg = Message::new();
        msg.push(BinaryRecord::new(4)).unwrap();
        let mut bytes = msg.to_bytes().unwrap();
        let at = bytes.windows(4).position(|w| w == b"\x1e4\x1f1").unwrap();
        bytes[at + 1] = b'1';
        assert_eq!(
            Message::parse(&bytes).unwrap_err().to_string(),
            "UNKNOWN_ATTRIBUTE: Bad attribute name <IDC> while trying to define a field in record of type 1"
        );
    }
}
