//! Character encodings declared in field 1.015, DCS (LIB-18).
//!
//! As `nistitl`, the crate reads and writes text as Latin-1: each byte is
//! one character. A file whose 1.015 declares UTF-8 therefore reads `"é"`
//! as `"Ã©"`. This module is opt-in: [`decode`] turns that Latin-1 view into
//! the text of the declared encoding, [`encode`] does the reverse before
//! writing, and nothing changes for who does not call them.
//!
//! The traditional encoding allows UTF-8 directly (in the fields of type U
//! and user-defined). UTF-16 and UTF-32 need the switch sequences of Annex
//! B.1.8 (`STX` … `ETX`, with Base-64 text), whose exact syntax the
//! standard gives no example of and that ANSI/NIST-ITL 1-2025 removed: they
//! are not supported, and text holding an `STX` is refused.
//!
//! ```
//! use ansi_nist_itl::encoding::{decode, encode, CharacterEncoding};
//! use ansi_nist_itl::{AsciiRecord, Message, Value};
//!
//! let mut msg = Message::new();
//! msg.type1_mut().unwrap().set("DCS", vec![vec!["3", "UTF-8"]])?;
//! assert_eq!(msg.character_encoding()?, CharacterEncoding::Utf8);
//!
//! let mut r2 = AsciiRecord::new(2)?;
//! r2.set_encoded("_18", "Hélène", CharacterEncoding::Utf8)?;
//! assert_eq!(r2.get("_18")?, Value::from("H\u{c3}\u{a9}l\u{c3}\u{a8}ne")); // the bytes, as Latin-1
//! assert_eq!(r2.get_decoded("_18", CharacterEncoding::Utf8)?, Value::from("Hélène"));
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```

use crate::error::{Error, ErrorKind, Result};
use crate::field::AnyField;
use crate::message::Message;
use crate::py::py_int;
use crate::record::AsciiRecord;
use crate::value::{Item, ListElem, Scalar, Value};

/// A character encoding set of Table 4 of the standard, by its index (CSI).
///
/// ```
/// use ansi_nist_itl::encoding::CharacterEncoding;
///
/// assert_eq!(CharacterEncoding::from_csi(3), Some(CharacterEncoding::Utf8));
/// assert_eq!(CharacterEncoding::from_csi(1), Some(CharacterEncoding::Latin1));
/// assert_eq!(CharacterEncoding::from_csi(200), Some(CharacterEncoding::UserDefined(200)));
/// assert_eq!(CharacterEncoding::from_csi(5), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CharacterEncoding {
    /// `0`: 7-bit ASCII, the default of the traditional encoding.
    Ascii,
    /// `1`: Latin-1 (ISO/IEC 8859-1), a legacy value ("8-bit ASCII" in
    /// the 2007 and 2008 versions).
    Latin1,
    /// `2`: UTF-16.
    Utf16,
    /// `3`: UTF-8.
    Utf8,
    /// `4`: UTF-32.
    Utf32,
    /// `128` to `999`: user-defined sets.
    UserDefined(u16),
}

impl CharacterEncoding {
    /// The set of an index; `None` for the reserved ones (5 to 127) and
    /// beyond 999.
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    ///
    /// assert_eq!(CharacterEncoding::from_csi(0), Some(CharacterEncoding::Ascii));
    /// ```
    pub fn from_csi(csi: u32) -> Option<Self> {
        match csi {
            0 => Some(CharacterEncoding::Ascii),
            1 => Some(CharacterEncoding::Latin1),
            2 => Some(CharacterEncoding::Utf16),
            3 => Some(CharacterEncoding::Utf8),
            4 => Some(CharacterEncoding::Utf32),
            128..=999 => u16::try_from(csi).ok().map(CharacterEncoding::UserDefined),
            _ => None,
        }
    }

    /// Index of the set (CSI).
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    ///
    /// assert_eq!(CharacterEncoding::Utf16.csi(), 2);
    /// ```
    pub fn csi(self) -> u32 {
        match self {
            CharacterEncoding::Ascii => 0,
            CharacterEncoding::Latin1 => 1,
            CharacterEncoding::Utf16 => 2,
            CharacterEncoding::Utf8 => 3,
            CharacterEncoding::Utf32 => 4,
            CharacterEncoding::UserDefined(n) => u32::from(n),
        }
    }

