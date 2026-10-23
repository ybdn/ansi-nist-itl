//! Errors (SPEC §4.2–4.4).
//!
//! [`Error`] mirrors the exceptions of `nistitl`: a [`NistError`] code for
//! `NistException`, and a dedicated [`ErrorKind`] for each Python exception
//! that `nistitl` lets through (SPEC §12). [`Display`](fmt::Display) gives the
//! exact text of the Python exception; the [`Position`] is an addition that
//! never appears in it.

use std::fmt;

use crate::py::{BadFormat, EncodeError, IntError, repr_str};
use crate::value::PyInt;

/// `Result` with the crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error codes of `nistitl` (`NistError`), with the same numeric values.
///
/// ```
/// use ansi_nist_itl::NistError;
///
/// assert_eq!(NistError::BadRecord as u8, 1);
/// assert_eq!(NistError::BadSubfieldValue.name(), "BAD_SUBFIELD_VALUE");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum NistError {
    /// `BAD_RECORD`: a field of a tagged record cannot be split.
    BadRecord = 1,
    /// `BAD_TAG_NAME`: malformed tag name (unreachable in practice).
    BadTagName = 2,
    /// `BAD_TAG_FORMAT`: malformed tag of the binary data field (999).
    BadTagFormat = 3,
    /// `BAD_RECORD_NUMBER`: the record number of a tag differs from the record type.
    BadRecordNumber = 4,
    /// `BAD_CONTENT`: unrecognised binary record, or CNT (1.003) inconsistent with the records.
    BadContent = 5,
    /// `CANNOT_ADD_TYPE1`: a Type-1 record can only be the first record.
    CannotAddType1 = 6,
    /// `CANNOT_DELETE_TYPE1`: the Type-1 record cannot be removed.
    CannotDeleteType1 = 7,
    /// `RECORD_NOT_FOUND`: the record to remove is not in the message.
    RecordNotFound = 8,
    /// `RECORD_NOT_TERMINATED`: tagged record without final FS (unreachable in practice).
    RecordNotTerminated = 9,
    /// `NIST_TOO_SHORT`: the buffer ends before the length announced by a record.
    NistTooShort = 10,
    /// `NIST_TOO_LONG`: bytes remain after the records, or a record is shorter than its LEN.
    NistTooLong = 11,
    /// `BAD_TAG_DUPLICATE`: the tag already exists in the record.
    BadTagDuplicate = 12,
    /// `BAD_ALIAS_DUPLICATE`: the alias already exists in the record.
    BadAliasDuplicate = 13,
    /// `UNKNOWN_ATTRIBUTE`: unknown alias or attribute name.
    UnknownAttribute = 14,
    /// `BAD_FIELD_VALUE`: value incompatible with the kind (F/S/I) of the field.
    BadFieldValue = 15,
    /// `BAD_SUBFIELD_VALUE`: value incompatible with the kind (S/I) of the subfield.
    BadSubfieldValue = 16,
}

impl NistError {
    /// Python name of the code, as printed by `NistException`.
    ///
    /// ```
    /// use ansi_nist_itl::NistError;
    ///
    /// assert_eq!(NistError::CannotDeleteType1.name(), "CANNOT_DELETE_TYPE1");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            NistError::BadRecord => "BAD_RECORD",
            NistError::BadTagName => "BAD_TAG_NAME",
            NistError::BadTagFormat => "BAD_TAG_FORMAT",
            NistError::BadRecordNumber => "BAD_RECORD_NUMBER",
            NistError::BadContent => "BAD_CONTENT",
            NistError::CannotAddType1 => "CANNOT_ADD_TYPE1",
            NistError::CannotDeleteType1 => "CANNOT_DELETE_TYPE1",
            NistError::RecordNotFound => "RECORD_NOT_FOUND",
            NistError::RecordNotTerminated => "RECORD_NOT_TERMINATED",
            NistError::NistTooShort => "NIST_TOO_SHORT",
            NistError::NistTooLong => "NIST_TOO_LONG",
            NistError::BadTagDuplicate => "BAD_TAG_DUPLICATE",
            NistError::BadAliasDuplicate => "BAD_ALIAS_DUPLICATE",
            NistError::UnknownAttribute => "UNKNOWN_ATTRIBUTE",
            NistError::BadFieldValue => "BAD_FIELD_VALUE",
            NistError::BadSubfieldValue => "BAD_SUBFIELD_VALUE",
        }
    }
}

