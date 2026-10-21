//! Text and binary fields of a tagged record (SPEC §6, §7).

use crate::RS;
use crate::bytes::Bytes;
use crate::error::{Error, Position, Result};
use crate::py::{PrintfTemplate, encode_latin1, py_int};
use crate::subfield::SubField;
use crate::value::{Kind, ListElem, PyInt, Scalar, SubValue, Value};

/// Default tag format of `nistitl`: `'%d.%03d:'`.
const DEFAULT_FORMAT: &str = "%d.%03d:";

/// `int(tag)`: the tag given as a number or as text.
fn tag_number(tag: Scalar, record: u32) -> Result<PyInt> {
    let tag = match tag {
        Scalar::Int(n) => Ok(n),
        Scalar::Bool(b) => Ok(PyInt::from(u32::from(b))),
        Scalar::Text(s) => py_int(&s).map_err(Error::from),
        Scalar::Null => Err(Error::int_argument("NoneType")),
    };
    tag.map_err(|e| {
        e.with_position(Position {
            record_type: Some(record),
            ..Position::default()
        })
    })
}

/// Parses a tag format template (SPEC §3.5).
fn tag_format(format: &str, record: u32, tag: &PyInt) -> Result<PrintfTemplate> {
    PrintfTemplate::parse(format)
        .map_err(|e| Error::from(e).with_position(field_position(record, tag)))
}

/// Position of an error raised by a field.
fn field_position(record: u32, tag: &PyInt) -> Position {
    Position {
        record_type: Some(record),
        tag: Some(tag.clone()),
        ..Position::default()
    }
}

/// Text field of a tagged record: a scalar value, or subfields separated by
/// RS (`0x1E`) when written.
///
/// The [`Kind`] restricts the content: `F` allows a non-empty scalar value,
/// `S` subfields with a scalar value, `I` subfields with items (SPEC §6).
/// The kind of the field is given to all its subfields.
///
/// ```
/// use ansi_nist_itl::Field;
///
/// let f = Field::new(2, 3, vec![vec!["a", "b"], vec!["c"]])?;
/// assert_eq!(f.len(), 2);
/// assert_eq!(f.to_bytes()?, b"2.003:a\x1fb\x1ec");
/// assert_eq!(f.value().to_string(), "[['a', 'b'], ['c']]");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    record: u32,
    tag: PyInt,
    kind: Kind,
    format: PrintfTemplate,
    alias: String,
    subfields: Vec<SubField>,
    value: Scalar,
}

