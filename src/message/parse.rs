//! Parsing of a message (`Message.parse`, SPEC §10.8–10.10), and its
//! tolerant mode (SPEC §13.2).

use super::lexer::{Lexed, lex, match_tag_begin, match_tag_len};
use super::{Message, RecordVisitor, int_of, int_of_scalar};
use crate::alias;
use crate::bytes::Bytes;
use crate::error::{
    Diagnostic, DiagnosticCode, Error, ErrorKind, Position, Result, Severity, Warning,
};
use crate::field::{AnyField, BinaryField, Field};
use crate::py::{decode_latin1, py_int};
use crate::record::{AsciiRecord, BinaryRecord, FieldKey, RawField, Record};
use crate::subfield::SubField;
use crate::value::{Item, PyInt, Scalar, SubValue};
use crate::{FS, GS};

/// How [`Message::parse_with`] reads a buffer.
///
/// ```
/// use ansi_nist_itl::{Message, ParseOptions};
///
/// assert_eq!(ParseOptions::strict(), ParseOptions::default());
/// assert!(Message::parse_with(b"", ParseOptions::strict()).is_err());
/// assert!(Message::parse_with(b"", ParseOptions::tolerant()).is_ok());
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParseOptions {
    tolerant: bool,
}

impl ParseOptions {
    /// The behaviour of `nistitl`: the first error stops the parsing (the
    /// default, LIB-14).
    ///
    /// ```
    /// use ansi_nist_itl::ParseOptions;
    ///
    /// assert_eq!(ParseOptions::strict(), ParseOptions::default());
    /// ```
    pub fn strict() -> Self {
        ParseOptions { tolerant: false }
    }

    /// Reads as much as possible (LIB-13): the parsing always succeeds, and
    /// each error `nistitl` would raise becomes a [`Diagnostic`] with a
    /// recovery (SPEC §13.2). The quirks E3 to E5 of `nistitl`, kept as is,
    /// are reported too.
    ///
    /// On a buffer that the strict mode accepts, the message read is the
    /// same, with warnings only.
    ///
    /// ```
    /// use ansi_nist_itl::{DiagnosticCode, Message, ParseOptions, Severity};
    ///
    /// let bytes = Message::new().to_bytes()?;
    /// let truncated = &bytes[..bytes.len() - 3];
    /// assert!(Message::parse(truncated).is_err());
    ///
    /// let parsed = Message::parse_with(truncated, ParseOptions::tolerant())?;
    /// let codes: Vec<_> = parsed.diagnostics.iter().map(|d| d.code).collect();
    /// assert_eq!(codes, [DiagnosticCode::Truncated]);
    /// assert_eq!(parsed.diagnostics[0].severity, Severity::Error);
    /// assert_eq!(parsed.message.tot()?.to_string(), "");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn tolerant() -> Self {
        ParseOptions { tolerant: true }
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
/// assert!(parsed.diagnostics.is_empty());
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Parsed {
    /// The message read.
    pub message: Message,
    /// Problems met by the tolerant mode; always empty in strict mode.
    pub diagnostics: Vec<Diagnostic>,
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
        Parser::run(buffer.into(), false).map(|parsed| parsed.message)
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
        Parser::run(Bytes::from(buffer), false).map(|parsed| (parsed.message, parsed.warnings))
    }

