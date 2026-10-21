//! Messages: a Type-1 record followed by the other records (SPEC §10).

mod lexer;
mod parse;

pub use lexer::{RecordVisitor, parse_record};
pub use parse::{ParseOptions, Parsed};

use crate::alias;
use crate::defaults::Defaults;
use crate::error::{Error, Position, Result};
use crate::field::{AnyField, Field};
use crate::py::py_int;
use crate::record::{AsciiRecord, Record};
use crate::subfield::SubField;
use crate::value::{Item, PyInt, Scalar, Value};

/// `int(value)` of a field value.
fn int_of(value: &Value) -> Result<PyInt> {
    match value {
        Value::Scalar(s) => int_of_scalar(s),
        Value::List(_) => Err(Error::int_argument("list")),
    }
}

/// `int(value)` of a scalar.
fn int_of_scalar(value: &Scalar) -> Result<PyInt> {
    match value {
        Scalar::Int(n) => Ok(n.clone()),
        Scalar::Bool(b) => Ok(PyInt::from(u32::from(*b))),
        Scalar::Text(s) => py_int(s).map_err(Error::from),
        Scalar::Null => Err(Error::int_argument("NoneType")),
    }
}

/// Sets the index of the record in error, keeping the rest of its position.
fn at_record(index: usize) -> impl Fn(Error) -> Error {
    move |mut e| {
        e.position_mut().record_index = Some(index);
        e
    }
}

/// ANSI/NIST-ITL message (`Message`): the Type-1 record, then the other
/// records in order.
///
/// The Type-1 is always the first record and cannot be removed; its CNT
/// (`1.003`) is recomputed from the records on write.
///
/// ```
/// use ansi_nist_itl::{AsciiRecord, Defaults, Message};
///
/// let defaults = Defaults { date: Some("20200904".into()), tcn: Some("TCN".into()) };
/// let mut msg = Message::with_defaults(&defaults);
/// msg.set_tot("TEST")?;
/// let mut r2 = AsciiRecord::new(2)?;
/// r2.set("IDC", 1)?;
/// msg.push(r2)?;
/// let bytes = msg.to_bytes()?;
/// assert!(bytes.starts_with(b"1.001:"));
/// assert_eq!(msg.cnt()?.to_bytes()?, b"1.003:1\x1f1\x1e2\x1f1");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    records: Vec<Record>,
}

impl Default for Message {
    fn default() -> Self {
        Message::new()
    }
}

