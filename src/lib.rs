//! Parser and writer for ANSI/NIST-ITL biometric transaction files
//! (`.nist`, `.eft`, `.an2`, `.int`…).
//!
//! The crate is a port of [`nistitl`](https://github.com/idemia/python-nistitl)
//! 0.6 (Python, © IDEMIA, CeCILL-C): by default it reads and writes exactly
//! as `nistitl` does, quirks included, with the same error messages. Two
//! additions are opt-in: an exact writing that gives back the bytes read
//! ([`Message::to_bytes_exact`]), and a tolerant mode that reads damaged
//! files with located diagnostics ([`ParseOptions::tolerant`]).
//!
//! # Reading
//!
//! ```
//! use ansi_nist_itl::{alias, Message};
//!
//! # let bytes = {
//! #     let mut msg = Message::new();
//! #     msg.set_tot("CAR")?;
//! #     let mut r = ansi_nist_itl::AsciiRecord::new(14)?;
//! #     r.set("IDC", 1)?;
//! #     r.set_bytes("DATA", b"wsq".to_vec())?;
//! #     msg.push(r)?;
//! #     msg.to_bytes()?
//! # };
//! let msg = Message::parse(&bytes)?; // or Message::from_path("file.nist")
//! assert_eq!(msg.tot()?.to_string(), "CAR");
//! for record in msg.records_of(14) {
//!     let r = record.as_ascii().unwrap();
//!     println!("IDC {}: {} bytes of image", r.idc()?, r.get_bytes(alias::DATA)?.len());
//! }
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! # Writing
//!
//! ```
//! use ansi_nist_itl::{AsciiRecord, BinaryRecord, ImageHeader, Message};
//!
//! let mut msg = Message::new(); // Type-1 with today's date and a new TCN
//! msg.set_tot("CAR")?;
//!
//! let mut r2 = AsciiRecord::new(2)?;
//! r2.set("IDC", 0)?;
//! r2.set("_18", vec!["DOE", "JOHN"])?; // subfields, separated by RS
//! msg.push(r2)?;
//!
//! let mut r4 = BinaryRecord::new(4);
//! r4.set_idc(1);
//! r4.set_image_header(&ImageHeader { fgp: [1, 255, 255, 255, 255, 255], ..Default::default() })?;
//! r4.set_image_data(b"wsq".to_vec())?;
//! msg.push(r4)?;
//!
//! let bytes = msg.to_bytes()?; // LEN and CNT recomputed, fields sorted
//! assert_eq!(Message::parse(&bytes)?.len(), 3);
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! # Model
//!
//! A [`Message`] holds a Type-1 record, then [`Record`]s: tagged
//! ([`AsciiRecord`]: Types 1, 2, 9 to 22, 98, 99) or binary
//! ([`BinaryRecord`]: Types 3 to 8). A tagged record holds [`Field`]s (text,
//! split into [`SubField`]s and [`Item`]s by RS and US) and
//! [`BinaryField`]s (the data at tag 999). Fields are found by tag or by
//! [`alias`] (`IDC`, `SRC`, `DATA`…), as `nistitl` names them.
//!
//! Values are Python values ([`Value`], [`Scalar`], [`PyInt`]): `nistitl`
//! stores text, numbers (LEN and IDC are read as numbers), `None` and
//! booleans, and writes them with Python's `str()`. Binary data are
//! [`Bytes`], views on the buffer read, never copied.
//!
//! # Parity with `nistitl`
//!
//! [`Message::parse`] accepts and refuses what `nistitl` does, with the
//! same error messages ([`Error`]'s `Display`); [`Message::to_bytes`] and
//! [`Message::summary`] give the same bytes and text. Python exceptions that
//! `nistitl` does not catch become typed errors ([`ErrorKind`]); every error
//! also carries a [`Position`]: record, tag, byte offset. The crate never
//! panics on its input, which is fuzzed.
//!
//! # Versions
//!
//! As `nistitl`, the crate never reads the version of the standard (1.002,
//! kept as text): every version is read and written the same way, the
//! traditional ones (0200 to 0500) as the later ones. No rule depends on the
//! version: a record type is accepted whatever the version declares, and
//! Types 3, 5 and 6 give a [`Warning::Deprecated`] in every version.
//!
//! ```
//! use ansi_nist_itl::{Message, Value};
//!
//! let mut msg = Message::new();
//! msg.type1_mut().unwrap().set("VER", "0200")?;
//! let bytes = msg.to_bytes()?;
//! let read = Message::parse(&bytes)?;
//! assert_eq!(read.type1().unwrap().get("VER")?, Value::from("0200"));
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! # Exact writing
//!
//! `nistitl` sorts fields, writes labels on 3 digits and recomputes LEN and
//! CNT. [`Message::to_bytes_exact`] keeps the bytes read for what did not
//! change instead:
//!
//! ```
//! use ansi_nist_itl::Message;
//!
//! # let bytes = Message::new().to_bytes()?;
//! let msg = Message::parse(&bytes)?;
//! assert_eq!(msg.to_bytes_exact()?, bytes);
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! # Tolerant mode
//!
//! ```
//! use ansi_nist_itl::{Message, ParseOptions, Severity};
//!
//! # let bytes = Message::new().to_bytes()?;
//! let truncated = &bytes[..bytes.len() - 3];
//! let parsed = Message::parse_with(truncated, ParseOptions::tolerant())?;
//! for d in &parsed.diagnostics {
//!     println!("{d} at byte {:?}", d.position.offset);
//! }
//! assert!(parsed.diagnostics.iter().all(|d| d.severity < Severity::Fatal));
//! assert_eq!(parsed.message.to_bytes_exact()?, truncated);
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! # Serialization
//!
//! With the `serde` feature, the model implements `Serialize` and
//! `Deserialize`, in a stable form: values as Python shows them, binary data
//! in base64 in human-readable formats (raw bytes in the others). Reading
//! goes through the API, with the same checks and the same messages.
//!
//! ```
//! # #[cfg(feature = "serde")] {
//! use ansi_nist_itl::{AsciiRecord, Message};
//! use serde_json::json;
//!
//! let mut r2 = AsciiRecord::new(2)?;
//! r2.set("IDC", 1)?;
//! r2.set("_18", vec![vec!["DOE", "JOHN"]])?;
//! assert_eq!(
//!     serde_json::to_value(&r2).unwrap(),
//!     json!({ "type": 2, "fields": [
//!         { "tag": 1, "alias": "LEN", "value": 0 },
//!         { "tag": 2, "alias": "IDC", "value": 1 },
//!         { "tag": 18, "value": [["DOE", "JOHN"]] },
//!     ] })
//! );
//!
//! let json = json!({ "records": [{ "type": 1, "fields": [] }, { "type": 1, "fields": [] }] });
//! let err = serde_json::from_value::<Message>(json).unwrap_err();
//! assert_eq!(
//!     err.to_string(),
//!     "CANNOT_ADD_TYPE1: Cannot add a type 1 record: it must be the first record"
//! );
//! # }
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```
//!
//! | Rust | JSON |
//! |------|------|
//! | `None`, booleans, text | `null`, `true`/`false`, string |
//! | [`PyInt`] | number if it fits in `i64`, else `{"int": "-123…"}` |
//! | lists ([`Value`], [`ListElem`], [`Item`]) | nested arrays |
//! | [`Kind`] | `"FSI"`, `"SI"`… |
//! | [`Bytes`] | base64 (RFC 4648, padded) |
//! | text field | `{"tag", "alias"?, "kind"?, "format"?, "value"}` |
//! | binary field | `{"tag", "alias"?, "format"?, "bytes"}` |
//! | tagged record | `{"type", "autosort"?, "fields"}` |
//! | binary record | `{"type", "idc", "format"?, "bytes"}` |
//! | message | `{"records"}`, the Type-1 first |
//!
//! Optional keys are left out at their default: no alias, kind `FSI` (`SI`
//! for a lone [`SubField`]), format `%d.%03d:`, `autosort` true. A lone
//! field also carries its `record`. Unknown keys are refused. The bytes
//! read for [`Message::to_bytes_exact`] are not serialized.
//!
//! # Features
//!
//! - `local-date` (default): today's date of a new Type-1 is the local date,
//!   as `nistitl` gives it; without it, the UTC date. [`Defaults`] fixes the
//!   date and the transaction control number.
//! - `serde`: `Serialize` and `Deserialize` on the model (see above).
//! - `dictionary`: the `dictionary` module, the fields of
//!   each record type, after ANSI/NIST-ITL 1-2011 Update 2015.
//! - `validation` (implies `dictionary`): the `validation` module,
//!   `Message::validate` against the tables of the standard, and
//!   `Message::validate_profile` against FBI EBTS 11.2 or INTERPOL INT-I 5.03.
//!
//! Image decoding (WSQ, JPEG 2000) is out of scope.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod alias;
mod bytes;
pub mod codes;
mod defaults;
#[cfg(feature = "dictionary")]
pub mod dictionary;
pub mod encoding;
mod error;
mod field;
pub mod image;
mod message;
mod py;
mod record;
#[cfg(feature = "serde")]
mod serde_impl;
mod structfmt;
mod subfield;
#[cfg(feature = "validation")]
pub mod validation;
mod value;

pub use bytes::Bytes;
pub use defaults::Defaults;
pub use error::{
    Diagnostic, DiagnosticCode, Error, ErrorKind, NistError, Position, Result, Severity, Warning,
};
pub use field::{AnyField, BinaryField, Field};
pub use message::{Message, ParseOptions, Parsed, RecordVisitor, parse_record};
pub use record::{AsciiRecord, BinaryRecord, FieldKey, ImageHeader, Record, SignatureHeader};
pub use structfmt::StructValue;
pub use subfield::SubField;
pub use value::{Item, Kind, ListElem, PyInt, Scalar, SubValue, Value};

/// File separator (`0x1C`): ends a tagged record.
pub const FS: u8 = 0x1C;
/// Group separator (`0x1D`): separates the fields of a tagged record.
pub const GS: u8 = 0x1D;
/// Record separator (`0x1E`): separates the subfields of a field.
pub const RS: u8 = 0x1E;
/// Unit separator (`0x1F`): separates the items of a subfield.
pub const US: u8 = 0x1F;