impl Field {
    /// `Field(record, tag, value)`: a field of kind `FSI`, with the format
    /// `%d.%03d:` and no alias.
    ///
    /// `tag` goes through Python's `int()`: a number, or text such as `"007"`.
    /// Invalid text gives an `InvalidLiteral` error, `None` a `TypeMismatch`.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// let f = Field::new(2, " 7", "x")?;
    /// assert_eq!(f.to_bytes()?, b"2.007:x");
    ///
    /// let err = Field::new(2, "\x1c3", "x").unwrap_err();
    /// assert_eq!(err.to_string(), "invalid literal for int() with base 10: '\\x1c3'");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn new(record: u32, tag: impl Into<Scalar>, value: impl Into<Value>) -> Result<Self> {
        Field::with(record, tag, value, Kind::FSI, DEFAULT_FORMAT, "")
    }

    /// `Field(record, tag, value, type, format, alias)`.
    ///
    /// `format` is a `printf` template applied to `(record, tag)`, made of
    /// literal text and exactly two `%[0][width]d` conversions; any other
    /// template gives a `BadFormat` error. The value is then set as with
    /// [`Field::set_value`].
    ///
    /// ```
    /// use ansi_nist_itl::{Field, Kind};
    ///
    /// let f = Field::with(2, 3, "x", Kind::FSI, "%02d.%d:", "NAME")?;
    /// assert_eq!(f.to_bytes()?, b"02.3:x");
    /// assert_eq!(f.alias(), "NAME");
    ///
    /// let err = Field::with(2, 3, "x", "S", "%d.%03d:", "").unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)"
    /// );
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn with(
        record: u32,
        tag: impl Into<Scalar>,
        value: impl Into<Value>,
        kind: impl Into<Kind>,
        format: &str,
        alias: impl Into<String>,
    ) -> Result<Self> {
        let tag = tag_number(tag.into(), record)?;
        let format = tag_format(format, record, &tag)?;
        let mut field = Field {
            record,
            tag,
            kind: kind.into(),
            format,
            alias: alias.into(),
            subfields: Vec::new(),
            value: Scalar::default(),
        };
        field.set_value(value)?;
        Ok(field)
    }

    /// Value: the list of the subfield values if there are subfields, the
    /// scalar value otherwise.
    ///
    /// ```
    /// use ansi_nist_itl::{Field, Value};
    ///
    /// assert_eq!(Field::new(2, 3, "x")?.value(), Value::from("x"));
    /// assert_eq!(Field::new(2, 3, vec!["a", "b"])?.value(), Value::from(vec!["a", "b"]));
    /// assert_eq!(Field::new(2, 3, Vec::<&str>::new())?.value(), Value::from(""));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn value(&self) -> Value {
        if self.subfields.is_empty() {
            return Value::Scalar(self.value.clone());
        }
        let elems = self
            .subfields
            .iter()
            .map(|sf| match sf.value() {
                SubValue::Scalar(s) => ListElem::Scalar(s),
                SubValue::Items(items) => ListElem::List(items),
            })
            .collect();
        Value::List(elems)
    }

    /// Replaces the content with `value`.
    ///
    /// The content is cleared first. A scalar becomes the value of the
    /// field: if it is true, the kind needs `F`. A list needs `S` or `I`; each
    /// scalar element becomes a subfield value, each list element the items
    /// of a subfield, checked against the kind of the field. On error, the
    /// subfields added before the faulty element stay in place.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// let mut f = Field::with(2, 3, "", "S", "%d.%03d:", "")?;
    /// f.set_value(vec!["a", "b"])?;
    /// let err = f.set_value(vec![vec!["a", "b"]]).unwrap_err();
    /// assert_eq!(err.to_string(), "BAD_SUBFIELD_VALUE: Subfield cannot have items");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_value(&mut self, value: impl Into<Value>) -> Result<()> {
        self.set_value_inner(value.into())
            .map_err(|e| e.with_position(self.position()))
    }

    fn set_value_inner(&mut self, value: Value) -> Result<()> {
        self.value = Scalar::default();
        self.subfields.clear();
        let elems = match value {
            Value::Scalar(v) => {
                if v.is_truthy() && !self.kind.field_value() {
                    return Err(Error::field_cannot_have_value(&self.label()));
                }
                self.value = v;
                return Ok(());
            }
            Value::List(elems) => elems,
        };
        if !self.kind.subfields() && !self.kind.items() {
            return Err(Error::field_cannot_have_subfields(&self.label()));
        }
        for elem in elems {
            let sf = match elem {
                ListElem::Scalar(v) => SubField::new(v, self.kind)?,
                ListElem::List(items) => {
                    let mut sf = SubField::new("", self.kind)?;
                    sf.add_values(items)?;
                    sf
                }
            };
            self.add_subfields_inner([sf])?;
        }
        Ok(())
    }

    /// Appends `subfields`, after clearing the scalar value.
    ///
    /// A non-empty list needs `S` or `I` in the kind of the field. Each
    /// subfield is then checked in turn: items need `I`, a scalar value
    /// needs `S`. A subfield that passes takes the kind of the field; those
    /// added before a failing one stay in place.
    ///
    /// ```
    /// use ansi_nist_itl::{Field, SubField};
    ///
    /// let mut f = Field::with(2, 3, "", "FI", "%d.%03d:", "")?;
    /// let err = f.add_subfields([SubField::new("a", "SI")?]).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have a value: 'a'"
    /// );
    /// f.add_subfields([SubField::new(vec!["a", "b"], "SI")?])?;
    /// assert_eq!(f.to_bytes()?, b"2.003:a\x1fb");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn add_subfields(&mut self, subfields: impl IntoIterator<Item = SubField>) -> Result<()> {
        self.add_subfields_inner(subfields)
            .map_err(|e| e.with_position(self.position()))
    }

    fn add_subfields_inner(&mut self, subfields: impl IntoIterator<Item = SubField>) -> Result<()> {
        let mut subfields = subfields.into_iter().peekable();
        if subfields.peek().is_some() && !self.kind.subfields() && !self.kind.items() {
            return Err(Error::field_cannot_have_subfields(&self.label()));
        }
        self.value = Scalar::default();
        for mut sf in subfields {
            match sf.value() {
                SubValue::Items(_) if !self.kind.items() => {
                    return Err(Error::subfield_of_cannot_have_items(&self.label()));
                }
                SubValue::Scalar(v) if !self.kind.subfields() => {
                    return Err(Error::subfield_of_cannot_have_value(
                        &self.label(),
                        &v.repr(),
                    ));
                }
                _ => {}
            }
            sf.set_kind(self.kind);
            self.subfields.push(sf);
        }
        Ok(())
    }

    /// Clears the scalar value and the subfields.
    ///
    /// ```
    /// use ansi_nist_itl::{Field, Value};
    ///
    /// let mut f = Field::new(2, 3, vec!["a"])?;
    /// f.reset();
    /// assert!(f.is_empty());
    /// assert_eq!(f.value(), Value::from(""));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn reset(&mut self) {
        self.value = Scalar::default();
        self.subfields.clear();
    }

    /// Number of subfields.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert_eq!(Field::new(2, 3, vec!["one", "two"])?.len(), 2);
    /// assert_eq!(Field::new(2, 21, "hello")?.len(), 0);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.subfields.len()
    }

    /// `true` if the field has no subfield: Python's `not field`, whatever
    /// the scalar value.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert!(Field::new(2, 3, "x")?.is_empty());
    /// assert!(!Field::new(2, 3, vec![""])?.is_empty());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.subfields.is_empty()
    }

    /// Subfields.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// let f = Field::new(2, 3, vec!["one", "two"])?;
    /// assert_eq!(f.subfields().len(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn subfields(&self) -> &[SubField] {
        &self.subfields
    }

    /// Subfield at `index` (`IndexError` if out of range).
    ///
    /// ```
    /// use ansi_nist_itl::{Field, SubValue};
    ///
    /// let f = Field::new(2, 3, vec!["one", "two"])?;
    /// assert_eq!(f.subfield(1)?.value(), SubValue::from("two"));
    /// assert!(f.subfield(2).is_err());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn subfield(&self, index: usize) -> Result<&SubField> {
        self.subfields
            .get(index)
            .ok_or_else(|| Error::index_out_of_range().with_position(self.position()))
    }

    /// Mutable subfield at `index` (`IndexError` if out of range).
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// let mut f = Field::new(2, 3, vec!["one", "two"])?;
    /// f.subfield_mut(0)?.set_value("1")?;
    /// assert_eq!(f.to_bytes()?, b"2.003:1\x1etwo");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn subfield_mut(&mut self, index: usize) -> Result<&mut SubField> {
        let position = self.position();
        self.subfields
            .get_mut(index)
            .ok_or_else(|| Error::index_out_of_range().with_position(position))
    }

    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert_eq!(Field::new(2, 3, "")?.record(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record(&self) -> u32 {
        self.record
    }

    /// Tag number.
    ///
    /// ```
    /// use ansi_nist_itl::{Field, PyInt};
    ///
    /// assert_eq!(Field::new(2, "003", "")?.tag(), &PyInt::from(3u32));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn tag(&self) -> &PyInt {
        &self.tag
    }

    /// Kind of the field.
    ///
    /// ```
    /// use ansi_nist_itl::{Field, Kind};
    ///
    /// assert_eq!(Field::new(2, 3, "")?.kind(), Kind::FSI);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Tag format template.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert_eq!(Field::new(2, 3, "")?.format(), "%d.%03d:");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn format(&self) -> &str {
        self.format.as_str()
    }

    /// Alias (`""` if none).
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert_eq!(Field::new(2, 3, "")?.alias(), "");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Sets the alias.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// let mut f = Field::new(2, 2, "")?;
    /// f.set_alias("IDC");
    /// assert_eq!(f.alias(), "IDC");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_alias(&mut self, alias: impl Into<String>) {
        self.alias = alias.into();
    }

    /// Bytes of the field (`NIST`): the tag label, then the value.
    ///
    /// Without subfields, the scalar value as `str(value)` (nothing for
    /// `None`); otherwise the subfields separated by RS. A character outside
    /// Latin-1 gives an `Encoding` error.
    ///
    /// ```
    /// use ansi_nist_itl::Field;
    ///
    /// assert_eq!(Field::new(2, 2, ())?.to_bytes()?, b"2.002:");
    /// assert_eq!(Field::new(2, 3, true)?.to_bytes()?, b"2.003:True");
    /// assert_eq!(Field::new(2, 1000, "x")?.to_bytes()?, b"2.1000:x");
    /// assert!(Field::new(2, 3, "€")?.to_bytes().is_err());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.to_bytes_inner()
            .map_err(|e| e.with_position(self.position()))
    }

    fn to_bytes_inner(&self) -> Result<Vec<u8>> {
        let label = self.label();
        if self.subfields.is_empty() {
            let text = match &self.value {
                Scalar::Null => label,
                value => format!("{label}{value}"),
            };
            return Ok(encode_latin1(&text)?);
        }
        let mut out = encode_latin1(&label)?;
        for (i, sf) in self.subfields.iter().enumerate() {
            if i > 0 {
                out.push(RS);
            }
            out.extend(sf.to_bytes()?);
        }
        Ok(out)
    }

    /// `format % (record, tag)`, such as `2.003:`.
    fn label(&self) -> String {
        self.format.render(&PyInt::from(self.record), &self.tag)
    }

    fn position(&self) -> Position {
        field_position(self.record, &self.tag)
    }
}