    /// [`Message::parse`] with options: strict (the default) or tolerant,
    /// which always succeeds.
    ///
    /// ```
    /// use ansi_nist_itl::{DiagnosticCode, Message, ParseOptions};
    ///
    /// let bytes = Message::new().to_bytes()?;
    /// let parsed = Message::parse_with(&bytes, ParseOptions::strict())?;
    /// assert_eq!(parsed.message.len(), 1);
    ///
    /// let mut odd = bytes.clone();
    /// odd.extend_from_slice(b"\x00\x00\x00\x05\x01");
    /// let parsed = Message::parse_with(&odd, ParseOptions::tolerant())?;
    /// assert_eq!(parsed.diagnostics[0].code, DiagnosticCode::UnknownBinaryRecord);
    /// assert_eq!(parsed.message.trailing(), b"\x00\x00\x00\x05\x01");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn parse_with(buffer: &[u8], options: ParseOptions) -> Result<Parsed> {
        Parser::run(Bytes::from(buffer), options.tolerant)
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

/// Problems noted in tolerant mode; errors in strict mode.
#[derive(Default)]
struct Notes {
    tolerant: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Notes {
    /// The error `e` stops a strict parsing; a tolerant one notes it and
    /// goes on.
    fn recover(&mut self, code: DiagnosticCode, severity: Severity, e: Error) -> Result<()> {
        if !self.tolerant {
            return Err(e);
        }
        self.diagnostics.push(Diagnostic::new(code, severity, &e));
        Ok(())
    }

    /// A quirk of `nistitl`, kept as is, that the tolerant mode reports
    /// (E3–E5): `nistitl` raises nothing, so the message is the crate's.
    fn note(&mut self, code: DiagnosticCode, message: String, position: Position) {
        if self.tolerant {
            self.diagnostics.push(Diagnostic {
                code,
                severity: Severity::Warning,
                position,
                message,
            });
        }
    }
}

/// Visitor of `nistitl` (`_Parser`, SPEC §10.8): fills a tagged record.
struct TextParser<'a> {
    record: &'a mut AsciiRecord,
    subfields: Vec<SubField>,
    values: Vec<Item>,
    closed: bool,
    /// `start, label_end, end` of each field, for the exact writing.
    spans: Vec<(usize, usize, usize)>,
    notes: &'a mut Notes,
    /// Index of the record, for the positions.
    index: usize,
}

impl<'a> TextParser<'a> {
    fn new(record: &'a mut AsciiRecord, index: usize, notes: &'a mut Notes) -> Self {
        TextParser {
            record,
            subfields: Vec::new(),
            values: Vec::new(),
            closed: false,
            spans: Vec::new(),
            notes,
            index,
        }
    }

    /// Position of the current field.
    fn position(&self, tag: Option<PyInt>) -> Position {
        Position {
            record_index: Some(self.index),
            record_type: Some(self.record.record_type()),
            tag,
            offset: self.spans.last().map(|&(start, _, _)| start),
        }
    }

    fn clear(&mut self) {
        self.subfields.clear();
        self.values.clear();
    }
}

impl Lexed for TextParser<'_> {
    fn field_span(&mut self, start: usize, label_end: usize, end: usize) {
        self.spans.push((start, label_end, end));
    }