impl fmt::Display for NistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Category of an [`Error`].
///
/// [`ErrorKind::Nist`] stands for `NistException`. The other variants stand
/// for the Python exceptions that `nistitl` does not catch (SPEC §12): the
/// crate turns them into typed errors and keeps the Python message.
///
/// ```
/// use ansi_nist_itl::{ErrorKind, NistError};
///
/// let kind = ErrorKind::Nist(NistError::BadRecord);
/// assert_ne!(kind, ErrorKind::MissingField);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// `NistException` with its code.
    Nist(NistError),
    /// `ValueError` raised by `int()`: `invalid literal for int() with base 10: 'A'`.
    InvalidLiteral,
    /// `ValueError` raised by `int()` beyond 4 300 digits: `Exceeds the limit (4300 digits)…`.
    DigitsLimit,
    /// `KeyError` on the alias table: the record type is unknown (`50`).
    UnknownRecordType(PyInt),
    /// `TypeError` / `AttributeError` on a missing field (CNT, LEN…).
    MissingField,
    /// `IndexError`.
    IndexOutOfRange,
    /// `struct.error`: `'B' format requires 0 <= number <= 255`.
    Struct,
    /// `UnicodeEncodeError`: a character outside Latin-1 cannot be written.
    Encoding,
    /// `TypeError` / `AttributeError` on an unexpected value type.
    TypeMismatch,
    /// `ValueError` of `min()` on an empty list: `min() iterable argument is empty`.
    EmptySequence,
    /// Unsupported tag format template (only `%[0][n]d`, SPEC §3.5); no Python equivalent.
    BadFormat,
    /// I/O error of the file and reader helpers; no Python equivalent.
    Io(std::io::ErrorKind),
    /// Text that is not valid in the character encoding it is decoded
    /// with (`encoding` module); no Python equivalent.
    Decoding,
    /// A character encoding the crate does not decode: its index in field
    /// 1.015, or `None` for an `STX` switch sequence or an index out of
    /// range (`encoding` module); no Python equivalent.
    UnsupportedEncoding(Option<u32>),
}

/// Where an error occurred (LIB-11). Never part of [`Display`](fmt::Display).
///
/// ```
/// use ansi_nist_itl::Position;
///
/// let p = Position::default();
/// assert_eq!(p.offset, None);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Position {
    /// Index of the record in the message (Type-1 = 0).
    pub record_index: Option<usize>,
    /// Type of the record.
    pub record_type: Option<u32>,
    /// Number of the field.
    pub tag: Option<PyInt>,
    /// Absolute offset, in bytes, in the input buffer.
    pub offset: Option<usize>,
}