/// Binary field of a tagged record, such as the image data `999`.
///
/// The value is kept as raw [`Bytes`], without copy nor check (SPEC §7).
///
/// ```
/// use ansi_nist_itl::BinaryField;
///
/// let f = BinaryField::new(14, 999, b"\xff\xa0")?;
/// assert_eq!(f.to_bytes()?, b"14.999:\xff\xa0");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryField {
    record: u32,
    tag: PyInt,
    value: Bytes,
    format: PrintfTemplate,
    alias: String,
}

impl BinaryField {
    /// `BinaryField(record, tag, value)`, with the format `%d.%03d:` and no
    /// alias.
    ///
    /// `tag` goes through Python's `int()`, as for [`Field::new`].
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// let f = BinaryField::new(10, "999", Vec::new())?;
    /// assert_eq!(f.to_bytes()?, b"10.999:");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn new(record: u32, tag: impl Into<Scalar>, value: impl Into<Bytes>) -> Result<Self> {
        BinaryField::with(record, tag, value, DEFAULT_FORMAT, "")
    }

    /// `BinaryField(record, tag, value, format, alias)`.
    ///
    /// `format` follows the rules of [`Field::with`].
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// let f = BinaryField::with(10, 999, b"x", "%d.%d:", "DATA")?;
    /// assert_eq!(f.to_bytes()?, b"10.999:x");
    /// assert_eq!(f.alias(), "DATA");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn with(
        record: u32,
        tag: impl Into<Scalar>,
        value: impl Into<Bytes>,
        format: &str,
        alias: impl Into<String>,
    ) -> Result<Self> {
        let tag = tag_number(tag.into(), record)?;
        let format = tag_format(format, record, &tag)?;
        Ok(BinaryField {
            record,
            tag,
            value: value.into(),
            format,
            alias: alias.into(),
        })
    }

    /// Value.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// assert_eq!(BinaryField::new(10, 999, b"abc")?.value(), b"abc");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// Value as a shared buffer, cloned without copying the bytes.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// let f = BinaryField::new(10, 999, b"abc")?;
    /// let data = f.bytes().clone();
    /// assert_eq!(&*data, b"abc");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn bytes(&self) -> &Bytes {
        &self.value
    }

    /// Replaces the value.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// let mut f = BinaryField::new(10, 999, Vec::new())?;
    /// f.set_value(b"new");
    /// assert_eq!(f.value(), b"new");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_value(&mut self, value: impl Into<Bytes>) {
        self.value = value.into();
    }

    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// assert_eq!(BinaryField::new(10, 999, b"")?.record(), 10);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record(&self) -> u32 {
        self.record
    }

    /// Tag number.
    ///
    /// ```
    /// use ansi_nist_itl::{BinaryField, PyInt};
    ///
    /// assert_eq!(BinaryField::new(10, 999, b"")?.tag(), &PyInt::from(999u32));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn tag(&self) -> &PyInt {
        &self.tag
    }

    /// Tag format template.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// assert_eq!(BinaryField::new(10, 999, b"")?.format(), "%d.%03d:");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn format(&self) -> &str {
        self.format.as_str()
    }

    /// Alias (`""` if none).
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// assert_eq!(BinaryField::new(10, 999, b"")?.alias(), "");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Sets the alias.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// let mut f = BinaryField::new(10, 999, b"")?;
    /// f.set_alias("DATA");
    /// assert_eq!(f.alias(), "DATA");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_alias(&mut self, alias: impl Into<String>) {
        self.alias = alias.into();
    }

    /// Bytes of the field (`NIST`): the tag label in Latin-1, then the value.
    ///
    /// ```
    /// use ansi_nist_itl::BinaryField;
    ///
    /// assert_eq!(BinaryField::new(2, 999, b"B")?.to_bytes()?, b"2.999:B");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let label = self.format.render(&PyInt::from(self.record), &self.tag);
        let mut out = encode_latin1(&label)
            .map_err(|e| Error::from(e).with_position(field_position(self.record, &self.tag)))?;
        out.extend_from_slice(&self.value);
        Ok(out)
    }
}