    /// Name of the set in Table 4 (CSN); `None` for user-defined sets.
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    ///
    /// assert_eq!(CharacterEncoding::Utf8.name(), Some("UTF-8"));
    /// ```
    pub fn name(self) -> Option<&'static str> {
        match self {
            CharacterEncoding::Ascii => Some("ASCII"),
            CharacterEncoding::Latin1 => Some("Latin-1"),
            CharacterEncoding::Utf16 => Some("UTF-16"),
            CharacterEncoding::Utf8 => Some("UTF-8"),
            CharacterEncoding::Utf32 => Some("UTF-32"),
            CharacterEncoding::UserDefined(_) => None,
        }
    }

    fn unsupported(self) -> Error {
        let what = self.name().map_or_else(
            || format!("character encoding set {}", self.csi()),
            String::from,
        );
        Error::new(
            ErrorKind::UnsupportedEncoding(Some(self.csi())),
            format!("{what} is not supported: only ASCII, Latin-1 and UTF-8 are decoded"),
        )
    }
}

/// `STX`, which opens a switch sequence of Annex B.1.8.
const STX: char = '\u{2}';

fn refuse_switch(text: &str) -> Result<()> {
    if text.contains(STX) {
        return Err(Error::new(
            ErrorKind::UnsupportedEncoding(None),
            "STX switch sequences (Annex B.1.8) are not supported",
        ));
    }
    Ok(())
}

/// The bytes of a Latin-1 view; a character beyond `U+00FF` cannot be one.
fn latin1_bytes(text: &str) -> Result<Vec<u8>> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).ok())
        .collect::<Option<Vec<u8>>>()
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Decoding,
                "the text holds a character beyond U+00FF: it is not a Latin-1 view of bytes",
            )
        })
}

/// Turns the Latin-1 view of a text, as the crate reads it, into the text
/// of `encoding`.
///
/// Fails with [`ErrorKind::Decoding`] if the bytes are not valid in the
/// encoding (a byte above `0x7F` in ASCII, malformed UTF-8), and with
/// [`ErrorKind::UnsupportedEncoding`] for UTF-16, UTF-32, user-defined sets
/// and `STX` switch sequences.
///
/// ```
/// use ansi_nist_itl::encoding::{decode, CharacterEncoding};
/// use ansi_nist_itl::ErrorKind;
///
/// assert_eq!(decode("Ã©t\u{e9}", CharacterEncoding::Latin1)?, "Ã©t\u{e9}");
/// assert_eq!(decode("\u{c3}\u{a9}t\u{c3}\u{a9}", CharacterEncoding::Utf8)?, "été");
/// assert_eq!(decode("\u{e9}", CharacterEncoding::Utf8).unwrap_err().kind(), &ErrorKind::Decoding);
/// assert_eq!(decode("\u{e9}", CharacterEncoding::Ascii).unwrap_err().kind(), &ErrorKind::Decoding);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
pub fn decode(text: &str, encoding: CharacterEncoding) -> Result<String> {
    refuse_switch(text)?;
    match encoding {
        CharacterEncoding::Latin1 => latin1_bytes(text).map(|_| text.to_owned()),
        CharacterEncoding::Ascii => {
            if let Some(c) = text.chars().find(|c| !c.is_ascii()) {
                return Err(Error::new(
                    ErrorKind::Decoding,
                    format!("character U+{:04X} is not 7-bit ASCII", u32::from(c)),
                ));
            }
            Ok(text.to_owned())
        }
        CharacterEncoding::Utf8 => String::from_utf8(latin1_bytes(text)?).map_err(|e| {
            Error::new(
                ErrorKind::Decoding,
                format!("invalid UTF-8 at byte {}", e.utf8_error().valid_up_to()),
            )
        }),
        other => Err(other.unsupported()),
    }
}

/// Turns a text into the Latin-1 view of its bytes in `encoding`, ready to
/// be set in a field and written as `nistitl` writes text.
///
/// Fails with [`ErrorKind::Encoding`] for a character the encoding cannot
/// hold, and with [`ErrorKind::UnsupportedEncoding`] as [`decode`].
///
/// ```
/// use ansi_nist_itl::encoding::{encode, CharacterEncoding};
/// use ansi_nist_itl::ErrorKind;
///
/// assert_eq!(encode("été", CharacterEncoding::Utf8)?, "\u{c3}\u{a9}t\u{c3}\u{a9}");
/// assert_eq!(encode("été", CharacterEncoding::Latin1)?, "été");
/// assert_eq!(encode("€", CharacterEncoding::Latin1).unwrap_err().kind(), &ErrorKind::Encoding);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
pub fn encode(text: &str, encoding: CharacterEncoding) -> Result<String> {
    refuse_switch(text)?;
    let limit = match encoding {
        CharacterEncoding::Ascii => 0x80,
        CharacterEncoding::Latin1 => 0x100,
        CharacterEncoding::Utf8 => return Ok(text.bytes().map(char::from).collect()),
        other => return Err(other.unsupported()),
    };
    if let Some(c) = text.chars().find(|&c| u32::from(c) >= limit) {
        return Err(Error::new(
            ErrorKind::Encoding,
            format!(
                "character U+{:04X} cannot be encoded in {}",
                u32::from(c),
                encoding.name().unwrap_or_default()
            ),
        ));
    }
    Ok(text.to_owned())
}

