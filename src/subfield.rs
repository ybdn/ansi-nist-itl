//! Subfields of a text field (SPEC §5).

use crate::US;
use crate::error::{Error, Result};
use crate::py::encode_latin1;
use crate::value::{Item, Kind, Scalar, SubValue};

/// Subfield of a [`Field`](crate::Field): a scalar value, or a list of items
/// separated by US (`0x1F`) when written.
///
/// The [`Kind`] restricts the content: `S` allows a non-empty scalar value,
/// `I` allows items (SPEC §5.1). A false value (`""`, `None`, `0`, `False`)
/// is always accepted.
///
/// ```
/// use ansi_nist_itl::{Kind, SubField};
///
/// let mut sf = SubField::new("ok", Kind::new("S"))?;
/// let err = sf.set_value(vec!["a", "b"]).unwrap_err();
/// assert_eq!(err.to_string(), "BAD_SUBFIELD_VALUE: Subfield cannot have items");
///
/// let sf = SubField::new(vec!["a", "b"], Kind::SI)?;
/// assert_eq!(sf.to_bytes()?, b"a\x1fb");
/// # Ok::<(), ansi_nist_itl::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubField {
    value: Scalar,
    values: Vec<Item>,
    kind: Kind,
}

impl SubField {
    /// `SubField(value, type)`: a subfield of kind `kind`, set to `value`.
    ///
    /// Fails like [`SubField::set_value`].
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// let err = SubField::new("x", "I").unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "BAD_SUBFIELD_VALUE: Subfield cannot have a value, only items"
    /// );
    /// ```
    pub fn new(value: impl Into<SubValue>, kind: impl Into<Kind>) -> Result<Self> {
        let mut sf = SubField {
            value: Scalar::default(),
            values: Vec::new(),
            kind: kind.into(),
        };
        sf.set_value(value)?;
        Ok(sf)
    }

    /// Value: the items if the kind allows them and there are some, the
    /// scalar value otherwise.
    ///
    /// ```
    /// use ansi_nist_itl::{Item, SubField, SubValue};
    ///
    /// let sf = SubField::new(vec!["a", "b"], "SI")?;
    /// assert_eq!(sf.value(), SubValue::Items(vec![Item::from("a"), Item::from("b")]));
    /// assert_eq!(SubField::default().value(), SubValue::from(""));
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn value(&self) -> SubValue {
        if self.kind.items() && !self.values.is_empty() {
            SubValue::Items(self.values.clone())
        } else {
            SubValue::Scalar(self.value.clone())
        }
    }

    /// Replaces the content with `value`: a scalar, or a list of items.
    ///
    /// The content is cleared first, and stays cleared on error: a true
    /// scalar without `S` in the kind gives `BAD_SUBFIELD_VALUE`, and so does
    /// a non-empty list without `I` (see [`SubField::add_values`]).
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// let mut sf = SubField::new(vec!["a", "b"], "I")?;
    /// assert!(sf.set_value("error").is_err());
    /// assert_eq!(sf.len(), 0);
    /// sf.set_value("")?;
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_value(&mut self, value: impl Into<SubValue>) -> Result<()> {
        self.value = Scalar::default();
        self.values.clear();
        match value.into() {
            SubValue::Scalar(v) => {
                if v.is_truthy() && !self.kind.subfields() {
                    return Err(Error::subfield_cannot_have_value());
                }
                self.value = v;
                Ok(())
            }
            SubValue::Items(items) => self.add_values(items),
        }
    }

    /// Appends `items`, as given: an item may itself be a list.
    ///
    /// Fails with `BAD_SUBFIELD_VALUE` if `items` is not empty and the kind
    /// has no `I`; nothing is added then.
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// let mut sf = SubField::default();
    /// sf.add_values(["a", "b"])?;
    /// sf.add_values([1])?;
    /// assert_eq!(sf.to_bytes()?, b"a\x1fb\x1f1");
    ///
    /// let mut sf = SubField::new("", "S")?;
    /// assert!(sf.add_values(["a"]).is_err());
    /// sf.add_values(Vec::<&str>::new())?;
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn add_values<I>(&mut self, items: I) -> Result<()>
    where
        I: IntoIterator,
        I::Item: Into<Item>,
    {
        let mut items = items.into_iter().peekable();
        if items.peek().is_some() && !self.kind.items() {
            return Err(Error::subfield_cannot_have_items());
        }
        self.values.extend(items.map(Into::into));
        Ok(())
    }

    /// Number of items.
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// assert_eq!(SubField::new(vec!["one", "two"], "I")?.len(), 2);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// `true` if the subfield has no item.
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// assert!(SubField::new("x", "SI")?.is_empty());
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Item at `index` (`IndexError` if out of range).
    ///
    /// ```
    /// use ansi_nist_itl::{Item, SubField};
    ///
    /// let sf = SubField::new(vec!["a", "b"], "SI")?;
    /// assert_eq!(sf.item(1)?, &Item::from("b"));
    /// assert_eq!(sf.item(2).unwrap_err().to_string(), "list index out of range");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn item(&self, index: usize) -> Result<&Item> {
        self.values.get(index).ok_or_else(Error::index_out_of_range)
    }

    /// Items, whatever the kind.
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// assert_eq!(SubField::new(vec!["a"], "I")?.items().len(), 1);
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn items(&self) -> &[Item] {
        &self.values
    }

    /// Kind of the subfield.
    ///
    /// ```
    /// use ansi_nist_itl::{Kind, SubField};
    ///
    /// assert_eq!(SubField::default().kind(), Kind::SI);
    /// ```
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Changes the kind, without checking the current content.
    ///
    /// Without `I`, existing items are hidden from [`SubField::value`] but
    /// still written by [`SubField::to_bytes`], as in `nistitl`.
    ///
    /// ```
    /// use ansi_nist_itl::{SubField, SubValue};
    ///
    /// let mut sf = SubField::new(vec!["a"], "SI")?;
    /// sf.set_kind("S");
    /// assert_eq!(sf.value(), SubValue::from(""));
    /// assert_eq!(sf.to_bytes()?, b"a");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn set_kind(&mut self, kind: impl Into<Kind>) {
        self.kind = kind.into();
    }

    /// Bytes of the subfield (`NIST`).
    ///
    /// Without items: nothing for `None`, the decimal value of an integer,
    /// `True` / `False`, or the Latin-1 text. With items: each item as
    /// `str(item)` in Latin-1 (`None` is written `None`), separated by US.
    /// A character outside Latin-1 gives an `Encoding` error.
    ///
    /// ```
    /// use ansi_nist_itl::SubField;
    ///
    /// assert_eq!(SubField::new((), "SI")?.to_bytes()?, b"");
    /// assert_eq!(SubField::new(12, "SI")?.to_bytes()?, b"12");
    /// assert_eq!(SubField::new("é", "SI")?.to_bytes()?, b"\xe9");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if self.values.is_empty() {
            return match &self.value {
                Scalar::Null => Ok(Vec::new()),
                Scalar::Text(s) => Ok(encode_latin1(s)?),
                other => Ok(other.to_string().into_bytes()),
            };
        }
        let mut out = Vec::new();
        for (i, item) in self.values.iter().enumerate() {
            if i > 0 {
                out.push(US);
            }
            out.extend(encode_latin1(&item.to_string())?);
        }
        Ok(out)
    }
}

impl Default for SubField {
    /// `SubField()`: value `""`, kind `SI`.
    fn default() -> Self {
        SubField {
            value: Scalar::default(),
            values: Vec::new(),
            kind: Kind::SI,
        }
    }
}
