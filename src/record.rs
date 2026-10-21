//! Tagged records (SPEC §8).

use std::fmt;

use crate::alias;
use crate::bytes::Bytes;
use crate::defaults::Defaults;
use crate::error::{Error, Position, Result};
use crate::field::{AnyField, BinaryField, Field};
use crate::py::{pad_int, pad_str, py_int, thousands};
use crate::value::{PyInt, Value};
use crate::{FS, GS};

/// Key of [`AsciiRecord::field`]: a tag number or an alias.
///
/// ```
/// use ansi_nist_itl::{FieldKey, PyInt};
///
/// assert_eq!(FieldKey::from(3), FieldKey::Tag(PyInt::from(3)));
/// assert_eq!(FieldKey::from("IDC"), FieldKey::Alias("IDC".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldKey {
    /// Tag number.
    Tag(PyInt),
    /// Alias; an empty alias matches no field.
    Alias(String),
}

impl From<PyInt> for FieldKey {
    fn from(tag: PyInt) -> Self {
        FieldKey::Tag(tag)
    }
}

impl From<&PyInt> for FieldKey {
    fn from(tag: &PyInt) -> Self {
        FieldKey::Tag(tag.clone())
    }
}

macro_rules! tag_key {
    ($($t:ty),*) => {
        $(
            impl From<$t> for FieldKey {
                fn from(tag: $t) -> Self {
                    FieldKey::Tag(PyInt::from(tag))
                }
            }
        )*
    };
}

tag_key!(i32, u32, i64, u64);

impl From<&str> for FieldKey {
    fn from(alias: &str) -> Self {
        FieldKey::Alias(alias.to_owned())
    }
}

impl From<String> for FieldKey {
    fn from(alias: String) -> Self {
        FieldKey::Alias(alias)
    }
}

/// Value assigned by [`AsciiRecord::set`] or [`AsciiRecord::set_bytes`].
enum Assigned {
    Text(Value),
    Bytes(Bytes),
}

impl Assigned {
    fn text(self, record_type: u32, tag: &PyInt) -> Result<Value> {
        match self {
            Assigned::Text(v) => Ok(v),
            Assigned::Bytes(_) => {
                Err(Error::text_field_value(tag).with_position(field_position(record_type, tag)))
            }
        }
    }

    fn bytes(self, record_type: u32, tag: &PyInt) -> Result<Bytes> {
        match self {
            Assigned::Bytes(b) => Ok(b),
            Assigned::Text(_) => {
                Err(Error::binary_field_value(tag).with_position(field_position(record_type, tag)))
            }
        }
    }

    /// `field.value = v`.
    fn assign_to(self, field: &mut AnyField) -> Result<()> {
        match field {
            AnyField::Text(f) => f.set_value(self.text(f.record(), f.tag())?),
            AnyField::Binary(f) => {
                f.set_value(self.bytes(f.record(), f.tag())?);
                Ok(())
            }
        }
    }
}

/// Position of an error on the field `tag` of a record.
fn field_position(record_type: u32, tag: &PyInt) -> Position {
    Position {
        record_type: Some(record_type),
        tag: Some(tag.clone()),
        ..Position::default()
    }
}

/// `tag` as a key of the alias table, if it fits.
fn table_tag(tag: &PyInt) -> Option<u32> {
    tag.to_i64().and_then(|t| u32::try_from(t).ok())
}

/// Number of a `_N` attribute name: `int(name[1:])`.
fn underscore_tag(name: &str) -> Option<PyInt> {
    name.strip_prefix('_').and_then(|n| py_int(n).ok())
}

/// One line of the summary of a record (SPEC §8.9).
fn summary_line(field: &AnyField) -> String {
    let value = match field {
        AnyField::Text(f) => f.value().to_string(),
        AnyField::Binary(f) => {
            let size = u64::try_from(f.value().len()).unwrap_or(u64::MAX);
            format!("<buffer, size={}>", thousands(size))
        }
    };
    format!(
        "{:2}.{}: {}: {value}",
        field.record(),
        pad_int(field.tag(), 3, true),
        pad_str(field.alias(), 30),
    )
}