impl Message {
    /// `Message()`: a message made of a new Type-1 record, with today's date
    /// and a new transaction control number.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// let msg = Message::new();
    /// assert_eq!(msg.len(), 1);
    /// assert_eq!(msg.tot()?.to_string(), "");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn new() -> Self {
        Message::with_defaults(&Defaults::default())
    }

    /// [`Message::new`], taking the date and the transaction control number
    /// from `defaults` (SPEC §14.4).
    ///
    /// ```
    /// use ansi_nist_itl::{Defaults, Message, Value};
    ///
    /// let defaults = Defaults { date: Some("20190517".into()), tcn: None };
    /// let msg = Message::with_defaults(&defaults);
    /// assert_eq!(msg.type1().unwrap().get("DAT")?, Value::from("20190517"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn with_defaults(defaults: &Defaults) -> Self {
        let mut msg = Message {
            records: Vec::new(),
        };
        msg.reset_with(true, true, defaults);
        msg
    }

    /// Leaves only a new Type-1 record: with its mandatory fields if
    /// `autocreate`, empty otherwise; its text fields are written sorted by
    /// tag if `autosort`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// msg.reset(false, true);
    /// assert_eq!(msg.len(), 1);
    /// assert!(msg.type1().unwrap().fields().is_empty());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn reset(&mut self, autocreate: bool, autosort: bool) {
        self.reset_with(autocreate, autosort, &Defaults::default());
    }

    fn reset_with(&mut self, autocreate: bool, autosort: bool, defaults: &Defaults) {
        // The Type-1 is in the alias table: its creation cannot fail.
        let type1 = AsciiRecord::with_defaults(1, autocreate, autosort, defaults)
            .or_else(|_| AsciiRecord::with(1, false, autosort));
        self.records = type1.map(Record::Ascii).into_iter().collect();
    }

    /// Adds a record at the end (`msg += record`). A Type-1 gives
    /// `CANNOT_ADD_TYPE1`; IDCs are not checked.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// assert_eq!(msg.len(), 2);
    ///
    /// let err = msg.push(AsciiRecord::new(1)?).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "CANNOT_ADD_TYPE1: Cannot add a type 1 record: it must be the first record"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn push(&mut self, record: impl Into<Record>) -> Result<()> {
        let record = record.into();
        if record.record_type() == 1 {
            return Err(Error::cannot_add_type1().with_position(Position {
                record_index: Some(self.records.len()),
                record_type: Some(1),
                ..Position::default()
            }));
        }
        self.records.push(record);
        Ok(())
    }

    /// All the records, the Type-1 first (`iter(msg)`).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message, Record};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// let types: Vec<u32> = msg.records().iter().map(Record::record_type).collect();
    /// assert_eq!(types, [1, 2]);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// The records of type `record_type`, in order (`msg.iter(t)`).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, BinaryRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// msg.push(BinaryRecord::new(4))?;
    /// msg.push(BinaryRecord::new(4))?;
    /// assert_eq!(msg.records_of(4).count(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn records_of(&self, record_type: u32) -> impl Iterator<Item = &Record> + '_ {
        self.records
            .iter()
            .filter(move |r| r.record_type() == record_type)
    }

    /// Record at `index` (`msg[i]`), the Type-1 being at 0.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// let msg = Message::new();
    /// assert_eq!(msg.record(0)?.record_type(), 1);
    /// assert_eq!(msg.record(100).unwrap_err().to_string(), "list index out of range");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record(&self, index: usize) -> Result<&Record> {
        self.records
            .get(index)
            .ok_or_else(|| self.index_error(index, Error::index_out_of_range()))
    }

    /// Mutable record at `index`.
    ///
    /// Replacing the Type-1 by another kind of record makes the operations
    /// that need it ([`tot`](Message::tot), [`cnt`](Message::cnt)…) fail.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message, Value};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// msg.record_mut(1)?.as_ascii_mut().unwrap().set("IDC", 3)?;
    /// assert_eq!(msg.record(1)?.idc()?, Value::from(3));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record_mut(&mut self, index: usize) -> Result<&mut Record> {
        let error = self.index_error(index, Error::index_out_of_range());
        self.records.get_mut(index).ok_or(error)
    }

    /// The Type-1 record (`msg[0]`), unless it was replaced through
    /// [`record_mut`](Message::record_mut) by a binary record.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// assert_eq!(Message::new().type1().unwrap().record_type(), 1);
    /// ```
    pub fn type1(&self) -> Option<&AsciiRecord> {
        self.records.first().and_then(Record::as_ascii)
    }

    /// The Type-1 record, mutable.
    ///
    /// ```
    /// use ansi_nist_itl::{Message, Value};
    ///
    /// let mut msg = Message::new();
    /// msg.type1_mut().unwrap().set("DAT", "20090924")?;
    /// assert_eq!(msg.type1().unwrap().get("DAT")?, Value::from("20090924"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn type1_mut(&mut self) -> Option<&mut AsciiRecord> {
        self.records.first_mut().and_then(Record::as_ascii_mut)
    }

    /// First record of type `record_type` whose IDC is `idc`
    /// (`msg[(t, idc)]`), both compared through Python's `int()`.
    ///
    /// Python's order of evaluation is kept: for each record, its type,
    /// then `int(record.IDC)`, then `int(idc)`. A record of the type without
    /// IDC (a Type-1) gives `UNKNOWN_ATTRIBUTE`; no record gives
    /// `IndexOutOfRange`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("IDC", "07")?;
    /// msg.push(r)?;
    /// assert_eq!(msg.by_key(2, 7)?.record_type(), 2);
    /// assert_eq!(msg.by_key(2, " 7")?.record_type(), 2);
    /// assert_eq!(msg.by_key(2, 100).unwrap_err().to_string(), "list index out of range");
    /// // `"abc"` is only converted when a Type-9 exists.
    /// assert_eq!(msg.by_key(9, "abc").unwrap_err().to_string(), "list index out of range");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn by_key(&self, record_type: u32, idc: impl Into<Scalar>) -> Result<&Record> {
        let index = self.find(record_type, &idc.into())?;
        Ok(&self.records[index])
    }

    /// [`Message::by_key`], mutable.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// msg.by_key_mut(2, 0)?.as_ascii_mut().unwrap().set("_3", "x")?;
    /// assert!(msg.by_key(2, 0)?.as_ascii().unwrap().field(3).is_some());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn by_key_mut(&mut self, record_type: u32, idc: impl Into<Scalar>) -> Result<&mut Record> {
        let index = self.find(record_type, &idc.into())?;
        Ok(&mut self.records[index])
    }

    /// Index of the first record matching `(t, idc)`, `None` if there is none.
    fn position_of(&self, record_type: u32, idc: &Scalar) -> Result<Option<usize>> {
        for (i, r) in self.records.iter().enumerate() {
            if r.record_type() != record_type {
                continue;
            }
            let own = r.idc().and_then(|v| int_of(&v)).map_err(at_record(i))?;
            let key = int_of_scalar(idc).map_err(at_record(i))?;
            if own == key {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn find(&self, record_type: u32, idc: &Scalar) -> Result<usize> {
        self.position_of(record_type, idc)?.ok_or_else(|| {
            Error::index_out_of_range().with_position(Position {
                record_type: Some(record_type),
                ..Position::default()
            })
        })
    }

    /// Number of records, the Type-1 included (`len(msg)`).
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// assert_eq!(Message::new().len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Always `false`: a message has at least its Type-1.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// assert!(!Message::new().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Removes the record at `index` (`del msg[i]`) and returns it.
    ///
    /// The Type-1, at 0, gives `CANNOT_DELETE_TYPE1`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// assert_eq!(msg.remove(1)?.record_type(), 2);
    /// assert_eq!(
    ///     msg.remove(0).unwrap_err().to_string(),
    ///     "CANNOT_DELETE_TYPE1: Cannot delete the type 1 record"
    /// );
    /// assert_eq!(msg.remove(5).unwrap_err().to_string(), "list assignment index out of range");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn remove(&mut self, index: usize) -> Result<Record> {
        if index == 0 {
            return Err(self.index_error(0, Error::cannot_delete_type1()));
        }
        if index >= self.records.len() {
            return Err(self.index_error(index, Error::assignment_index_out_of_range()));
        }
        Ok(self.records.remove(index))
    }

    /// Removes the first record of type `record_type` whose IDC is `idc`
    /// (`del msg[(t, idc)]`), found as by [`Message::by_key`], and returns
    /// it.
    ///
    /// Type 1 gives `CANNOT_DELETE_TYPE1`; no record gives the `TypeError`
    /// of `nistitl` (`TypeMismatch`).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// msg.push(AsciiRecord::new(2)?)?;
    /// msg.remove_by_key(2, 0)?;
    /// assert_eq!(msg.len(), 1);
    /// assert_eq!(
    ///     msg.remove_by_key(2, 9).unwrap_err().to_string(),
    ///     "list indices must be integers or slices, not tuple"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn remove_by_key(&mut self, record_type: u32, idc: impl Into<Scalar>) -> Result<Record> {
        if record_type == 1 {
            return Err(self.index_error(0, Error::cannot_delete_type1()));
        }
        match self.position_of(record_type, &idc.into())? {
            Some(i) => Ok(self.records.remove(i)),
            None => Err(Error::tuple_index().with_position(Position {
                record_type: Some(record_type),
                ..Position::default()
            })),
        }
    }

    /// Removes the first record equal to `record` (`msg - record`) and
    /// returns it.
    ///
    /// `nistitl` compares identities; records are compared by value here. A
    /// Type-1 gives `CANNOT_DELETE_TYPE1` (`nistitl` would remove it), no
    /// equal record `RECORD_NOT_FOUND`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message, Record};
    ///
    /// let mut msg = Message::new();
    /// let r = Record::from(AsciiRecord::new(2)?);
    /// msg.push(r.clone())?;
    /// msg.remove_record(&r)?;
    /// assert_eq!(msg.len(), 1);
    /// assert_eq!(
    ///     msg.remove_record(&r).unwrap_err().to_string(),
    ///     "RECORD_NOT_FOUND: Cannot remove a record: record not found"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn remove_record(&mut self, record: &Record) -> Result<Record> {
        if record.record_type() == 1 {
            return Err(self.index_error(0, Error::cannot_delete_type1()));
        }
        match self.records.iter().position(|r| r == record) {
            Some(i) => Ok(self.records.remove(i)),
            None => Err(Error::record_not_found().with_position(Position {
                record_type: Some(record.record_type()),
                ..Position::default()
            })),
        }
    }

    /// TOT of the Type-1 (`msg.TOT`), as [`AsciiRecord::get`] reads it.
    ///
    /// ```
    /// use ansi_nist_itl::{Message, Value};
    ///
    /// let mut msg = Message::new();
    /// msg.set_tot("TOTFORTEST")?;
    /// assert_eq!(msg.tot()?, Value::from("TOTFORTEST"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn tot(&self) -> Result<Value> {
        self.first_ascii(alias::TOT)?.get(alias::TOT)
    }

    /// Sets the TOT of the Type-1, as [`AsciiRecord::set`] does.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// let mut msg = Message::new();
    /// msg.set_tot("CAR")?;
    /// assert_eq!(msg.tot()?.to_string(), "CAR");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_tot(&mut self, value: impl Into<Value>) -> Result<()> {
        let value = value.into();
        self.first_ascii_mut(alias::TOT)?.set(alias::TOT, value)
    }

    /// Recomputes the CNT of the Type-1 (`msg.CNT`) and returns it.
    ///
    /// The CNT is reset, then gets one subfield `1 US <number of records>`,
    /// then one subfield `<type> US <IDC>` per record, the IDC taken as is
    /// (`"07"` stays `07`). An absent CNT gives `MissingField`.
    ///
    /// The value cannot be set: `nistitl` raises `AttributeError`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("IDC", 3)?;
    /// msg.push(r)?;
    /// assert_eq!(msg.cnt()?.to_bytes()?, b"1.003:1\x1f1\x1e2\x1f3");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    ///
    /// ```compile_fail
    /// let mut msg = ansi_nist_itl::Message::new();
    /// msg.set_cnt(12);
    /// ```
    pub fn cnt(&mut self) -> Result<&Field> {
        let others = self.records.len().saturating_sub(1);
        let (first, rest) = self
            .records
            .split_first_mut()
            .ok_or_else(|| Error::index_out_of_range().with_position(Position::default()))?;
        let field = match first {
            Record::Ascii(r) => match r.field_mut(alias::CNT) {
                Some(AnyField::Text(f)) => f,
                Some(AnyField::Binary(_)) => {
                    return Err(
                        Error::no_attribute("BinaryField", "reset").with_position(cnt_position())
                    );
                }
                None => return Err(Error::none_attribute("reset").with_position(cnt_position())),
            },
            Record::Binary(_) => {
                return Err(Error::not_subscriptable("BinaryRecord").with_position(cnt_position()));
            }
        };
        field.reset();
        let count = u64::try_from(others).unwrap_or(u64::MAX);
        add_cnt_entry(field, || Ok((Item::from(1), Item::from(count)))).map_err(at_record(0))?;
        for (i, r) in rest.iter().enumerate() {
            // `r.IDC` is read once the subfield is added: on error, the
            // empty subfield stays, as in `nistitl`.
            add_cnt_entry(field, || {
                Ok((Item::from(r.record_type()), Item::from(r.idc()?)))
            })
            .map_err(at_record(i + 1))?;
        }
        Ok(field)
    }

    /// Bytes of the message (`NIST`): the CNT is recomputed, then the
    /// records are written in order, each updating its LEN.
    ///
    /// ```
    /// use ansi_nist_itl::{Defaults, Message};
    ///
    /// let defaults = Defaults { date: Some("20190517".into()), tcn: Some("doctest".into()) };
    /// let mut msg = Message::with_defaults(&defaults);
    /// msg.set_tot("TEST")?;
    /// assert_eq!(
    ///     msg.to_bytes()?,
    ///     b"1.001:115\x1d1.002:0400\x1d1.003:1\x1f0\x1d1.004:TEST\x1d1.005:20190517\x1d\
    ///       1.007:000\x1d1.008:000\x1d1.009:doctest\x1d1.011:00.00\x1d1.012:00.00\x1c"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        self.cnt()?;
        let mut out = Vec::new();
        for (i, r) in self.records.iter_mut().enumerate() {
            out.extend(r.to_bytes().map_err(at_record(i))?);
        }
        Ok(out)
    }

    /// Summary of the message (`str(msg)`): the CNT is recomputed, then the
    /// summaries of the records are joined by newlines. LEN is not
    /// recomputed.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Message};
    ///
    /// let mut msg = Message::new();
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("IDC", 3)?;
    /// msg.push(r)?;
    /// let summary = msg.summary()?;
    /// assert!(summary.contains(" 1.003: CNT                           : [[1, 1], [2, 3]]\n"));
    /// assert!(summary.ends_with(" 2.002: IDC                           : 3"));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn summary(&mut self) -> Result<String> {
        self.cnt()?;
        let mut parts = Vec::with_capacity(self.records.len());
        for (i, r) in self.records.iter().enumerate() {
            parts.push(r.summary().map_err(at_record(i))?);
        }
        Ok(parts.join("\n"))
    }

    fn index_error(&self, index: usize, e: Error) -> Error {
        let record_type = self.records.get(index).map(Record::record_type);
        e.with_position(Position {
            record_index: Some(index),
            record_type,
            ..Position::default()
        })
    }

    /// The Type-1, for reading `attribute`.
    fn first_ascii(&self, attribute: &str) -> Result<&AsciiRecord> {
        match self.records.first() {
            Some(Record::Ascii(r)) => Ok(r),
            other => Err(self.not_type1(other, attribute)),
        }
    }

    fn first_ascii_mut(&mut self, attribute: &str) -> Result<&mut AsciiRecord> {
        if !matches!(self.records.first(), Some(Record::Ascii(_))) {
            return Err(self.not_type1(self.records.first(), attribute));
        }
        match self.records.first_mut() {
            Some(Record::Ascii(r)) => Ok(r),
            _ => Err(Error::index_out_of_range()),
        }
    }

    fn not_type1(&self, first: Option<&Record>, attribute: &str) -> Error {
        let class = first.map_or("NoneType", Record::class_name);
        self.index_error(0, Error::no_attribute(class, attribute))
    }
}