    fn bad_field(&mut self, error: Error) -> Result<()> {
        let error = locate(
            error,
            Some(self.index),
            Some(self.record.record_type()),
            None,
        );
        self.notes
            .recover(DiagnosticCode::BadField, Severity::Error, error)
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
            let e = Error::illegal_tag_name(label).with_position(self.position(None));
            self.clear();
            return self
                .notes
                .recover(DiagnosticCode::BadField, Severity::Error, e);
        };
        let tag_number = py_int(tag);
        if py_int(record).ok().as_ref().and_then(to_u32) != Some(record_type) {
            // `int(record)` may fail on more than 4 300 digits: a record
            // number that cannot be the one of the record.
            let e = match py_int(record) {
                Err(e) if !self.notes.tolerant => Error::from(e),
                _ => Error::illegal_record_number(label),
            };
            let e = e.with_position(self.position(tag_number.as_ref().ok().cloned()));
            self.notes
                .recover(DiagnosticCode::TagRecordMismatch, Severity::Warning, e)?;
        }
        let tag = match tag_number {
            Ok(tag) => tag,
            Err(e) => {
                let e = Error::from(e).with_position(self.position(None));
                self.clear();
                return self
                    .notes
                    .recover(DiagnosticCode::BadField, Severity::Error, e);
            }
        };
        let position = self.position(Some(tag.clone()));
        if !self.record.field(&tag).is_some_and(AnyField::is_truthy) {
            let field = Field::new(record_type, tag.clone(), "")?;
            if let Err(e) = self.record.add_field(field) {
                // A first occurrence with a simple value: BAD_TAG_DUPLICATE.
                // The tolerant mode reuses it.
                let e = locate(e, position.record_index, None, position.offset);
                self.notes
                    .recover(DiagnosticCode::DuplicateTag, Severity::Warning, e)?;
            }
        }
        let field = match self.record.field_mut(FieldKey::Tag(tag.clone())) {
            Some(AnyField::Text(f)) => f,
            _ => {
                // A binary field of the same tag (not reachable from a
                // buffer): `reset` does not exist.
                let e = Error::no_attribute("BinaryField", "reset").with_position(position);
                self.clear();
                return self
                    .notes
                    .recover(DiagnosticCode::BadField, Severity::Error, e);
            }
        };
        field.reset();
        match value.filter(|v| !v.is_empty()) {
            Some(value) => {
                let text = decode_latin1(value);
                // IDC and LEN are numeric.
                if (record_type != 1 && tag == PyInt::from(2)) || tag == PyInt::from(1) {
                    match py_int(&text) {
                        Ok(n) => field.set_value(n)?,
                        Err(e) => {
                            let e = Error::from(e).with_position(position);
                            self.notes.recover(
                                DiagnosticCode::InvalidNumber,
                                Severity::Warning,
                                e,
                            )?;
                            field.set_value(text)?;
                        }
                    }
                } else {
                    field.set_value(text)?;
                }
            }
            None => field.add_subfields(std::mem::take(&mut self.subfields))?,
        }
        self.clear();
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

fn position(index: usize, record_type: u32, offset: usize) -> Position {
    Position {
        record_index: Some(index),
        record_type: Some(record_type),
        tag: None,
        offset: Some(offset),
    }
}

/// What follows a record.
enum Step {
    /// The next record starts after this length.
    Next(usize),
    /// The parsing stops.
    Stop,
}

/// State of `Message.parse`.
struct Parser {
    buffer: Bytes,
    records: Vec<Record>,
    warnings: Vec<Warning>,
    notes: Notes,
    /// Bytes the tolerant mode could not read.
    trailing: Bytes,
    /// Number of the last tagged record read (`record` in `nistitl`).
    last_tagged: Option<PyInt>,
    /// Last search of the data label: from where, for which label, and what
    /// it found. Offsets only grow, so a match after the new start stands.
    data_search: Option<(usize, Vec<u8>, Option<usize>)>,
    /// Characters of a text CNT, indexed by the binary records; dropped
    /// whenever the Type-1 is read again.
    cnt_chars: Option<Vec<char>>,
    /// For each record, the bytes read and the bytes of its fields
    /// (SPEC §13.1).
    origins: Vec<(Vec<Bytes>, Vec<RawField>)>,
}

impl Parser {
    fn run(buffer: Bytes, tolerant: bool) -> Result<Parsed> {
        let mut parser = Parser {
            buffer,
            records: Vec::new(),
            warnings: Vec::new(),
            notes: Notes {
                tolerant,
                diagnostics: Vec::new(),
            },
            trailing: Bytes::new(),
            last_tagged: None,
            data_search: None,
            cnt_chars: None,
            origins: vec![(Vec::new(), Vec::new())],
        };
        // `reset(autocreate=False)`: an empty Type-1.
        parser
            .records
            .push(Record::Ascii(AsciiRecord::with(1, false, true)?));
        if let Err(e) = parser.parse() {
            if !tolerant {
                return Err(e);
            }
            // Not expected: every error has a recovery. Keep what was read.
            parser.notes.diagnostics.push(Diagnostic::new(
                DiagnosticCode::UnknownBinaryRecord,
                Severity::Fatal,
                &e,
            ));
        }
        let mut records = parser.records;
        for (record, (segments, raw)) in records.iter_mut().zip(parser.origins) {
            match record {
                Record::Ascii(r) => r.set_origin(segments, raw),
                Record::Binary(r) => r.set_origin(segments.into_iter().next().unwrap_or_default()),
            }
        }
        let mut message = Message {
            records,
            layout: None,
            trailing: parser.trailing,
        };
        if let Err(e) = check_cnt(&message) {
            let code = if e.kind() == &ErrorKind::MissingField {
                DiagnosticCode::MissingType1
            } else {
                DiagnosticCode::CntMismatch
            };
            let severity = if code == DiagnosticCode::MissingType1 {
                Severity::Error
            } else {
                Severity::Warning
            };
            parser.notes.recover(code, severity, e)?;
        }
        message.layout = Some(message.layout_now());
        Ok(Parsed {
            message,
            diagnostics: parser.notes.diagnostics,
            warnings: parser.warnings,
        })
    }

    fn parse(&mut self) -> Result<()> {
        let len = self.buffer.len();
        let mut offset = 0usize;
        while offset + 4 < len {
            let step = match match_tag_len(self.buffer.get(offset..).unwrap_or_default()) {
                Some((record, value)) => {
                    let (record, value) = (record.to_vec(), value.to_vec());
                    self.tagged(offset, &record, &value)?
                }
                None => self.binary(offset)?,
            };
            match step {
                Step::Next(length) => offset += length,
                Step::Stop => return Ok(()),
            }
        }
        if offset < len {
            let e = Error::nist_too_long().with_position(Position {
                offset: Some(offset),
                ..Position::default()
            });
            self.notes
                .recover(DiagnosticCode::TrailingBytes, Severity::Warning, e)?;
            self.trailing = self.buffer.slice(offset..).unwrap_or_default();
        }
        Ok(())
    }