/// Tagged record (`AsciiRecord`): Types 1, 2, 9 to 22, 98 and 99.
///
/// Fields are text ([`Field`]) or binary ([`BinaryField`]), each identified
/// by its tag number and, optionally, an alias taken from the table of
/// `nistitl` when the field is added.
///
/// ```
/// use ansi_nist_itl::{AsciiRecord, Value};
///
/// let mut r = AsciiRecord::new(10)?;
/// r.set("IDC", 2)?;
/// r.set_bytes("_999", b"my image data")?;
/// assert_eq!(r.get("IDC")?, Value::from(2));
/// assert_eq!(r.to_bytes()?, b"10.001:40\x1d10.002:2\x1d10.999:my image data\x1c");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsciiRecord {
    record_type: u32,
    fields: Vec<AnyField>,
    autosort: bool,
}

impl AsciiRecord {
    /// `AsciiRecord(t)`: a record whose mandatory fields are created, and
    /// whose text fields are written sorted by tag.
    ///
    /// The fields created are LEN and IDC, both `0`, or for a Type-1 LEN and
    /// the fields 2 to 5, 7 to 9, 11 and 12, with today's date and a new
    /// transaction control number. A type absent from the alias table gives
    /// an `UnknownRecordType` error.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let r = AsciiRecord::new(2)?;
    /// assert_eq!(r.to_string(), " 2.001: LEN                           : 0\n 2.002: IDC                           : 0");
    ///
    /// assert_eq!(AsciiRecord::new(100).unwrap_err().to_string(), "100");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn new(record_type: u32) -> Result<Self> {
        AsciiRecord::with(record_type, true, true)
    }

    /// `AsciiRecord(t, autocreate, autosort)`.
    ///
    /// Without `autocreate`, the record is empty, and an unknown type only
    /// fails when a field without alias is added. Without `autosort`, text
    /// fields are written in insertion order.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let r = AsciiRecord::with(100, false, true)?;
    /// assert!(r.fields().is_empty());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn with(record_type: u32, autocreate: bool, autosort: bool) -> Result<Self> {
        AsciiRecord::with_defaults(record_type, autocreate, autosort, &Defaults::default())
    }

    /// [`AsciiRecord::with`], taking the date and the transaction control
    /// number of a Type-1 from `defaults` (SPEC §14.4).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Defaults};
    ///
    /// let defaults = Defaults {
    ///     date: Some("20200904".into()),
    ///     tcn: Some("TCN".into()),
    /// };
    /// let mut r = AsciiRecord::with_defaults(1, true, true, &defaults)?;
    /// assert_eq!(
    ///     r.to_bytes()?,
    ///     b"1.001:104\x1d1.002:0400\x1d1.003:\x1d1.004:\x1d1.005:20200904\x1d\
    ///       1.007:000\x1d1.008:000\x1d1.009:TCN\x1d1.011:00.00\x1d1.012:00.00\x1c"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn with_defaults(
        record_type: u32,
        autocreate: bool,
        autosort: bool,
        defaults: &Defaults,
    ) -> Result<Self> {
        let mut record = AsciiRecord {
            record_type,
            fields: Vec::new(),
            autosort,
        };
        if !autocreate {
            return Ok(record);
        }
        record.add_field(Field::new(record_type, 1, 0)?)?;
        if record_type == 1 {
            let date = defaults.date();
            let tcn = defaults.tcn();
            let fields = [
                (2, "0400"),
                (3, ""),
                (4, ""),
                (5, date.as_str()),
                (7, "000"),
                (8, "000"),
                (9, tcn.as_str()),
                (11, "00.00"),
                (12, "00.00"),
            ];
            for (tag, value) in fields {
                record.add_field(Field::new(1, tag, value)?)?;
            }
        } else {
            record.add_field(Field::new(record_type, 2, 0)?)?;
        }
        Ok(record)
    }

    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// assert_eq!(AsciiRecord::new(14)?.record_type(), 14);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record_type(&self) -> u32 {
        self.record_type
    }

    /// `true` if text fields are written sorted by tag.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// assert!(AsciiRecord::new(2)?.autosort());
    /// assert!(!AsciiRecord::with(2, true, false)?.autosort());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn autosort(&self) -> bool {
        self.autosort
    }

    /// Sets whether text fields are written sorted by tag.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Field};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.add_field(Field::new(2, 5, "b")?)?.add_field(Field::new(2, 3, "a")?)?;
    /// r.set_autosort(false);
    /// assert_eq!(r.to_bytes()?, b"2.001:33\x1d2.002:0\x1d2.005:b\x1d2.003:a\x1c");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_autosort(&mut self, autosort: bool) {
        self.autosort = autosort;
    }

    /// Fields, in insertion order.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let tags: Vec<String> = AsciiRecord::new(2)?
    ///     .fields()
    ///     .iter()
    ///     .map(|f| f.tag().to_string())
    ///     .collect();
    /// assert_eq!(tags, ["1", "2"]);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn fields(&self) -> &[AnyField] {
        &self.fields
    }

    /// Adds a field (`record += field`), after these checks, in this order:
    ///
    /// 1. a field without alias takes the alias of its tag in the table of
    ///    the record type, which must exist (`UnknownRecordType`);
    /// 2. the field must belong to this record type (`BAD_RECORD_NUMBER`);
    /// 3. for each field already there, in order: same tag
    ///    (`BAD_TAG_DUPLICATE`), or else same non-empty alias
    ///    (`BAD_ALIAS_DUPLICATE`).
    ///
    /// Returns the record, so that calls can be chained.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Field};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.add_field(Field::new(2, 3, "OK")?)?;
    /// assert_eq!(
    ///     r.to_string(),
    ///     " 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : OK"
    /// );
    ///
    /// let err = r.add_field(Field::new(2, 3, "again")?).unwrap_err();
    /// assert_eq!(err.to_string(), "BAD_TAG_DUPLICATE: Tag 3 already defined in record 2");
    /// let err = r.add_field(Field::new(9, 4, "")?).unwrap_err();
    /// assert_eq!(err.to_string(), "BAD_RECORD_NUMBER: Bad record number 9 for tag 4 in record 2");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn add_field(&mut self, field: impl Into<AnyField>) -> Result<&mut Self> {
        let mut field = field.into();
        if field.alias().is_empty() {
            let table = self.alias_table()?;
            if let Some(alias) = table_tag(field.tag()).and_then(|t| alias::alias_of(table, t)) {
                field.set_alias(alias);
            }
        }
        if field.record() != self.record_type {
            return Err(
                Error::bad_record_number(field.record(), field.tag(), self.record_type)
                    .with_position(self.field_position(field.tag())),
            );
        }
        for other in &self.fields {
            if other.tag() == field.tag() {
                return Err(Error::tag_duplicate(field.tag(), self.record_type)
                    .with_position(self.field_position(field.tag())));
            }
            if !field.alias().is_empty() && other.alias() == field.alias() {
                return Err(Error::alias_duplicate(field.alias(), self.record_type)
                    .with_position(self.field_position(field.tag())));
            }
        }
        self.fields.push(field);
        Ok(self)
    }

    /// First field whose tag, or whose non-empty alias, is `key`
    /// (`record[key]`).
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Field, Value};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.add_field(Field::with(2, 3, "OK", "FSI", "%d.%03d:", "test")?)?;
    /// assert_eq!(r.field(3).unwrap().tag().to_string(), "3");
    /// assert_eq!(r.field("test").unwrap().alias(), "test");
    /// assert!(r.field(4).is_none());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn field(&self, key: impl Into<FieldKey>) -> Option<&AnyField> {
        let key = key.into();
        self.fields.iter().find(|f| Self::matches(f, &key))
    }

    /// Mutable access to the first field matching `key`, as
    /// [`AsciiRecord::field`].
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, AsciiRecord, Value};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// if let Some(AnyField::Text(f)) = r.field_mut("IDC") {
    ///     f.set_value(7)?;
    /// }
    /// assert_eq!(r.get("IDC")?, Value::from(7));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn field_mut(&mut self, key: impl Into<FieldKey>) -> Option<&mut AnyField> {
        let key = key.into();
        self.fields.iter_mut().find(|f| Self::matches(f, &key))
    }

    fn matches(field: &AnyField, key: &FieldKey) -> bool {
        match key {
            FieldKey::Tag(tag) => field.tag() == tag,
            FieldKey::Alias(alias) => !alias.is_empty() && field.alias() == alias,
        }
    }

    /// Removes the first field with this tag number (`del record[tag]`), and
    /// returns it; nothing happens if there is none.
    ///
    /// `nistitl` compares the key with the tag only: an alias never removes
    /// anything, so only a number is accepted here.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Field};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.add_field(Field::new(2, 3, "OK")?)?;
    /// assert!(r.remove_field(3).is_some());
    /// assert!(r.remove_field(4).is_none());
    /// assert_eq!(r.fields().len(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    ///
    /// ```compile_fail
    /// # use ansi_nist_itl::AsciiRecord;
    /// let mut r = AsciiRecord::new(2)?;
    /// r.remove_field("IDC"); // `del r['IDC']` has no effect in `nistitl`
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn remove_field(&mut self, tag: impl Into<PyInt>) -> Option<AnyField> {
        let tag = tag.into();
        let index = self.fields.iter().position(|f| f.tag() == &tag)?;
        Some(self.fields.remove(index))
    }

    /// Sets a field by name (`record.name = value`).
    ///
    /// In this order:
    ///
    /// 1. the first field whose alias is `name` takes the value;
    /// 2. the alias table of the type is required (`UnknownRecordType`);
    /// 3. `DATA` adds a binary field (see [`AsciiRecord::set_bytes`]);
    /// 4. an alias of the table adds the field of its tag, with this alias;
    /// 5. `_N`, where `N` is read by Python's `int()`, sets the field of tag
    ///    `N`, or adds it;
    /// 6. anything else gives `UNKNOWN_ATTRIBUTE`.
    ///
    /// A binary field only takes bytes: setting it gives a `TypeMismatch`
    /// error.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let mut r = AsciiRecord::new(10)?;
    /// r.set("IDC", 4)?;           // known
    /// r.set("_2", 5)?;            // known, by the tag number
    /// r.set("SRC", "my src")?;    // new one, predeclared
    /// r.set("_7", "my vll")?;     // by the tag number
    /// r.set("_345", "no alias")?;
    /// assert_eq!(r.to_string(), "\
    /// 10.001: LEN                           : 0
    /// 10.002: IDC                           : 5
    /// 10.004: SRC                           : my src
    /// 10.007: VLL                           : my vll
    /// 10.345:                               : no alias");
    ///
    /// let err = r.set("UUU", "unknown alias").unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "UNKNOWN_ATTRIBUTE: Bad attribute name <UUU> while trying to define a field in record of type 10"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set(&mut self, name: &str, value: impl Into<Value>) -> Result<()> {
        self.assign(name, Assigned::Text(value.into()))
    }

    /// Sets a binary field by name, following the steps of
    /// [`AsciiRecord::set`]; `DATA` and `_N` add a [`BinaryField`] when the
    /// tag is the `DATA` of the table.
    ///
    /// `DATA` on a type without it gives the `TypeError` of `nistitl`, and a
    /// text field only takes text (`TypeMismatch`).
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let mut r = AsciiRecord::new(10)?;
    /// r.set_bytes("DATA", b"data")?;
    /// assert_eq!(r.get_bytes("_999")?, b"data");
    ///
    /// let err = AsciiRecord::new(2)?.set_bytes("DATA", b"x").unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_bytes(&mut self, name: &str, value: impl Into<Bytes>) -> Result<()> {
        self.assign(name, Assigned::Bytes(value.into()))
    }

    fn assign(&mut self, name: &str, value: Assigned) -> Result<()> {
        if let Some(field) = self.fields.iter_mut().find(|f| f.alias() == name) {
            return value.assign_to(field);
        }
        let record_type = self.record_type;
        let table = self.alias_table()?;
        let tag = alias::tag_of(table, name);
        if name == alias::DATA {
            let Some(tag) = tag else {
                return Err(Error::int_argument("NoneType").with_position(self.record_position()));
            };
            let tag = PyInt::from(tag);
            let mut field =
                BinaryField::new(record_type, tag.clone(), value.bytes(record_type, &tag)?)?;
            field.set_alias(name);
            self.add_field(field)?;
            return Ok(());
        }
        if let Some(tag) = tag {
            let tag = PyInt::from(tag);
            let mut field = Field::new(record_type, tag.clone(), value.text(record_type, &tag)?)?;
            field.set_alias(name);
            self.add_field(field)?;
            return Ok(());
        }
        let Some(tag) = underscore_tag(name) else {
            return Err(
                Error::bad_attribute_set(name, record_type).with_position(self.record_position())
            );
        };
        if let Some(field) = self.fields.iter_mut().find(|f| f.tag() == &tag) {
            return value.assign_to(field);
        }
        if table_tag(&tag).and_then(|t| alias::alias_of(table, t)) == Some(alias::DATA) {
            let bytes = value.bytes(record_type, &tag)?;
            self.add_field(BinaryField::new(record_type, tag, bytes)?)?;
        } else {
            let text = value.text(record_type, &tag)?;
            self.add_field(Field::new(record_type, tag, text)?)?;
        }
        Ok(())
    }

    /// Value of a field by name (`record.name`): the first field whose alias
    /// is `name`, or else the field of tag `N` for `_N`.
    ///
    /// Other names, and fields not created yet, give `UNKNOWN_ATTRIBUTE`. A
    /// binary field gives `TypeMismatch`: use [`AsciiRecord::get_bytes`].
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Value};
    ///
    /// let mut r = AsciiRecord::new(10)?;
    /// r.set("IDC", 4)?;
    /// r.set("SRC", "my src")?;
    /// assert_eq!(r.get("IDC")?, Value::from(4));
    /// assert_eq!(r.get("_4")?, Value::from("my src"));
    ///
    /// let err = r.get("_76a").unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <_76a> while trying to retrieve a field in record of type 10"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn get(&self, name: &str) -> Result<Value> {
        match self.lookup(name)? {
            AnyField::Text(f) => Ok(f.value()),
            AnyField::Binary(f) => {
                Err(Error::binary_field_value(f.tag()).with_position(self.field_position(f.tag())))
            }
        }
    }

    /// Value of a binary field by name, found as by [`AsciiRecord::get`]. A
    /// text field gives `TypeMismatch`.
    ///
    /// ```
    /// use ansi_nist_itl::AsciiRecord;
    ///
    /// let mut r = AsciiRecord::new(14)?;
    /// r.set_bytes("_999", b"img")?;
    /// assert_eq!(r.get_bytes("DATA")?, b"img");
    /// assert!(r.get_bytes("IDC").is_err());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn get_bytes(&self, name: &str) -> Result<&[u8]> {
        match self.lookup(name)? {
            AnyField::Binary(f) => Ok(f.value()),
            AnyField::Text(f) => {
                Err(Error::text_field_value(f.tag()).with_position(self.field_position(f.tag())))
            }
        }
    }

    fn lookup(&self, name: &str) -> Result<&AnyField> {
        let by_alias = self.fields.iter().find(|f| f.alias() == name);
        let by_tag = || {
            let tag = underscore_tag(name)?;
            self.fields.iter().find(|f| f.tag() == &tag)
        };
        by_alias.or_else(by_tag).ok_or_else(|| {
            Error::bad_attribute_get(name, self.record_type).with_position(self.record_position())
        })
    }

    /// Fields from index `start` in write order: text fields, sorted by tag
    /// (stable sort) if [`autosort`](AsciiRecord::autosort), then binary
    /// fields in insertion order.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, BinaryField, Field};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.add_field(BinaryField::new(2, 999, b"B")?)?
    ///     .add_field(Field::new(2, 5, "z")?)?
    ///     .add_field(BinaryField::new(2, 998, b"A")?)?;
    /// let tags: Vec<String> = r.sorted_fields(1).iter().map(|f| f.tag().to_string()).collect();
    /// assert_eq!(tags, ["2", "5", "999", "998"]);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn sorted_fields(&self, start: usize) -> Vec<&AnyField> {
        let rest = self.fields.get(start..).unwrap_or_default();
        let (mut text, binary): (Vec<&AnyField>, Vec<&AnyField>) =
            rest.iter().partition(|f| matches!(f, AnyField::Text(_)));
        if self.autosort {
            text.sort_by(|a, b| a.tag().cmp(b.tag()));
        }
        text.extend(binary);
        text
    }

    /// Bytes of the record (`NIST`): the first field, then the others in the
    /// order of [`AsciiRecord::sorted_fields`], separated by GS and ended by
    /// FS.
    ///
    /// The first field, whatever its tag, takes the length of the record:
    /// it keeps this value afterwards. An empty record gives
    /// `IndexOutOfRange`, a binary first field `TypeMismatch`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, Value};
    ///
    /// let mut r = AsciiRecord::new(2)?;
    /// r.set("_3", "x".repeat(80))?;
    /// assert_eq!(&r.to_bytes()?[..10], b"2.001:105\x1d");
    /// assert_eq!(r.get("LEN")?, Value::from(105));
    ///
    /// let err = AsciiRecord::with(2, false, true)?.to_bytes().unwrap_err();
    /// assert_eq!(err.to_string(), "list index out of range");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        let mut rest = Vec::new();
        for (i, field) in self.sorted_fields(1).into_iter().enumerate() {
            if i > 0 {
                rest.push(GS);
            }
            rest.extend(field.to_bytes()?);
        }
        let position = self.record_position();
        let first = match self.fields.first_mut() {
            None => return Err(Error::index_out_of_range().with_position(position)),
            Some(AnyField::Binary(f)) => {
                let position = Position {
                    tag: Some(f.tag().clone()),
                    ..position
                };
                return Err(Error::concat_int_to_bytes().with_position(position));
            }
            Some(AnyField::Text(f)) => f,
        };
        first.set_value(0)?;
        let mut last_len = None;
        loop {
            let mut out = first.to_bytes()?;
            out.push(GS);
            out.extend_from_slice(&rest);
            let len = u64::try_from(out.len()).unwrap_or(u64::MAX);
            first.set_value(len.saturating_add(1))?;
            if last_len == Some(out.len()) {
                out.push(FS);
                return Ok(out);
            }
            last_len = Some(out.len());
        }
    }

    /// Summary of the record (`str(record)`): one line per field, in the
    /// order of [`AsciiRecord::sorted_fields`], with the tag, the alias and
    /// the value, or the size of a binary value. Same text as `Display`.
    ///
    /// ```
    /// use ansi_nist_itl::{AsciiRecord, BinaryField, Field};
    ///
    /// let mut r = AsciiRecord::new(14)?;
    /// r.add_field(Field::new(14, 3, "OK")?)?;
    /// r.add_field(BinaryField::new(14, 999, b"data")?)?;
    /// assert_eq!(r.summary(), "\
    /// 14.001: LEN                           : 0
    /// 14.002: IDC                           : 0
    /// 14.003: IMP                           : OK
    /// 14.999: DATA                          : <buffer, size=4>");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn summary(&self) -> String {
        let lines: Vec<String> = self
            .sorted_fields(0)
            .into_iter()
            .map(summary_line)
            .collect();
        lines.join("\n")
    }

    /// `ALIASES[type]`.
    fn alias_table(&self) -> Result<&'static [(u32, &'static str)]> {
        alias::table(self.record_type).ok_or_else(|| {
            Error::unknown_record_type(PyInt::from(self.record_type))
                .with_position(self.record_position())
        })
    }

    fn record_position(&self) -> Position {
        Position {
            record_type: Some(self.record_type),
            ..Position::default()
        }
    }

    fn field_position(&self, tag: &PyInt) -> Position {
        field_position(self.record_type, tag)
    }
}