/// Error of the crate: the equivalent of the exception raised by `nistitl`.
///
/// `Display` gives the text of the Python exception, character for
/// character: `"{NAME}: {message}"` for a `NistException`, the bare message
/// otherwise.
///
/// The content is boxed, so that `Result<T, Error>` stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(Box<Inner>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Inner {
    kind: ErrorKind,
    message: String,
    position: Position,
    source: Option<Error>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Error(Box::new(Inner {
            kind,
            message: message.into(),
            position: Position::default(),
            source: None,
        }))
    }

    fn nist(code: NistError, message: impl Into<String>) -> Self {
        Error::new(ErrorKind::Nist(code), message)
    }

    /// Category of the error.
    pub fn kind(&self) -> &ErrorKind {
        &self.0.kind
    }

    /// Message of the exception, without the `NistError` name.
    pub fn message(&self) -> &str {
        &self.0.message
    }

    /// Where the error occurred, as far as known.
    pub fn position(&self) -> &Position {
        &self.0.position
    }

    /// The `nistitl` code, for a `NistException`.
    pub fn nist_error(&self) -> Option<NistError> {
        match self.0.kind {
            ErrorKind::Nist(code) => Some(code),
            _ => None,
        }
    }

    pub(crate) fn with_position(mut self, position: Position) -> Self {
        self.0.position = position;
        self
    }

    pub(crate) fn position_mut(&mut self) -> &mut Position {
        &mut self.0.position
    }

    /// `raise self from source`.
    pub(crate) fn with_source(mut self, source: Error) -> Self {
        self.0.source = Some(source);
        self
    }

    // Messages of SPEC §4.4, typos included. `%s` and `%d` become `Display`,
    // `%r` becomes `repr`.

    /// BAD_RECORD (§10.7).
    pub(crate) fn bad_record() -> Self {
        Error::nist(NistError::BadRecord, "Invalid record data")
    }

    /// BAD_TAG_NAME (§10.8).
    pub(crate) fn illegal_tag_name(tag: impl fmt::Display) -> Self {
        Error::nist(NistError::BadTagName, format!("Illegal tag name {tag}"))
    }

    /// BAD_TAG_FORMAT (§10.9).
    pub(crate) fn illegal_tag_format(tag: impl fmt::Display, found: impl fmt::Display) -> Self {
        Error::nist(
            NistError::BadTagFormat,
            format!("Illegal format for tag {tag} ({found})"),
        )
    }

    /// BAD_RECORD_NUMBER raised by the parser (§10.8).
    pub(crate) fn illegal_record_number(tag: impl fmt::Display) -> Self {
        Error::nist(
            NistError::BadRecordNumber,
            format!("Illegal record number in tag name {tag}"),
        )
    }

    /// BAD_RECORD_NUMBER raised when adding a field (§8.2).
    pub(crate) fn bad_record_number(
        record: impl fmt::Display,
        tag: impl fmt::Display,
        record_type: impl fmt::Display,
    ) -> Self {
        Error::nist(
            NistError::BadRecordNumber,
            format!("Bad record number {record} for tag {tag} in record {record_type}"),
        )
    }

    /// BAD_CONTENT on a binary record (§10.9).
    pub(crate) fn unrecognized_binary_record() -> Self {
        Error::nist(
            NistError::BadContent,
            "Could not recognize binary record, bad content or bad record",
        )
    }

    /// BAD_CONTENT: CNT and records differ in number (§10.10).
    pub(crate) fn cnt_record_count() -> Self {
        Error::nist(
            NistError::BadContent,
            "Bad CNT tag in record 1 (different number of records)",
        )
    }

    /// BAD_CONTENT: a CNT subfield has not two items (§10.10).
    pub(crate) fn cnt_value_count(subfield: usize) -> Self {
        Error::nist(
            NistError::BadContent,
            format!("Bad CNT tag in record 1 (bad number of values for subfield #{subfield})"),
        )
    }

    /// BAD_CONTENT: the first CNT subfield does not announce a Type-1 (§10.10).
    pub(crate) fn cnt_record_type(subfield: usize) -> Self {
        Error::nist(
            NistError::BadContent,
            format!("Bad CNT tag in record 1 (bad record type for subfield #{subfield})"),
        )
    }

    /// BAD_CONTENT: a CNT subfield matches neither type nor IDC (§10.10).
    pub(crate) fn cnt_record_type_or_idc(subfield: usize) -> Self {
        Error::nist(
            NistError::BadContent,
            format!(
                "Bad CNT tag in record 1 (bad record type or bad IDC for subfield #{subfield})"
            ),
        )
    }

    /// CANNOT_ADD_TYPE1 (§10.2).
    pub(crate) fn cannot_add_type1() -> Self {
        Error::nist(
            NistError::CannotAddType1,
            "Cannot add a type 1 record: it must be the first record",
        )
    }

    /// CANNOT_DELETE_TYPE1 (§10.4).
    pub(crate) fn cannot_delete_type1() -> Self {
        Error::nist(
            NistError::CannotDeleteType1,
            "Cannot delete the type 1 record",
        )
    }

    /// RECORD_NOT_FOUND (§10.4).
    pub(crate) fn record_not_found() -> Self {
        Error::nist(
            NistError::RecordNotFound,
            "Cannot remove a record: record not found",
        )
    }

    /// RECORD_NOT_TERMINATED (§10.9).
    pub(crate) fn record_not_terminated(record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::RecordNotTerminated,
            format!("Record type {record_type} not terminated"),
        )
    }

    /// NIST_TOO_SHORT (§10.9).
    pub(crate) fn nist_too_short(record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::NistTooShort,
            format!("NIST buffer too short (missing bytes) when parsing record {record_type}"),
        )
    }

    /// NIST_TOO_LONG (§10.9).
    pub(crate) fn nist_too_long() -> Self {
        Error::nist(NistError::NistTooLong, "NIST buffer too long (extra bytes)")
    }

    /// BAD_TAG_DUPLICATE (§8.2).
    pub(crate) fn tag_duplicate(tag: impl fmt::Display, record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::BadTagDuplicate,
            format!("Tag {tag} already defined in record {record_type}"),
        )
    }

    /// BAD_ALIAS_DUPLICATE (§8.2).
    pub(crate) fn alias_duplicate(alias: &str, record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::BadAliasDuplicate,
            format!(
                "Alias {} already defined in record {record_type}",
                repr_str(alias)
            ),
        )
    }

    /// UNKNOWN_ATTRIBUTE raised by `set` (§8.5).
    pub(crate) fn bad_attribute_set(name: &str, record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::UnknownAttribute,
            format!(
                "Bad attribute name <{name}> while trying to define a field in record of type {record_type}"
            ),
        )
    }

    /// UNKNOWN_ATTRIBUTE raised by `get` (§8.6).
    pub(crate) fn bad_attribute_get(name: &str, record_type: impl fmt::Display) -> Self {
        Error::nist(
            NistError::UnknownAttribute,
            format!(
                "Unkown or bad attribute <{name}> while trying to retrieve a field in record of type {record_type}"
            ),
        )
    }

    /// BAD_FIELD_VALUE: scalar value on a field without `F` (§6.3).
    pub(crate) fn field_cannot_have_value(label: &str) -> Self {
        Error::nist(
            NistError::BadFieldValue,
            format!("Field {label} cannot have a value (only subfields and/or items)"),
        )
    }

    /// BAD_FIELD_VALUE: list value on a field without `S` nor `I` (§6.3, §6.5).
    pub(crate) fn field_cannot_have_subfields(label: &str) -> Self {
        Error::nist(
            NistError::BadFieldValue,
            format!("Field {label} cannot have subfields"),
        )
    }

    /// BAD_SUBFIELD_VALUE: scalar value on a subfield without `S` (§5.3).
    pub(crate) fn subfield_cannot_have_value() -> Self {
        Error::nist(
            NistError::BadSubfieldValue,
            "Subfield cannot have a value, only items",
        )
    }

    /// BAD_SUBFIELD_VALUE: items on a subfield without `I` (§5.5).
    pub(crate) fn subfield_cannot_have_items() -> Self {
        Error::nist(NistError::BadSubfieldValue, "Subfield cannot have items")
    }

    /// BAD_SUBFIELD_VALUE: subfield with items added to a field without `I` (§6.5).
    pub(crate) fn subfield_of_cannot_have_items(label: &str) -> Self {
        Error::nist(
            NistError::BadSubfieldValue,
            format!("Subfield of {label} cannot have items"),
        )
    }

    /// BAD_SUBFIELD_VALUE: subfield with a value added to a field without `S` (§6.5).
    ///
    /// `value_repr` is the `repr` of the subfield value.
    pub(crate) fn subfield_of_cannot_have_value(label: &str, value_repr: &str) -> Self {
        Error::nist(
            NistError::BadSubfieldValue,
            format!("Subfield of {label} cannot have a value: {value_repr}"),
        )
    }

    /// `KeyError` on the alias table: `str(KeyError(50))` is `50`.
    pub(crate) fn unknown_record_type(record_type: PyInt) -> Self {
        let message = record_type.to_string();
        Error::new(ErrorKind::UnknownRecordType(record_type), message)
    }

    /// `ValueError` of `min()` on an empty sequence (§10.9).
    pub(crate) fn empty_sequence() -> Self {
        Error::new(ErrorKind::EmptySequence, "min() iterable argument is empty")
    }

    /// `IndexError` of a list.
    pub(crate) fn index_out_of_range() -> Self {
        Error::new(ErrorKind::IndexOutOfRange, "list index out of range")
    }

    /// `TypeError` of `int()` on a value of Python type `type_name` (`NoneType`).
    pub(crate) fn int_argument(type_name: &str) -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            format!(
                "int() argument must be a string, a bytes-like object or a real number, not {}",
                repr_str(type_name)
            ),
        )
    }

    /// `TypeError` of `len(x)` on an object without length.
    pub(crate) fn no_len(class: &str) -> Self {
        let kind = if class == "NoneType" {
            ErrorKind::MissingField
        } else {
            ErrorKind::TypeMismatch
        };
        Error::new(kind, format!("object of type '{class}' has no len()"))
    }

    /// A typed header asked of a binary record of another type (SPEC §13.3).
    pub(crate) fn header_type(header: &str, record_type: u32, types: &str) -> Self {
        Error::nist(
            NistError::BadContent,
            format!("No {header} header in a record of type {record_type} ({types} only)"),
        )
    }

    /// A binary record too short for its typed header (SPEC §13.3).
    pub(crate) fn header_too_short(record_type: u32, len: usize, needed: usize) -> Self {
        Error::nist(
            NistError::BadContent,
            format!(
                "Record of type {record_type} too short for its header: {len} bytes, {needed} needed"
            ),
        )
    }

    /// `IndexError` of `del list[i]`.
    pub(crate) fn assignment_index_out_of_range() -> Self {
        Error::new(
            ErrorKind::IndexOutOfRange,
            "list assignment index out of range",
        )
    }

    /// `TypeError` of `del list[(t, idc)]`.
    pub(crate) fn tuple_index() -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            "list indices must be integers or slices, not tuple",
        )
    }

    /// `AttributeError` on a missing field (`None`), such as an absent CNT.
    pub(crate) fn none_attribute(attribute: &str) -> Self {
        Error::new(
            ErrorKind::MissingField,
            format!("'NoneType' object has no attribute '{attribute}'"),
        )
    }

    /// `AttributeError` on an object of the wrong class.
    pub(crate) fn no_attribute(class: &str, attribute: &str) -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            format!("'{class}' object has no attribute '{attribute}'"),
        )
    }

    /// `TypeError` of `record[key]` on a binary record.
    pub(crate) fn not_subscriptable(class: &str) -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            format!("'{class}' object is not subscriptable"),
        )
    }

    /// `TypeError` of `bytes + int`: a binary field given the length of its
    /// record (SPEC §8.8).
    pub(crate) fn concat_int_to_bytes() -> Self {
        Error::new(ErrorKind::TypeMismatch, "can't concat int to bytes")
    }

    /// Text value given to a binary field (SPEC §11): no Python counterpart.
    pub(crate) fn binary_field_value(tag: impl fmt::Display) -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            format!("field {tag} is binary: its value must be bytes"),
        )
    }

    /// Bytes given to, or read from, a text field (SPEC §11): no Python
    /// counterpart.
    pub(crate) fn text_field_value(tag: impl fmt::Display) -> Self {
        Error::new(
            ErrorKind::TypeMismatch,
            format!("field {tag} is text: its value cannot be bytes"),
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.kind {
            ErrorKind::Nist(code) => write!(f, "{}: {}", code.name(), self.0.message),
            _ => f.write_str(&self.0.message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0
            .source
            .as_ref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// Warning raised while parsing, where `nistitl` calls `warnings.warn`.
///
/// `Display` is the text of the Python warning.
///
/// ```
/// use ansi_nist_itl::Warning;
///
/// let w = Warning::Deprecated { record_type: 3 };
/// assert_eq!(w.to_string(), "Record of type 3");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Warning {
    /// `DeprecationWarning`: a record of type 3, 5 or 6, deprecated by the
    /// standard.
    Deprecated {
        /// Type of the record.
        record_type: u32,
    },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Warning::Deprecated { record_type } => write!(f, "Record of type {record_type}"),
        }
    }
}

/// Kind of a [`Diagnostic`] of the tolerant mode (SPEC §13.2).
///
/// ```
/// use ansi_nist_itl::{DiagnosticCode, Message, ParseOptions};
///
/// let parsed = Message::parse_with(b"", ParseOptions::tolerant())?;
/// assert_eq!(parsed.diagnostics[0].code, DiagnosticCode::MissingType1);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// The buffer ends before the LEN of a record: the record is kept as
    /// read, then the parsing stops.
    Truncated,
    /// The LEN of a record ends before its FS or its data label: the record
    /// is extended to them.
    LenMismatch,
    /// A field without a valid label: it is skipped.
    BadField,
    /// A field labelled with another record type: it is kept in the record.
    TagRecordMismatch,
    /// A tag defined twice: the second occurrence replaces the first.
    DuplicateTag,
    /// The data label is not preceded by a valid label: the record is read
    /// as text.
    BadDataTag,
    /// A LEN or IDC that is not a number: it is kept as text.
    InvalidNumber,
    /// A record type absent from the alias table: the record is read
    /// without aliases nor data field.
    UnknownRecordType,
    /// A binary record that the CNT does not describe, or shorter than its
    /// header: the parsing stops, the rest goes to
    /// [`Message::trailing`](crate::Message::trailing).
    UnknownBinaryRecord,
    /// A binary record whose CNT type is a tagged type: it is kept as a
    /// binary record.
    UnexpectedBinaryRecord,
    /// A tagged record without FS nor data label: it extends to the end of
    /// the buffer.
    Unterminated,
    /// Bytes after the last record, kept in
    /// [`Message::trailing`](crate::Message::trailing).
    TrailingBytes,
    /// The CNT does not describe the records read; they prevail.
    CntMismatch,
    /// No CNT, or no Type-1 first: an empty Type-1 heads the records read.
    MissingType1,
    /// The data label was found in a text value (SPEC E3).
    DataTagInText,
    /// The last byte of a record with data is not FS; it is dropped (E4).
    MissingFinalFs,
    /// Fields follow the data field; they are part of the data (E5).
    DataNotLast,
}