    /// Stops the parsing, the bytes from `offset` being trailing.
    fn stop(&mut self, offset: usize) -> Step {
        self.trailing = self.buffer.slice(offset..).unwrap_or_default();
        Step::Stop
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

    /// A tagged record at `offset`.
    fn tagged(&mut self, offset: usize, record_digits: &[u8], len_digits: &[u8]) -> Result<Step> {
        let len = self.buffer.len();
        let index = self.records.len();
        let at = |e: Error, t: Option<u32>| locate(e, Some(index), t, Some(offset));
        let record = match py_int(&digits_text(record_digits)) {
            Ok(record) => record,
            Err(e) => {
                let e = at(e.into(), None);
                self.notes
                    .recover(DiagnosticCode::UnknownRecordType, Severity::Fatal, e)?;
                return Ok(self.stop(offset));
            }
        };
        let length = match py_int(&digits_text(len_digits)) {
            Ok(length) => Some(length),
            Err(e) => {
                let e = at(e.into(), to_u32(&record));
                self.notes
                    .recover(DiagnosticCode::InvalidNumber, Severity::Warning, e)?;
                None
            }
        };
        self.last_tagged = Some(record.clone());
        let table = to_u32(&record).and_then(alias::table);
        if table.is_none() {
            let e = at(Error::unknown_record_type(record.clone()), None);
            let severity = match to_u32(&record) {
                Some(_) => Severity::Warning,
                None => Severity::Fatal,
            };
            self.notes
                .recover(DiagnosticCode::UnknownRecordType, severity, e)?;
        }
        let Some(record_type) = to_u32(&record) else {
            return Ok(self.stop(offset));
        };
        let at = |e: Error| locate(e, Some(index), Some(record_type), Some(offset));
        let data_tag = table.and_then(|t| alias::tag_of(t, alias::DATA));
        let data_label = data_tag.map(|t| format!("{t}:").into_bytes());

        // A LEN that is not a number counts as 0: the record extends to its
        // FS or data label.
        let length = length.unwrap_or_else(|| PyInt::from(0));
        let mut future = None;
        let length = match to_usize(&length)
            .filter(|&l| offset.checked_add(l).is_some_and(|end| end <= len))
        {
            None => {
                future = Some((DiagnosticCode::Truncated, Error::nist_too_short(&record)));
                len - offset
            }
            Some(length) => {
                let pos_data = data_label.as_ref().and_then(|l| self.find_label(l, offset));
                let pos_fs = find(&self.buffer, &[FS], offset);
                match [pos_fs, pos_data].into_iter().flatten().min() {
                    None => {
                        let e = at(Error::empty_sequence());
                        self.notes
                            .recover(DiagnosticCode::Unterminated, Severity::Error, e)?;
                        len - offset
                    }
                    Some(first) if first - offset > length => {
                        future = Some((DiagnosticCode::LenMismatch, Error::nist_too_long()));
                        // `nistitl` stops before the FS, which then reads as
                        // extra bytes; the tolerant mode keeps it.
                        if self.notes.tolerant && Some(first) == pos_fs {
                            first - offset + 1
                        } else {
                            first - offset
                        }
                    }
                    Some(_) => length,
                }
            }
        };
        let future = future.map(|(code, e)| (code, at(e)));
        let record_buffer = self
            .buffer
            .slice(offset..offset + length)
            .unwrap_or_default();
        let segment = record_buffer.clone();
        let mut pos = data_label
            .as_ref()
            .and_then(|l| find(&record_buffer, l, 0))
            .filter(|&p| p > 0);

        // Tagged record with binary data: the text part, then the data.
        let mut data_part = None;
        if let Some(p) = pos {
            let pos_end = record_buffer
                .get(..p)
                .and_then(|b| b.iter().rposition(|&c| c == GS));
            let header_start = pos_end.map_or(0, |e| e + 1);
            let header = decode_latin1(record_buffer.get(header_start..p + 4).unwrap_or_default());
            match match_tag_begin(&header) {
                Some((r, t)) => {
                    if r.len() + t.len() + 2 < header.len() {
                        self.notes.note(
                            DiagnosticCode::DataTagInText,
                            format!("data label found in a text value: {header}"),
                            position(index, record_type, offset + p),
                        );
                    }
                    data_part = Some((pos_end, header_start));
                }
                None => {
                    let tag = data_tag.map_or_else(String::new, |t| t.to_string());
                    let e = locate(
                        Error::illegal_tag_format(tag, header),
                        Some(index),
                        Some(record_type),
                        Some(offset + header_start),
                    );
                    self.notes
                        .recover(DiagnosticCode::BadDataTag, Severity::Error, e)?;
                    pos = None;
                }
            }
        }

        match (pos, data_part) {
            (Some(pos), Some((pos_end, header_start))) => {
                // `record_buffer[:pos_end] + FS`, `pos_end` being -1 if no GS.
                let text_end = pos_end.unwrap_or(record_buffer.len().saturating_sub(1));
                let mut text = record_buffer.get(..text_end).unwrap_or_default().to_vec();
                text.push(FS);
                let record = self.text_record(record_type, table.is_some()).map_err(at)?;
                self.push(record, offset)?;
                self.lex_into(index, &text, offset, future.as_ref())?;
                let start = offset + pos + 4;
                let end = (offset + record_buffer.len()).saturating_sub(1);
                let data = self.buffer.slice(start..end.max(start)).unwrap_or_default();
                if record_buffer.last() != Some(&FS) {
                    self.notes.note(
                        DiagnosticCode::MissingFinalFs,
                        "the last byte of the record is not FS, it is dropped".to_owned(),
                        position(index, record_type, end),
                    );
                }
                let next_label = format!("\u{1d}{record_type}.").into_bytes();
                if let Some(at_label) = find(&data, &next_label, 0) {
                    let follows = data.get(at_label + next_label.len()..).unwrap_or_default();
                    if super::lexer::match_tag_content(
                        &[format!("{record_type}.").as_bytes(), follows].concat(),
                    )
                    .is_some()
                    {
                        self.notes.note(
                            DiagnosticCode::DataNotLast,
                            "fields follow the data field: they are part of the data".to_owned(),
                            position(index, record_type, start + at_label),
                        );
                    }
                }
                let located = |e| locate(e, Some(index), Some(record_type), Some(offset + pos));
                if let (Some(tag), Some(Record::Ascii(r))) = (data_tag, self.records.get_mut(index))
                {
                    let field =
                        BinaryField::new(record_type, tag, data.clone()).map_err(located)?;
                    if let Err(e) = r.add_field(field.clone()) {
                        self.notes.recover(
                            DiagnosticCode::DuplicateTag,
                            Severity::Warning,
                            located(e),
                        )?;
                        r.replace_field(field.into()).map_err(located)?;
                    }
                    let label = self
                        .buffer
                        .slice(offset + header_start..offset + pos + 4)
                        .unwrap_or_default();
                    if let Some((_, raw)) = self.origins.get_mut(index) {
                        raw.push(RawField {
                            tag: PyInt::from(tag),
                            label,
                            content: data,
                        });
                    }
                }
            }
            _ if record_type == 1 => {
                self.cnt_chars = None;
                self.lex_into(0, &record_buffer, offset, future.as_ref())?;
            }
            _ => {
                let record = self.text_record(record_type, table.is_some()).map_err(at)?;
                self.push(record, offset)?;
                self.lex_into(index, &record_buffer, offset, future.as_ref())?;
            }
        }
        let target = if pos.is_some() || record_type != 1 {
            index
        } else {
            0
        };
        if let Some((segments, _)) = self.origins.get_mut(target) {
            segments.push(segment);
        }
        match future {
            None => Ok(Step::Next(length)),
            Some((DiagnosticCode::Truncated, e)) => {
                self.notes
                    .recover(DiagnosticCode::Truncated, Severity::Error, e)?;
                Ok(Step::Next(length))
            }
            Some((code, e)) => {
                self.notes.recover(code, Severity::Warning, e)?;
                Ok(Step::Next(length))
            }
        }
    }

    /// A new tagged record: `_factory(t, False, True)`, or a record without
    /// alias table for a type absent from it (tolerant mode).
    fn text_record(&mut self, record_type: u32, known: bool) -> Result<Record> {
        if known {
            self.factory(&PyInt::from(record_type), false)
        } else {
            Ok(Record::Ascii(AsciiRecord::untabled(record_type)))
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
        self.origins.push((Vec::new(), Vec::new()));
        Ok(())
    }

    /// Lexes `text` into the tagged record at `index`, with the rule of the
    /// `future` error (SPEC §10.9).
    fn lex_into(
        &mut self,
        index: usize,
        text: &[u8],
        offset: usize,
        future: Option<&(DiagnosticCode, Error)>,
    ) -> Result<()> {
        let Some(Record::Ascii(record)) = self.records.get_mut(index) else {
            return Err(Error::index_out_of_range());
        };
        let record_type = record.record_type();
        let mut parser = TextParser::new(record, index, &mut self.notes);
        let lexed = lex(text, offset, &mut parser);
        let (closed, spans) = (parser.closed, parser.spans);
        if let Err(e) = lexed {
            let e = locate(e, Some(index), Some(record_type), None);
            return Err(match (future, e.kind()) {
                (Some((_, future)), ErrorKind::Nist(_)) => future.clone().with_source(e),
                _ => e,
            });
        }
        if !closed && !self.notes.tolerant {
            return Err(future.map_or_else(
                || {
                    locate(
                        Error::record_not_terminated(record_type),
                        Some(index),
                        Some(record_type),
                        Some(offset),
                    )
                },
                |(_, future)| future.clone(),
            ));
        }
        let buffer = &self.buffer;
        let raw = spans.into_iter().filter_map(|(start, label_end, end)| {
            let label = buffer.slice(start..label_end)?;
            let text = decode_latin1(&label);
            let (_, tag) = match_tag_begin(&text)?;
            Some(RawField {
                tag: py_int(tag).ok()?,
                label,
                content: buffer.slice(label_end..end)?,
            })
        });
        if let Some((_, fields)) = self.origins.get_mut(index) {
            fields.extend(raw);
        }
        Ok(())
    }

    /// A binary record at `offset`.
    fn binary(&mut self, offset: usize) -> Result<Step> {
        let len = self.buffer.len();
        let index = self.records.len();
        let at = |e: Error| locate(e, Some(index), None, Some(offset));
        // `struct.unpack("!LB", ...)`: the loop leaves at least 5 bytes.
        let head = self.buffer.get(offset..offset + 5).unwrap_or_default();
        let (Some(Ok(length)), Some(&idc)) = (head.get(..4).map(<[u8; 4]>::try_from), head.get(4))
        else {
            return Err(at(Error::unrecognized_binary_record()));
        };
        let length = u32::from_be_bytes(length);
        let mut length = usize::try_from(length).unwrap_or(usize::MAX);
        let record_type = match self.cnt_record_type(index) {
            Ok(t) => t,
            Err(e) => {
                self.notes
                    .recover(DiagnosticCode::UnknownBinaryRecord, Severity::Fatal, at(e))?;
                return Ok(self.stop(offset));
            }
        };
        if self.notes.tolerant && length < 5 {
            self.notes.recover(
                DiagnosticCode::UnknownBinaryRecord,
                Severity::Fatal,
                at(Error::unrecognized_binary_record()),
            )?;
            return Ok(self.stop(offset));
        }
        let mut truncated = false;
        if offset.checked_add(length).is_none_or(|end| end > len) {
            let record = self
                .last_tagged
                .as_ref()
                .map_or_else(String::new, PyInt::to_string);
            self.notes.recover(
                DiagnosticCode::Truncated,
                Severity::Error,
                at(Error::nist_too_short(record)),
            )?;
            length = len - offset;
            truncated = true;
        }
        let at = |e: Error| locate(e, Some(index), to_u32(&record_type), Some(offset));
        let value = self
            .buffer
            .slice(offset + 5..offset + length)
            .unwrap_or_default();
        let record = if self.notes.tolerant {
            match to_u32(&record_type) {
                Some(3..=8) => self.factory(&record_type, true).map_err(at)?,
                Some(t) if t != 1 => {
                    let e = at(Error::bad_attribute_set("value", t));
                    self.notes.recover(
                        DiagnosticCode::UnexpectedBinaryRecord,
                        Severity::Error,
                        e,
                    )?;
                    Record::Binary(BinaryRecord::new(t))
                }
                _ => {
                    let e = at(Error::unrecognized_binary_record());
                    self.notes
                        .recover(DiagnosticCode::UnknownBinaryRecord, Severity::Fatal, e)?;
                    return Ok(self.stop(offset));
                }
            }
        } else {
            self.factory(&record_type, true).map_err(at)?
        };
        let mut record = record;
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
        let segment = self
            .buffer
            .slice(offset..offset + length)
            .unwrap_or_default();
        if let Some((segments, _)) = self.origins.last_mut() {
            segments.push(segment);
        }
        if truncated {
            return Ok(Step::Stop);
        }
        Ok(Step::Next(length))
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
    use crate::value::Value;
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