impl fmt::Display for AsciiRecord {
    /// [`AsciiRecord::summary`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorKind, NistError};

    fn defaults() -> Defaults {
        Defaults {
            date: Some("20200904".into()),
            tcn: Some("TCN".into()),
        }
    }

    #[test]
    fn type1_autocreated_fields() {
        let r = AsciiRecord::with_defaults(1, true, true, &defaults()).unwrap();
        let tags: Vec<String> = r.fields().iter().map(|f| f.tag().to_string()).collect();
        assert_eq!(tags, ["1", "2", "3", "4", "5", "7", "8", "9", "11", "12"]);
        let aliases: Vec<&str> = r.fields().iter().map(AnyField::alias).collect();
        assert_eq!(
            aliases,
            [
                "LEN", "VER", "CNT", "TOT", "DAT", "DAI", "ORI", "TCN", "NSR", "NTR"
            ]
        );
        assert_eq!(r.get("VER").unwrap(), Value::from("0400"));
        assert_eq!(r.get("LEN").unwrap(), Value::from(0));
        assert_eq!(
            AsciiRecord::new(1)
                .unwrap()
                .get("DAT")
                .unwrap()
                .to_string()
                .len(),
            8
        );
    }

    #[test]
    fn unknown_type_checked_first() {
        // `ALIASES[type]` raises before the record number is compared.
        let mut r = AsciiRecord::with(50, false, true).unwrap();
        let e = r.add_field(Field::new(2, 3, "x").unwrap()).unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::UnknownRecordType(PyInt::from(50)));
        assert_eq!(e.position().record_type, Some(50));
        // A field with an alias does not need the table.
        let f = Field::with(50, 3, "x", "FSI", "%d.%03d:", "A").unwrap();
        r.add_field(f).unwrap();
        assert_eq!(r.to_bytes().unwrap(), b"50.003:11\x1d\x1c");
        r.set("A", "y").unwrap();
        assert_eq!(
            r.set("B", "y").unwrap_err().kind(),
            &ErrorKind::UnknownRecordType(PyInt::from(50))
        );
    }

    #[test]
    fn alias_duplicate() {
        let mut r = AsciiRecord::new(2).unwrap();
        let f = Field::with(2, 5, "x", "FSI", "%d.%03d:", "IDC").unwrap();
        let e = r.add_field(f).unwrap_err();
        assert_eq!(e.nist_error(), Some(NistError::BadAliasDuplicate));
        assert_eq!(
            e.to_string(),
            "BAD_ALIAS_DUPLICATE: Alias 'IDC' already defined in record 2"
        );
        assert_eq!(e.position().record_type, Some(2));
        assert_eq!(e.position().tag, Some(PyInt::from(5)));
        // Fields without alias never clash on it.
        r.add_field(Field::new(2, 5, "x").unwrap()).unwrap();
        r.add_field(Field::new(2, 6, "y").unwrap()).unwrap();
    }

    #[test]
    fn empty_name_matches_empty_alias() {
        // `setattr(r, '', v)` and `getattr(r, '')` find the first field
        // without alias; `r['']` finds nothing.
        let mut r = AsciiRecord::new(2).unwrap();
        r.add_field(Field::new(2, 3, "x").unwrap()).unwrap();
        r.set("", "v").unwrap();
        assert_eq!(r.get("").unwrap(), Value::from("v"));
        assert!(r.field("").is_none());
    }

    #[test]
    fn bytes_and_text_do_not_mix() {
        let mut r = AsciiRecord::new(10).unwrap();
        r.set_bytes("DATA", b"img").unwrap();
        let e = r.set("DATA", "text").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
        assert_eq!(e.position().tag, Some(PyInt::from(999)));
        assert_eq!(r.get("DATA").unwrap_err().kind(), &ErrorKind::TypeMismatch);
        assert_eq!(
            r.set_bytes("IDC", b"1").unwrap_err().kind(),
            &ErrorKind::TypeMismatch
        );
        assert_eq!(
            r.get_bytes("IDC").unwrap_err().kind(),
            &ErrorKind::TypeMismatch
        );
        // New fields: through an alias, a tag number, or DATA.
        let mut r = AsciiRecord::new(10).unwrap();
        assert!(r.set_bytes("SRC", b"x").is_err());
        assert!(r.set_bytes("_5", b"x").is_err());
        assert!(r.set("_999", "x").is_err());
        assert!(r.set("DATA", "x").is_err());
        assert_eq!(r.fields().len(), 2);
        // Type-2 has no DATA: `int(None)` fails first, whatever the value.
        let mut r = AsciiRecord::new(2).unwrap();
        let e = r.set("DATA", "x").unwrap_err();
        assert!(e.to_string().starts_with("int() argument"));
        // ... and `_999` is a text field there.
        r.set("_999", "abc").unwrap();
        assert!(matches!(r.field(999), Some(AnyField::Text(_))));
    }

    #[test]
    fn digits_limit_is_an_unknown_attribute() {
        // `int()` raises `ValueError` for both, caught by `nistitl`.
        let mut r = AsciiRecord::new(2).unwrap();
        let name = format!("_{}", "1".repeat(4301));
        let e = r.set(&name, "x").unwrap_err();
        assert_eq!(e.nist_error(), Some(NistError::UnknownAttribute));
        assert_eq!(
            r.get(&name).unwrap_err().nist_error(),
            Some(NistError::UnknownAttribute)
        );
    }

    #[test]
    fn first_field_takes_the_length() {
        // Its kind still applies: a field without `F` cannot hold it.
        let mut r = AsciiRecord::with(2, false, true).unwrap();
        r.add_field(Field::with(2, 1, "", "SI", "%d.%03d:", "").unwrap())
            .unwrap();
        assert_eq!(
            r.to_bytes().unwrap_err().to_string(),
            "BAD_FIELD_VALUE: Field 2.001: cannot have a value (only subfields and/or items)"
        );
        // A binary first field cannot.
        let mut r = AsciiRecord::with(2, false, true).unwrap();
        r.add_field(BinaryField::new(2, 1, b"").unwrap()).unwrap();
        let e = r.to_bytes().unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
        assert_eq!(e.to_string(), "can't concat int to bytes");
        assert_eq!(e.position().tag, Some(PyInt::from(1)));
        // Written twice, the same bytes.
        let mut r = AsciiRecord::new(2).unwrap();
        assert_eq!(r.to_bytes().unwrap(), r.to_bytes().unwrap());
    }

    #[test]
    fn summary_tags() {
        let mut r = AsciiRecord::new(2).unwrap();
        r.add_field(Field::new(2, 3, "x").unwrap()).unwrap();
        r.add_field(Field::new(2, -1, "n").unwrap()).unwrap();
        r.add_field(Field::new(2, 1000, "m").unwrap()).unwrap();
        assert_eq!(
            r.summary(),
            " 2.-01:                               : n\n 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : x\n 2.1000:                               : m"
        );
        assert_eq!(r.to_string(), r.summary());
        assert_eq!(AsciiRecord::with(2, false, true).unwrap().summary(), "");
    }

    #[test]
    fn sorted_fields_bounds() {
        let r = AsciiRecord::new(2).unwrap();
        assert_eq!(r.sorted_fields(2).len(), 0);
        assert_eq!(r.sorted_fields(usize::MAX).len(), 0);
    }
}
