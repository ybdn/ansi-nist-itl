//! Records: tagged (SPEC §8) and binary (SPEC §9).

mod ascii;
mod binary;

pub use ascii::{AsciiRecord, FieldKey};
pub use binary::BinaryRecord;

use crate::alias;
use crate::error::Result;
use crate::value::Value;

/// Record of a message: tagged or binary.
///
/// ```
/// use ansi_nist_itl::{AsciiRecord, BinaryRecord, Record, Value};
///
/// let records = [
///     Record::from(AsciiRecord::new(2)?),
///     Record::from(BinaryRecord::new(4)),
/// ];
/// let types: Vec<u32> = records.iter().map(Record::record_type).collect();
/// assert_eq!(types, [2, 4]);
/// assert_eq!(records[1].idc()?, Value::from(1));
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    /// Tagged record: Types 1, 2, 9 to 22, 98 and 99.
    Ascii(AsciiRecord),
    /// Binary record: Types 3 to 8.
    Binary(BinaryRecord),
}

impl Record {
    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, Record};
    ///
    /// assert_eq!(Record::from(BinaryRecord::new(7)).record_type(), 7);
    /// ```
    pub fn record_type(&self) -> u32 {
        match self {
            Record::Ascii(r) => r.record_type(),
            Record::Binary(r) => r.record_type(),
        }
    }

    /// IDC (`record.IDC`): the value of the field `IDC` of a tagged record,
    /// as [`AsciiRecord::get`] reads it, or the IDC byte of a binary record.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Record, Value};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("IDC", "07")?;
    /// assert_eq!(Record::from(r).idc()?, Value::from("07"));
    ///
    /// let err = Record::from(AsciiRecord::new(1)?).idc().unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <IDC> while trying to retrieve a field in record of type 1"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn idc(&self) -> Result<Value> {
        match self {
            Record::Ascii(r) => r.get(alias::IDC),
            Record::Binary(r) => Ok(Value::from(u32::from(r.idc()))),
        }
    }

    /// Bytes of the record (`NIST`). The LEN of a tagged record is updated
    /// in place ([`AsciiRecord::to_bytes`]).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Record};
    ///
    /// let mut r = Record::from(AsciiRecord::new(2)?);
    /// assert_eq!(r.to_bytes()?, b"2.001:17\x1d2.002:0\x1c");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        match self {
            Record::Ascii(r) => r.to_bytes(),
            Record::Binary(r) => r.to_bytes(),
        }
    }

    /// Summary of the record (`str(record)`).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Record};
    ///
    /// let r = Record::from(AsciiRecord::new(2)?);
    /// assert_eq!(r.summary()?, " 2.001: LEN                           : 0\n 2.002: IDC                           : 0");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn summary(&self) -> Result<String> {
        match self {
            Record::Ascii(r) => Ok(r.summary()),
            Record::Binary(r) => r.summary(),
        }
    }

    /// The tagged record, if it is one.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, BinaryRecord, Record};
    ///
    /// assert!(Record::from(AsciiRecord::new(2)?).as_ascii().is_some());
    /// assert!(Record::from(BinaryRecord::new(4)).as_ascii().is_none());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn as_ascii(&self) -> Option<&AsciiRecord> {
        match self {
            Record::Ascii(r) => Some(r),
            Record::Binary(_) => None,
        }
    }

    /// The tagged record, mutable, if it is one.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Record, Value};
    ///
    /// let mut r = Record::from(AsciiRecord::new(2)?);
    /// r.as_ascii_mut().unwrap().set("IDC", 5)?;
    /// assert_eq!(r.idc()?, Value::from(5));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn as_ascii_mut(&mut self) -> Option<&mut AsciiRecord> {
        match self {
            Record::Ascii(r) => Some(r),
            Record::Binary(_) => None,
        }
    }

    /// The binary record, if it is one.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, Record};
    ///
    /// assert_eq!(Record::from(BinaryRecord::new(4)).as_binary().unwrap().idc(), 1);
    /// ```
    pub fn as_binary(&self) -> Option<&BinaryRecord> {
        match self {
            Record::Binary(r) => Some(r),
            Record::Ascii(_) => None,
        }
    }

    /// The binary record, mutable, if it is one.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryRecord, Record, Value};
    ///
    /// let mut r = Record::from(BinaryRecord::new(4));
    /// r.as_binary_mut().unwrap().set_idc(9);
    /// assert_eq!(r.idc()?, Value::from(9));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn as_binary_mut(&mut self) -> Option<&mut BinaryRecord> {
        match self {
            Record::Binary(r) => Some(r),
            Record::Ascii(_) => None,
        }
    }

    /// Python class name, for error messages.
    pub(crate) fn class_name(&self) -> &'static str {
        match self {
            Record::Ascii(_) => "AsciiRecord",
            Record::Binary(_) => "BinaryRecord",
        }
    }
}

impl From<AsciiRecord> for Record {
    fn from(r: AsciiRecord) -> Self {
        Record::Ascii(r)
    }
}

impl From<BinaryRecord> for Record {
    fn from(r: BinaryRecord) -> Self {
        Record::Binary(r)
    }
}