fn cnt_position() -> Position {
    Position {
        record_index: Some(0),
        record_type: Some(1),
        tag: Some(PyInt::from(3)),
        ..Position::default()
    }
}

/// `sf = SubField(); f.add_subfields(sf); sf.add_values(a, b)`, the
/// values being computed after the subfield is added.
fn add_cnt_entry(field: &mut Field, values: impl FnOnce() -> Result<(Item, Item)>) -> Result<()> {
    field.add_subfields([SubField::default()])?;
    let (a, b) = values()?;
    let last = field.len().saturating_sub(1);
    field.subfield_mut(last)?.add_values([a, b])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::record::BinaryRecord;

    fn with_record(r: impl Into<Record>) -> Message {
        let mut msg = Message::new();
        msg.push(r).unwrap();
        msg
    }

    #[test]
    fn cnt_keeps_the_empty_subfield_of_a_failing_idc() {
        let mut r = AsciiRecord::with(2, false, true).unwrap();
        r.add_field(Field::new(2, 1, 0).unwrap()).unwrap();
        let mut msg = with_record(r);
        let e = msg.cnt().unwrap_err();
        assert_eq!(e.nist_error(), Some(crate::NistError::UnknownAttribute));
        assert_eq!(e.position().record_index, Some(1));
        let cnt = msg.type1().unwrap().field("CNT").unwrap();
        assert_eq!(cnt.to_bytes().unwrap(), b"1.003:1\x1f1\x1e");
    }

    #[test]
    fn cnt_idc_as_is() {
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", vec!["1", "2"]).unwrap();
        let mut msg = with_record(r);
        assert_eq!(
            msg.cnt().unwrap().to_bytes().unwrap(),
            b"1.003:1\x1f1\x1e2\x1f['1', '2']"
        );
        assert!(
            msg.summary()
                .unwrap()
                .contains(" 1.003: CNT                           : [[1, 1], [2, ['1', '2']]]")
        );
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", true).unwrap();
        let mut msg = with_record(r);
        assert_eq!(
            msg.cnt().unwrap().to_bytes().unwrap(),
            b"1.003:1\x1f1\x1e2\x1fTrue"
        );
        assert_eq!(msg.by_key(2, 1).unwrap().record_type(), 2);
        let mut msg = with_record(BinaryRecord::new(4));
        assert_eq!(
            msg.cnt().unwrap().to_bytes().unwrap(),
            b"1.003:1\x1f1\x1e4\x1f1"
        );
        assert_eq!(msg.by_key(4, "1").unwrap().record_type(), 4);
    }

    #[test]
    fn by_key_conversions() {
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", vec!["1", "2"]).unwrap();
        let msg = with_record(r);
        let e = msg.by_key(2, 1).unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
        assert_eq!(
            e.to_string(),
            "int() argument must be a string, a bytes-like object or a real number, not 'list'"
        );
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", ()).unwrap();
        let msg = with_record(r);
        assert!(
            msg.by_key(2, 0)
                .unwrap_err()
                .to_string()
                .ends_with("not 'NoneType'")
        );
        // The key is only converted for a record of the type.
        let msg = with_record(AsciiRecord::new(2).unwrap());
        assert_eq!(
            msg.by_key(2, "x").unwrap_err().kind(),
            &ErrorKind::InvalidLiteral
        );
        assert_eq!(
            msg.by_key(9, "x").unwrap_err().kind(),
            &ErrorKind::IndexOutOfRange
        );
    }

    #[test]
    fn missing_cnt() {
        let mut msg = Message::new();
        msg.type1_mut().unwrap().remove_field(3);
        let e = msg.cnt().unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::MissingField);
        assert_eq!(e.to_string(), "'NoneType' object has no attribute 'reset'");
        assert!(msg.to_bytes().is_err());
        // Any field with the alias CNT will do.
        msg.type1_mut().unwrap().set("_3", "x").unwrap();
        assert_eq!(msg.cnt().unwrap().to_bytes().unwrap(), b"1.003:1\x1f0");
    }

    #[test]
    fn reset_without_autocreate() {
        let mut msg = Message::new();
        msg.reset(false, true);
        assert_eq!(
            msg.tot().unwrap_err().to_string(),
            "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <TOT> while trying to retrieve a field in record of type 1"
        );
        assert_eq!(msg.cnt().unwrap_err().kind(), &ErrorKind::MissingField);
    }

    #[test]
    fn cnt_kind_checks() {
        let mut t1 = AsciiRecord::with(1, false, true).unwrap();
        t1.add_field(Field::new(1, 1, 0).unwrap()).unwrap();
        t1.add_field(Field::with(1, 3, "", "F", "%d.%03d:", "").unwrap())
            .unwrap();
        let mut msg = Message::new();
        *msg.record_mut(0).unwrap() = Record::from(t1.clone());
        assert_eq!(
            msg.cnt().unwrap_err().to_string(),
            "BAD_FIELD_VALUE: Field 1.003: cannot have subfields"
        );
        t1.remove_field(3);
        t1.add_field(Field::with(1, 3, "", "S", "%d.%03d:", "").unwrap())
            .unwrap();
        *msg.record_mut(0).unwrap() = Record::from(t1);
        assert_eq!(
            msg.cnt().unwrap_err().to_string(),
            "BAD_SUBFIELD_VALUE: Subfield cannot have items"
        );
        assert_eq!(
            msg.type1().unwrap().get("CNT").unwrap(),
            Value::from(vec![""])
        );
    }

    #[test]
    fn replaced_type1() {
        let mut msg = Message::new();
        *msg.record_mut(0).unwrap() = Record::from(BinaryRecord::new(4));
        assert!(msg.type1().is_none());
        assert_eq!(
            msg.tot().unwrap_err().to_string(),
            "'BinaryRecord' object has no attribute 'TOT'"
        );
        assert_eq!(
            msg.set_tot("x").unwrap_err().to_string(),
            "'BinaryRecord' object has no attribute 'TOT'"
        );
        assert_eq!(
            msg.cnt().unwrap_err().to_string(),
            "'BinaryRecord' object is not subscriptable"
        );
    }

    #[test]
    fn positions() {
        let mut msg = Message::new();
        let e = msg.remove(3).unwrap_err();
        assert_eq!(e.position().record_index, Some(3));
        let e = msg.push(AsciiRecord::new(1).unwrap()).unwrap_err();
        assert_eq!(e.position().record_index, Some(1));
        assert_eq!(e.position().record_type, Some(1));
        let mut r = BinaryRecord::new(4);
        r.pack("!H", &[1.into()], b"").unwrap();
        r.set_value(b"");
        msg.push(r).unwrap();
        let e = msg.summary().unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::Struct);
        assert_eq!(e.position().record_index, Some(1));
        assert_eq!(e.position().record_type, Some(4));
    }
}