/// Severity of a [`Diagnostic`].
///
/// ```
/// use ansi_nist_itl::Severity;
///
/// assert!(Severity::Warning < Severity::Error);
/// assert!(Severity::Error < Severity::Fatal);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The data are read in full, but not as the standard says.
    Warning,
    /// Some data are lost or guessed.
    Error,
    /// The parsing stopped before the end of the buffer.
    Fatal,
}

/// A problem met by the tolerant mode, and how it was overcome
/// (SPEC §13.2).
///
/// `message` is the text of the error that `nistitl` would have raised;
/// `Display` gives the severity, the code and this text.
///
/// ```
/// use ansi_nist_itl::{Message, ParseOptions, Severity};
///
/// let mut bytes = Message::new().to_bytes()?;
/// bytes.extend_from_slice(b"xyz");
/// let parsed = Message::parse_with(&bytes, ParseOptions::tolerant())?;
/// let d = &parsed.diagnostics[0];
/// assert_eq!(d.severity, Severity::Warning);
/// assert_eq!(d.message, "NIST_TOO_LONG: NIST buffer too long (extra bytes)");
/// assert_eq!(parsed.message.trailing(), b"xyz");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Diagnostic {
    /// What happened.
    pub code: DiagnosticCode,
    /// How serious it is.
    pub severity: Severity,
    /// Where it happened.
    pub position: Position,
    /// Text of the error that `nistitl` would have raised.
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn new(code: DiagnosticCode, severity: Severity, error: &Error) -> Self {
        Diagnostic {
            code,
            severity,
            position: error.position().clone(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} ({:?}): {}", self.severity, self.code, self.message)
    }
}

