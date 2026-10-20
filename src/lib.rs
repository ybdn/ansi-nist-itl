//! Parser and writer for ANSI/NIST-ITL biometric transaction files.
//!
//! This crate is under development. See `docs/PRD.md` for the scope.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// The conversions and error constructors are consumed by the field, record
// and message modules, which come in the next development steps.
#[allow(dead_code)]
mod alias;
mod bytes;
#[allow(dead_code)]
mod error;
#[allow(dead_code, unused_imports)]
mod py;
mod value;

pub use bytes::Bytes;
pub use error::{Error, ErrorKind, NistError, Position, Result};
pub use value::{Item, Kind, ListElem, PyInt, Scalar, SubValue, Value};
