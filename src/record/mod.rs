//! Records: tagged (SPEC §8) and binary (SPEC §9).

mod ascii;
mod binary;

pub use ascii::{AsciiRecord, FieldKey};
pub use binary::BinaryRecord;
