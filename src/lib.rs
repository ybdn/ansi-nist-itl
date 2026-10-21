//! Parser and writer for ANSI/NIST-ITL biometric transaction files.
//!
//! This crate is under development. See `docs/PRD.md` for the scope.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod alias;
mod bytes;
mod defaults;
mod error;
mod field;
mod message;
mod py;
mod record;
mod structfmt;
mod subfield;
mod value;

pub use bytes::Bytes;
pub use defaults::Defaults;
pub use error::{Error, ErrorKind, NistError, Position, Result, Warning};
pub use field::{AnyField, BinaryField, Field};
pub use message::{Message, ParseOptions, Parsed, RecordVisitor, parse_record};
pub use record::{AsciiRecord, BinaryRecord, FieldKey, Record};
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