/// Applies `f` to every text of a value.
fn map_texts(value: Value, f: &dyn Fn(&str) -> Result<String>) -> Result<Value> {
    fn scalar(s: Scalar, f: &dyn Fn(&str) -> Result<String>) -> Result<Scalar> {
        match s {
            Scalar::Text(t) => f(&t).map(Scalar::Text),
            other => Ok(other),
        }
    }
    fn item(i: Item, f: &dyn Fn(&str) -> Result<String>) -> Result<Item> {
        match i {
            Item::Scalar(s) => scalar(s, f).map(Item::Scalar),
            Item::List(items) => items
                .into_iter()
                .map(|i| item(i, f))
                .collect::<Result<_>>()
                .map(Item::List),
        }
    }
    match value {
        Value::Scalar(s) => scalar(s, f).map(Value::Scalar),
        Value::List(elems) => elems
            .into_iter()
            .map(|e| match e {
                ListElem::Scalar(s) => scalar(s, f).map(ListElem::Scalar),
                ListElem::List(items) => items
                    .into_iter()
                    .map(|i| item(i, f))
                    .collect::<Result<_>>()
                    .map(ListElem::List),
            })
            .collect::<Result<_>>()
            .map(Value::List),
    }
}

impl AsciiRecord {
    /// [`get`](AsciiRecord::get), then every text of the value through
    /// [`decode`].
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    /// use ansi_nist_itl::{AsciiRecord, Value};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("_18", vec![vec!["D\u{c3}\u{bc}rr", "J\u{c3}\u{b6}rg"]])?;
    /// assert_eq!(
    ///     r.get_decoded("_18", CharacterEncoding::Utf8)?,
    ///     Value::from(vec![vec!["Dürr", "Jörg"]])
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn get_decoded(&self, name: &str, encoding: CharacterEncoding) -> Result<Value> {
        map_texts(self.get(name)?, &|t| decode(t, encoding))
    }

    /// Every text of `value` through [`encode`], then
    /// [`set`](AsciiRecord::set).
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set_encoded("_18", "Ñandú", CharacterEncoding::Utf8)?;
    /// assert_eq!(r.to_bytes()?.windows(2).filter(|w| w == b"\xc3\x91").count(), 1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_encoded(
        &mut self,
        name: &str,
        value: impl Into<Value>,
        encoding: CharacterEncoding,
    ) -> Result<()> {
        let value = map_texts(value.into(), &|t| encode(t, encoding))?;
        self.set(name, value)
    }
}

impl Message {
    /// The character encoding declared by the CSI of field 1.015; ASCII,
    /// the default of the traditional encoding, without that field.
    ///
    /// A CSI that is not a number gives the `int()` error; a reserved one
    /// (5 to 127, beyond 999) gives [`ErrorKind::UnsupportedEncoding`].
    ///
    /// ```
    /// use ansi_nist_itl::encoding::CharacterEncoding;
    /// use ansi_nist_itl::Message;
    ///
    /// let mut msg = Message::new();
    /// assert_eq!(msg.character_encoding()?, CharacterEncoding::Ascii);
    /// msg.type1_mut().unwrap().set("DCS", vec![vec!["1", "Latin-1"]])?;
    /// assert_eq!(msg.character_encoding()?, CharacterEncoding::Latin1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn character_encoding(&self) -> Result<CharacterEncoding> {
        let Some(AnyField::Text(dcs)) = self.type1().and_then(|r| r.field(15)) else {
            return Ok(CharacterEncoding::Ascii);
        };
        let csi = match dcs.value() {
            Value::Scalar(s) => Some(s),
            Value::List(elems) => elems.into_iter().next().and_then(|e| match e {
                ListElem::Scalar(s) => Some(s),
                ListElem::List(items) => items.into_iter().next().and_then(|i| match i {
                    Item::Scalar(s) => Some(s),
                    Item::List(_) => None,
                }),
            }),
        };
        let csi = match csi {
            Some(Scalar::Int(n)) => n,
            Some(Scalar::Text(t)) => py_int(&t).map_err(Error::from)?,
            _ => return Ok(CharacterEncoding::Ascii),
        };
        let index = csi.to_i64().and_then(|n| u32::try_from(n).ok());
        index.and_then(CharacterEncoding::from_csi).ok_or_else(|| {
            Error::new(
                ErrorKind::UnsupportedEncoding(index),
                format!("character encoding index {csi} is reserved"),
            )
        })
    }
}