/// Field of a tagged record: text or binary.
///
/// ```
/// use ansi_nist_itl::{AnyField, BinaryField, Field};
///
/// let fields = [
///     AnyField::from(Field::new(14, 3, "x")?),
///     AnyField::from(BinaryField::new(14, 999, b"img")?),
/// ];
/// let tags: Vec<String> = fields.iter().map(|f| f.tag().to_string()).collect();
/// assert_eq!(tags, ["3", "999"]);
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyField {
    /// Text field.
    Text(Field),
    /// Binary field.
    Binary(BinaryField),
}

impl AnyField {
    /// Record type.
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, Field};
    ///
    /// assert_eq!(AnyField::from(Field::new(2, 3, "")?).record(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn record(&self) -> u32 {
        match self {
            AnyField::Text(f) => f.record(),
            AnyField::Binary(f) => f.record(),
        }
    }

    /// Tag number.
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, BinaryField, PyInt};
    ///
    /// let f = AnyField::from(BinaryField::new(10, 999, b"")?);
    /// assert_eq!(f.tag(), &PyInt::from(999u32));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn tag(&self) -> &PyInt {
        match self {
            AnyField::Text(f) => f.tag(),
            AnyField::Binary(f) => f.tag(),
        }
    }

    /// Tag format template.
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, Field};
    ///
    /// assert_eq!(AnyField::from(Field::new(2, 3, "")?).format(), "%d.%03d:");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn format(&self) -> &str {
        match self {
            AnyField::Text(f) => f.format(),
            AnyField::Binary(f) => f.format(),
        }
    }

    /// Alias (`""` if none).
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, BinaryField};
    ///
    /// let f = AnyField::from(BinaryField::with(10, 999, b"", "%d.%03d:", "DATA")?);
    /// assert_eq!(f.alias(), "DATA");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn alias(&self) -> &str {
        match self {
            AnyField::Text(f) => f.alias(),
            AnyField::Binary(f) => f.alias(),
        }
    }

    /// Sets the alias.
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, Field};
    ///
    /// let mut f = AnyField::from(Field::new(2, 2, "")?);
    /// f.set_alias("IDC");
    /// assert_eq!(f.alias(), "IDC");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_alias(&mut self, alias: impl Into<String>) {
        match self {
            AnyField::Text(f) => f.set_alias(alias),
            AnyField::Binary(f) => f.set_alias(alias),
        }
    }

    /// Python truthiness: a text field is true when it has subfields, a
    /// binary field is always true (it has no `__len__`).
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, BinaryField, Field};
    ///
    /// assert!(!AnyField::from(Field::new(2, 3, "x")?).is_truthy());
    /// assert!(AnyField::from(Field::new(2, 3, vec!["x"])?).is_truthy());
    /// assert!(AnyField::from(BinaryField::new(2, 999, b"")?).is_truthy());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            AnyField::Text(f) => !f.is_empty(),
            AnyField::Binary(_) => true,
        }
    }

    /// Bytes of the field (`NIST`).
    ///
    /// ```
    /// use ansi_nist_itl::{AnyField, Field};
    ///
    /// assert_eq!(AnyField::from(Field::new(2, 3, 0)?).to_bytes()?, b"2.003:0");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        match self {
            AnyField::Text(f) => f.to_bytes(),
            AnyField::Binary(f) => f.to_bytes(),
        }
    }
}