impl From<IntError> for Error {
    fn from(e: IntError) -> Self {
        let kind = match e {
            IntError::InvalidLiteral(_) => ErrorKind::InvalidLiteral,
            IntError::DigitsLimit(_) => ErrorKind::DigitsLimit,
        };
        Error::new(kind, e.to_string())
    }
}

impl From<EncodeError> for Error {
    fn from(e: EncodeError) -> Self {
        Error::new(ErrorKind::Encoding, e.to_string())
    }
}

impl From<BadFormat> for Error {
    fn from(e: BadFormat) -> Self {
        Error::new(ErrorKind::BadFormat, e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::new(ErrorKind::Io(e.kind()), e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::py::{PrintfTemplate, encode_latin1, py_int};
    use std::error::Error as _;

    #[test]
    fn codes() {
        let all = [
            (NistError::BadRecord, "BAD_RECORD"),
            (NistError::BadTagName, "BAD_TAG_NAME"),
            (NistError::BadTagFormat, "BAD_TAG_FORMAT"),
            (NistError::BadRecordNumber, "BAD_RECORD_NUMBER"),
            (NistError::BadContent, "BAD_CONTENT"),
            (NistError::CannotAddType1, "CANNOT_ADD_TYPE1"),
            (NistError::CannotDeleteType1, "CANNOT_DELETE_TYPE1"),
            (NistError::RecordNotFound, "RECORD_NOT_FOUND"),
            (NistError::RecordNotTerminated, "RECORD_NOT_TERMINATED"),
            (NistError::NistTooShort, "NIST_TOO_SHORT"),
            (NistError::NistTooLong, "NIST_TOO_LONG"),
            (NistError::BadTagDuplicate, "BAD_TAG_DUPLICATE"),
            (NistError::BadAliasDuplicate, "BAD_ALIAS_DUPLICATE"),
            (NistError::UnknownAttribute, "UNKNOWN_ATTRIBUTE"),
            (NistError::BadFieldValue, "BAD_FIELD_VALUE"),
            (NistError::BadSubfieldValue, "BAD_SUBFIELD_VALUE"),
        ];
        for (i, (code, name)) in all.into_iter().enumerate() {
            assert_eq!(usize::from(code as u8), i + 1);
            assert_eq!(code.name(), name);
            assert_eq!(code.to_string(), name);
        }
    }

    /// Every message of SPEC §4.4, compared with the text printed by `nistitl`.
    #[test]
    fn messages() {
        let cases: Vec<(Error, &str)> = vec![
            (Error::bad_record(), "BAD_RECORD: Invalid record data"),
            (
                Error::illegal_tag_name("2.x:"),
                "BAD_TAG_NAME: Illegal tag name 2.x:",
            ),
            (
                Error::illegal_tag_format(999, "X999:"),
                "BAD_TAG_FORMAT: Illegal format for tag 999 (X999:)",
            ),
            (
                Error::illegal_record_number("3.004:"),
                "BAD_RECORD_NUMBER: Illegal record number in tag name 3.004:",
            ),
            (
                Error::bad_record_number(3, 4, 2),
                "BAD_RECORD_NUMBER: Bad record number 3 for tag 4 in record 2",
            ),
            (
                Error::unrecognized_binary_record(),
                "BAD_CONTENT: Could not recognize binary record, bad content or bad record",
            ),
            (
                Error::cnt_record_count(),
                "BAD_CONTENT: Bad CNT tag in record 1 (different number of records)",
            ),
            (
                Error::cnt_value_count(1),
                "BAD_CONTENT: Bad CNT tag in record 1 (bad number of values for subfield #1)",
            ),
            (
                Error::cnt_record_type(0),
                "BAD_CONTENT: Bad CNT tag in record 1 (bad record type for subfield #0)",
            ),
            (
                Error::cnt_record_type_or_idc(1),
                "BAD_CONTENT: Bad CNT tag in record 1 (bad record type or bad IDC for subfield #1)",
            ),
            (
                Error::cannot_add_type1(),
                "CANNOT_ADD_TYPE1: Cannot add a type 1 record: it must be the first record",
            ),
            (
                Error::cannot_delete_type1(),
                "CANNOT_DELETE_TYPE1: Cannot delete the type 1 record",
            ),
            (
                Error::record_not_found(),
                "RECORD_NOT_FOUND: Cannot remove a record: record not found",
            ),
            (
                Error::record_not_terminated(2),
                "RECORD_NOT_TERMINATED: Record type 2 not terminated",
            ),
            (
                Error::nist_too_short(14),
                "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 14",
            ),
            (
                Error::nist_too_long(),
                "NIST_TOO_LONG: NIST buffer too long (extra bytes)",
            ),
            (
                Error::tag_duplicate(3, 2),
                "BAD_TAG_DUPLICATE: Tag 3 already defined in record 2",
            ),
            (
                Error::alias_duplicate("SRC", 10),
                "BAD_ALIAS_DUPLICATE: Alias 'SRC' already defined in record 10",
            ),
            (
                Error::alias_duplicate("it's", 10),
                "BAD_ALIAS_DUPLICATE: Alias \"it's\" already defined in record 10",
            ),
            (
                Error::bad_attribute_set("UUU", 10),
                "UNKNOWN_ATTRIBUTE: Bad attribute name <UUU> while trying to define a field in record of type 10",
            ),
            (
                Error::bad_attribute_get("IDC", 2),
                "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <IDC> while trying to retrieve a field in record of type 2",
            ),
            (
                Error::field_cannot_have_value("2.003:"),
                "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)",
            ),
            (
                Error::field_cannot_have_subfields("2.003:"),
                "BAD_FIELD_VALUE: Field 2.003: cannot have subfields",
            ),
            (
                Error::subfield_cannot_have_value(),
                "BAD_SUBFIELD_VALUE: Subfield cannot have a value, only items",
            ),
            (
                Error::subfield_cannot_have_items(),
                "BAD_SUBFIELD_VALUE: Subfield cannot have items",
            ),
            (
                Error::subfield_of_cannot_have_items("2.003:"),
                "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have items",
            ),
            (
                Error::subfield_of_cannot_have_value("2.003:", "'a'"),
                "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have a value: 'a'",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
            assert!(error.nist_error().is_some());
        }
    }

    #[test]
    fn python_exceptions() {
        let e = Error::unknown_record_type(PyInt::from(50u64));
        assert_eq!(e.to_string(), "50");
        assert_eq!(e.kind(), &ErrorKind::UnknownRecordType(PyInt::from(50u64)));
        assert_eq!(e.nist_error(), None);

        let e = Error::empty_sequence();
        assert_eq!(e.to_string(), "min() iterable argument is empty");

        let e = Error::from(py_int("A").unwrap_err());
        assert_eq!(e.kind(), &ErrorKind::InvalidLiteral);
        assert_eq!(e.to_string(), "invalid literal for int() with base 10: 'A'");

        let e = Error::from(py_int(&"0".repeat(4301)).unwrap_err());
        assert_eq!(e.kind(), &ErrorKind::DigitsLimit);
        assert!(e.to_string().starts_with("Exceeds the limit (4300 digits)"));

        let e = Error::index_out_of_range();
        assert_eq!(e.kind(), &ErrorKind::IndexOutOfRange);
        assert_eq!(e.to_string(), "list index out of range");

        let e = Error::int_argument("NoneType");
        assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
        assert_eq!(
            e.to_string(),
            "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
        );

        let e = Error::from(encode_latin1("\u{20ac}").unwrap_err());
        assert_eq!(e.kind(), &ErrorKind::Encoding);
        assert_eq!(
            e.to_string(),
            "'latin-1' codec can't encode character '\\u20ac' in position 0: ordinal not in range(256)"
        );

        let e = Error::from(PrintfTemplate::parse("%s").unwrap_err());
        assert_eq!(e.kind(), &ErrorKind::BadFormat);

        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e = Error::from(io);
        assert_eq!(e.kind(), &ErrorKind::Io(std::io::ErrorKind::NotFound));
        assert_eq!(e.to_string(), "missing");
    }

    #[test]
    fn position_is_not_displayed() {
        let position = Position {
            record_index: Some(1),
            record_type: Some(2),
            tag: Some(PyInt::from(3u64)),
            offset: Some(42),
        };
        let mut e = Error::tag_duplicate(3, 2).with_position(position.clone());
        assert_eq!(e.position(), &position);
        assert_eq!(
            e.to_string(),
            "BAD_TAG_DUPLICATE: Tag 3 already defined in record 2"
        );
        e.position_mut().offset = None;
        assert_eq!(e.position().offset, None);
    }

    #[test]
    fn source_chain() {
        // `raise future_exc from exc` (§10.9).
        let e = Error::nist_too_short(2).with_source(Error::bad_record());
        assert_eq!(
            e.to_string(),
            "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 2"
        );
        let source = e.source().map(ToString::to_string);
        assert_eq!(source.as_deref(), Some("BAD_RECORD: Invalid record data"));
        assert!(Error::bad_record().source().is_none());
    }
}