impl From<Field> for AnyField {
    fn from(f: Field) -> Self {
        AnyField::Text(f)
    }
}

impl From<BinaryField> for AnyField {
    fn from(f: BinaryField) -> Self {
        AnyField::Binary(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[test]
    fn tag_conversion() {
        assert_eq!(Field::new(2, true, "").unwrap().tag(), &PyInt::from(1u32));
        assert_eq!(Field::new(2, "0_7", "").unwrap().tag(), &PyInt::from(7u32));
        let big = "12345678901234567890";
        let f = Field::new(2, big, "x").unwrap();
        assert_eq!(f.to_bytes().unwrap(), b"2.12345678901234567890:x");

        let e = Field::new(2, (), "").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
        let e = BinaryField::new(2, "x", b"").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::InvalidLiteral);
        assert_eq!(e.position().record_type, Some(2));
        assert_eq!(e.position().tag, None);
        let e = Field::new(2, "1".repeat(4301), "").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::DigitsLimit);
    }

    #[test]
    fn bad_format() {
        let e = Field::with(2, 3, "", Kind::FSI, "%s", "").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::BadFormat);
        let e = BinaryField::with(2, 3, b"", "%d", "").unwrap_err();
        assert_eq!(e.kind(), &ErrorKind::BadFormat);
        assert_eq!(e.position().tag, Some(PyInt::from(3u32)));
    }

    #[test]
    fn error_position() {
        let e = Field::with(2, 3, "x", "S", DEFAULT_FORMAT, "").unwrap_err();
        assert_eq!(e.position().record_type, Some(2));
        assert_eq!(e.position().tag, Some(PyInt::from(3u32)));
        assert_eq!(e.position().offset, None);
        let e = Field::new(2, 3, "\u{20ac}")
            .unwrap()
            .to_bytes()
            .unwrap_err();
        assert_eq!(e.position().tag, Some(PyInt::from(3u32)));
    }

    #[test]
    fn partial_effects_kept() {
        // `S` only: `'a'` is accepted, then `['b']` fails; `'a'` stays.
        let mut f = Field::with(2, 3, "", "S", DEFAULT_FORMAT, "").unwrap();
        let e = f
            .set_value(vec![ListElem::from("a"), ListElem::from(vec!["b"])])
            .unwrap_err();
        assert_eq!(
            e.to_string(),
            "BAD_SUBFIELD_VALUE: Subfield cannot have items"
        );
        assert_eq!(f.value(), Value::from(vec!["a"]));

        let mut f = Field::with(2, 3, "", "FS", DEFAULT_FORMAT, "").unwrap();
        let ok = SubField::new("a", "SI").unwrap();
        let bad = SubField::new(vec!["b"], "SI").unwrap();
        assert!(f.add_subfields([ok, bad]).is_err());
        assert_eq!(f.len(), 1);
        assert_eq!(f.subfield(0).unwrap().kind(), Kind::new("FS"));
    }

    #[test]
    fn falsy_values_ignore_kind() {
        // `I` only: a false subfield value passes `SubField`, then fails in
        // `add_subfields`, whose message quotes its `repr`.
        let mut f = Field::with(2, 3, "", "I", DEFAULT_FORMAT, "").unwrap();
        let e = f.set_value(vec![0]).unwrap_err();
        assert_eq!(
            e.to_string(),
            "BAD_SUBFIELD_VALUE: Subfield of 2.003: cannot have a value: 0"
        );
        // No kind at all: a false scalar is still accepted.
        let f = Field::with(2, 3, (), "", DEFAULT_FORMAT, "").unwrap();
        assert_eq!(f.to_bytes().unwrap(), b"2.003:");
        // An empty list needs `S` or `I`, even with nothing to add.
        let e = Field::with(2, 3, Vec::<&str>::new(), "F", DEFAULT_FORMAT, "").unwrap_err();
        assert_eq!(
            e.to_string(),
            "BAD_FIELD_VALUE: Field 2.003: cannot have subfields"
        );
        // ... but `add_subfields` with nothing does not.
        let mut f = Field::with(2, 3, "x", "F", DEFAULT_FORMAT, "").unwrap();
        f.add_subfields([]).unwrap();
        assert_eq!(f.value(), Value::from(""));
    }

    #[test]
    fn subfields_take_field_kind() {
        let mut f = Field::new(2, 3, "").unwrap();
        f.add_subfields([SubField::new("a", "S").unwrap()]).unwrap();
        assert_eq!(f.subfield(0).unwrap().kind(), Kind::FSI);
        let f = Field::with(2, 3, vec!["a"], "FS", DEFAULT_FORMAT, "").unwrap();
        assert_eq!(f.subfield(0).unwrap().kind(), Kind::new("FS"));
    }

    #[test]
    fn non_latin1_label() {
        let f = Field::with(2, 3, "x", Kind::FSI, "\u{20ac}%d.%d", "").unwrap();
        assert_eq!(
            f.to_bytes().unwrap_err().to_string(),
            "'latin-1' codec can't encode character '\\u20ac' in position 0: ordinal not in range(256)"
        );
        let f = Field::with(2, 3, vec!["x"], Kind::FSI, "%d.%d\u{20ac}", "").unwrap();
        assert_eq!(f.to_bytes().unwrap_err().kind(), &ErrorKind::Encoding);
        let f = BinaryField::with(2, 3, b"", "%d.%d\u{20ac}", "").unwrap();
        assert_eq!(f.to_bytes().unwrap_err().kind(), &ErrorKind::Encoding);
    }

    #[test]
    fn subfield_encoding_position() {
        // Positions are relative to the item, as `str(v).encode()` in Python.
        let f = Field::new(2, 3, vec![vec!["ab", "c\u{20ac}"]]).unwrap();
        assert_eq!(
            f.to_bytes().unwrap_err().to_string(),
            "'latin-1' codec can't encode character '\\u20ac' in position 1: ordinal not in range(256)"
        );
    }
}
