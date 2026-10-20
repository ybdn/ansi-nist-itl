//! Values stored in fields, subfields and items (SPEC §2).
//!
//! `nistitl` stores arbitrary Python objects. The crate covers what it
//! produces when reading and what its examples write: `None`, `bool`, `int`
//! and `str` ([`Scalar`]), and lists of them ([`Value`], [`SubValue`],
//! [`Item`]). `Display` gives Python's `str()`.

use std::cmp::Ordering;
use std::fmt;

use crate::py;

/// Arbitrary-precision integer, the equivalent of a Python `int`.
///
/// `nistitl` reads tag numbers, LEN and IDC with Python's `int()`, which has
/// no upper bound: a tag of 20 digits is valid. `PyInt` keeps the decimal
/// digits in canonical form (no leading zeros, `0` is never negative) and
/// only supports what the crate needs: comparison, display and conversion
/// from and to machine integers.
///
/// ```
/// use ansi_nist_itl::PyInt;
///
/// let n = PyInt::from(-42i64);
/// assert_eq!(n.to_string(), "-42");
/// assert_eq!(n.to_i64(), Some(-42));
/// assert!(PyInt::from(7u64) > n);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyInt {
    negative: bool,
    /// ASCII decimal digits, without leading zeros (`"0"` for zero).
    digits: String,
}

impl PyInt {
    /// Builds a `PyInt` from ASCII decimal digits, leading zeros allowed.
    ///
    /// The caller guarantees that `digits` only contains `0`–`9`.
    pub(crate) fn from_ascii_digits(negative: bool, digits: &[u8]) -> Self {
        let first = digits.iter().position(|&d| d != b'0');
        let digits = match first {
            Some(i) => digits.get(i..).unwrap_or_default(),
            None => b"0",
        };
        let digits: String = digits.iter().map(|&d| char::from(d)).collect();
        let negative = negative && digits != "0";
        PyInt { negative, digits }
    }

    /// Returns `true` if the value is zero (a "false" value in Python).
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert!(PyInt::from(0u64).is_zero());
    /// assert!(!PyInt::from(-1i64).is_zero());
    /// ```
    pub fn is_zero(&self) -> bool {
        self.digits == "0"
    }

    /// Returns `true` if the value is strictly negative.
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert!(PyInt::from(-1i64).is_negative());
    /// assert!(!PyInt::from(0i64).is_negative());
    /// ```
    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// Decimal digits of the absolute value, without leading zeros.
    pub(crate) fn digits(&self) -> &str {
        &self.digits
    }

    /// Converts to `i64`, or `None` if the value does not fit.
    ///
    /// ```
    /// use ansi_nist_itl::PyInt;
    ///
    /// assert_eq!(PyInt::from(i64::MIN).to_i64(), Some(i64::MIN));
    /// assert_eq!(PyInt::from(u64::MAX).to_i64(), None);
    /// ```
    pub fn to_i64(&self) -> Option<i64> {
        let magnitude: u64 = self.digits.parse().ok()?;
        if self.negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }
}

impl From<u64> for PyInt {
    fn from(n: u64) -> Self {
        PyInt {
            negative: false,
            digits: n.to_string(),
        }
    }
}

impl From<i64> for PyInt {
    fn from(n: i64) -> Self {
        PyInt {
            negative: n < 0,
            digits: n.unsigned_abs().to_string(),
        }
    }
}

impl From<u32> for PyInt {
    fn from(n: u32) -> Self {
        PyInt::from(u64::from(n))
    }
}

impl From<i32> for PyInt {
    fn from(n: i32) -> Self {
        PyInt::from(i64::from(n))
    }
}

impl fmt::Display for PyInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        f.write_str(&self.digits)
    }
}

impl Ord for PyInt {
    fn cmp(&self, other: &Self) -> Ordering {
        // Canonical digits: a longer magnitude is a larger magnitude.
        let magnitude = || {
            self.digits
                .len()
                .cmp(&other.digits.len())
                .then_with(|| self.digits.cmp(&other.digits))
        };
        match (self.negative, other.negative) {
            (false, false) => magnitude(),
            (true, true) => magnitude().reverse(),
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
        }
    }
}

impl PartialOrd for PyInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Scalar value of a field, a subfield or an item.
///
/// `Display` is Python's `str()`.
///
/// ```
/// use ansi_nist_itl::Scalar;
///
/// assert_eq!(Scalar::from("abc").to_string(), "abc");
/// assert_eq!(Scalar::from(-7).to_string(), "-7");
/// assert_eq!(Scalar::from(true).to_string(), "True");
/// assert_eq!(Scalar::from(()).to_string(), "None");
/// assert!(!Scalar::from("").is_truthy());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scalar {
    /// `None`.
    Null,
    /// `True` / `False`.
    Bool(bool),
    /// `int`.
    Int(PyInt),
    /// `str` (Latin-1 when read, any Unicode text when built).
    Text(String),
}

impl Scalar {
    /// Python truthiness (`if value:`): `None`, `False`, `0` and `""` are false.
    ///
    /// ```
    /// use ansi_nist_itl::Scalar;
    ///
    /// assert!(!Scalar::from(0).is_truthy());
    /// assert!(Scalar::from("0").is_truthy());
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            Scalar::Null => false,
            Scalar::Bool(b) => *b,
            Scalar::Int(n) => !n.is_zero(),
            Scalar::Text(s) => !s.is_empty(),
        }
    }

    /// Python `repr()`: like `str()`, except that text is quoted and escaped.
    ///
    /// ```
    /// use ansi_nist_itl::Scalar;
    ///
    /// assert_eq!(Scalar::from("a'b").repr(), "\"a'b\"");
    /// assert_eq!(Scalar::from(3).repr(), "3");
    /// ```
    pub fn repr(&self) -> String {
        match self {
            Scalar::Text(s) => py::repr_str(s),
            other => other.to_string(),
        }
    }
}

impl Default for Scalar {
    /// `""`, the default value of `nistitl` fields and subfields.
    fn default() -> Self {
        Scalar::Text(String::new())
    }
}

impl fmt::Display for Scalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scalar::Null => f.write_str(py::NONE_STR),
            Scalar::Bool(b) => f.write_str(py::bool_str(*b)),
            Scalar::Int(n) => n.fmt(f),
            Scalar::Text(s) => f.write_str(s),
        }
    }
}

/// `repr` of a list: `[a, b]`.
fn repr_list<T>(items: &[T], repr: impl Fn(&T) -> String) -> String {
    let parts: Vec<String> = items.iter().map(repr).collect();
    format!("[{}]", parts.join(", "))
}

/// Item of a subfield: a scalar, or a nested list (an edge case, SPEC §5.5).
///
/// ```
/// use ansi_nist_itl::Item;
///
/// let item = Item::from(vec!["a", "b"]);
/// assert_eq!(item.to_string(), "['a', 'b']");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    /// Scalar item.
    Scalar(Scalar),
    /// Nested list.
    List(Vec<Item>),
}

impl Item {
    /// Python truthiness: a list is false when empty.
    ///
    /// ```
    /// use ansi_nist_itl::Item;
    ///
    /// assert!(!Item::List(vec![]).is_truthy());
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            Item::Scalar(s) => s.is_truthy(),
            Item::List(l) => !l.is_empty(),
        }
    }

    /// Python `repr()`.
    ///
    /// ```
    /// use ansi_nist_itl::Item;
    ///
    /// assert_eq!(Item::from("x").repr(), "'x'");
    /// ```
    pub fn repr(&self) -> String {
        match self {
            Item::Scalar(s) => s.repr(),
            Item::List(l) => repr_list(l, Item::repr),
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Item::Scalar(s) => s.fmt(f),
            Item::List(_) => f.write_str(&self.repr()),
        }
    }
}

/// Value of a subfield: a scalar, or a list of items.
///
/// ```
/// use ansi_nist_itl::{Item, SubValue};
///
/// let v = SubValue::Items(vec![Item::from("a"), Item::from(())]);
/// assert_eq!(v.to_string(), "['a', None]");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubValue {
    /// Scalar value.
    Scalar(Scalar),
    /// List of items.
    Items(Vec<Item>),
}

impl SubValue {
    /// Python truthiness: a list is false when empty.
    ///
    /// ```
    /// use ansi_nist_itl::{Scalar, SubValue};
    ///
    /// assert!(SubValue::Scalar(Scalar::from("a")).is_truthy());
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            SubValue::Scalar(s) => s.is_truthy(),
            SubValue::Items(l) => !l.is_empty(),
        }
    }

    /// Python `repr()`.
    ///
    /// ```
    /// use ansi_nist_itl::{Item, SubValue};
    ///
    /// assert_eq!(SubValue::Items(vec![Item::from(1)]).repr(), "[1]");
    /// ```
    pub fn repr(&self) -> String {
        match self {
            SubValue::Scalar(s) => s.repr(),
            SubValue::Items(l) => repr_list(l, Item::repr),
        }
    }
}

impl fmt::Display for SubValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubValue::Scalar(s) => s.fmt(f),
            SubValue::Items(_) => f.write_str(&self.repr()),
        }
    }
}

/// Element of a [`Value::List`]: a scalar, or a list of items (one subfield).
///
/// ```
/// use ansi_nist_itl::ListElem;
///
/// assert_eq!(ListElem::from(vec!["a", "b"]).to_string(), "['a', 'b']");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ListElem {
    /// Scalar element.
    Scalar(Scalar),
    /// List of items.
    List(Vec<Item>),
}

impl ListElem {
    /// Python `repr()`.
    ///
    /// ```
    /// use ansi_nist_itl::ListElem;
    ///
    /// assert_eq!(ListElem::from("a").repr(), "'a'");
    /// ```
    pub fn repr(&self) -> String {
        match self {
            ListElem::Scalar(s) => s.repr(),
            ListElem::List(l) => repr_list(l, Item::repr),
        }
    }
}

impl fmt::Display for ListElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListElem::Scalar(s) => s.fmt(f),
            ListElem::List(_) => f.write_str(&self.repr()),
        }
    }
}

/// Value given to a field mutator: the equivalent of the Python `value`.
///
/// ```
/// use ansi_nist_itl::Value;
///
/// assert_eq!(Value::from("x").to_string(), "x");
/// assert_eq!(Value::from(vec![vec!["a", "b"]]).to_string(), "[['a', 'b']]");
/// assert!(!Value::from(Vec::<&str>::new()).is_truthy());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Value {
    /// Scalar value.
    Scalar(Scalar),
    /// `list` / `tuple`.
    List(Vec<ListElem>),
}

impl Value {
    /// Python truthiness: a list is false when empty.
    ///
    /// ```
    /// use ansi_nist_itl::Value;
    ///
    /// assert!(Value::from(vec![""]).is_truthy());
    /// ```
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Scalar(s) => s.is_truthy(),
            Value::List(l) => !l.is_empty(),
        }
    }

    /// Python `repr()`.
    ///
    /// ```
    /// use ansi_nist_itl::Value;
    ///
    /// assert_eq!(Value::from(vec!["a"]).repr(), "['a']");
    /// ```
    pub fn repr(&self) -> String {
        match self {
            Value::Scalar(s) => s.repr(),
            Value::List(l) => repr_list(l, ListElem::repr),
        }
    }
}

impl Default for Value {
    /// `""`.
    fn default() -> Self {
        Value::Scalar(Scalar::default())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Scalar(s) => s.fmt(f),
            Value::List(_) => f.write_str(&self.repr()),
        }
    }
}

/// `From` conversions of the Python scalar types for `Scalar` and each wrapper.
macro_rules! scalar_conversions {
    ($($t:ty => |$v:ident| $e:expr),* $(,)?) => {
        $(
            impl From<$t> for Scalar {
                fn from($v: $t) -> Self {
                    $e
                }
            }
            impl From<$t> for Item {
                fn from(v: $t) -> Self {
                    Item::Scalar(Scalar::from(v))
                }
            }
            impl From<$t> for SubValue {
                fn from(v: $t) -> Self {
                    SubValue::Scalar(Scalar::from(v))
                }
            }
            impl From<$t> for ListElem {
                fn from(v: $t) -> Self {
                    ListElem::Scalar(Scalar::from(v))
                }
            }
            impl From<$t> for Value {
                fn from(v: $t) -> Self {
                    Value::Scalar(Scalar::from(v))
                }
            }
        )*
    };
}

scalar_conversions! {
    &str => |v| Scalar::Text(v.to_owned()),
    String => |v| Scalar::Text(v),
    i32 => |v| Scalar::Int(PyInt::from(v)),
    u32 => |v| Scalar::Int(PyInt::from(v)),
    i64 => |v| Scalar::Int(PyInt::from(v)),
    u64 => |v| Scalar::Int(PyInt::from(v)),
    PyInt => |v| Scalar::Int(v),
    bool => |v| Scalar::Bool(v),
    () => |_v| Scalar::Null,
}

impl From<Scalar> for Item {
    fn from(s: Scalar) -> Self {
        Item::Scalar(s)
    }
}

impl From<Scalar> for SubValue {
    fn from(s: Scalar) -> Self {
        SubValue::Scalar(s)
    }
}

impl From<Scalar> for ListElem {
    fn from(s: Scalar) -> Self {
        ListElem::Scalar(s)
    }
}

impl From<Scalar> for Value {
    fn from(s: Scalar) -> Self {
        Value::Scalar(s)
    }
}

impl<T: Into<Item>> From<Vec<T>> for Item {
    fn from(v: Vec<T>) -> Self {
        Item::List(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Item>> From<Vec<T>> for SubValue {
    fn from(v: Vec<T>) -> Self {
        SubValue::Items(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Item>> From<Vec<T>> for ListElem {
    fn from(v: Vec<T>) -> Self {
        ListElem::List(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<ListElem>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}

/// Kind of a field or subfield: the flags `F`, `S` and `I` of the Python `type`.
///
/// `F`: the field may hold a value; `S`: subfields; `I`: items. Only the
/// presence of the three letters matters (SPEC §5.1).
///
/// ```
/// use ansi_nist_itl::Kind;
///
/// let k = Kind::from("SI");
/// assert!(!k.field_value() && k.subfields() && k.items());
/// assert_eq!(k, Kind::SI);
/// assert_eq!(Kind::FSI.to_string(), "FSI");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Kind {
    f: bool,
    s: bool,
    i: bool,
}

impl Kind {
    /// `"FSI"`, the default kind of a field.
    pub const FSI: Kind = Kind {
        f: true,
        s: true,
        i: true,
    };

    /// `"SI"`, the default kind of a subfield.
    pub const SI: Kind = Kind {
        f: false,
        s: true,
        i: true,
    };

    /// Flags read from a Python `type` string.
    ///
    /// ```
    /// use ansi_nist_itl::Kind;
    ///
    /// assert_eq!(Kind::new("xIyF"), Kind::new("FI"));
    /// ```
    pub fn new(kind: &str) -> Self {
        Kind {
            f: kind.contains('F'),
            s: kind.contains('S'),
            i: kind.contains('I'),
        }
    }

    /// `'F' in type`: the field may hold a value.
    ///
    /// ```
    /// use ansi_nist_itl::Kind;
    ///
    /// assert!(Kind::FSI.field_value());
    /// ```
    pub fn field_value(self) -> bool {
        self.f
    }

    /// `'S' in type`: subfields are allowed.
    ///
    /// ```
    /// use ansi_nist_itl::Kind;
    ///
    /// assert!(Kind::new("S").subfields());
    /// ```
    pub fn subfields(self) -> bool {
        self.s
    }

    /// `'I' in type`: items are allowed.
    ///
    /// ```
    /// use ansi_nist_itl::Kind;
    ///
    /// assert!(!Kind::new("FS").items());
    /// ```
    pub fn items(self) -> bool {
        self.i
    }
}

impl From<&str> for Kind {
    fn from(kind: &str) -> Self {
        Kind::new(kind)
    }
}

impl fmt::Display for Kind {
    /// Canonical spelling: the present letters in the order `F`, `S`, `I`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (present, letter) in [(self.f, "F"), (self.s, "S"), (self.i, "I")] {
            if present {
                f.write_str(letter)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_and_repr() {
        // SPEC §3.3 / C84.
        let v = Value::List(vec![
            ListElem::from(vec!["a'b", "c\"d"]),
            ListElem::from("x\ty\u{a0}\u{ad}\\"),
        ]);
        assert_eq!(v.to_string(), r#"[["a'b", 'c"d'], 'x\ty\xa0\xad\\']"#);
        assert_eq!(Value::from(()).to_string(), "None");
        assert_eq!(Value::from(()).repr(), "None");
        assert_eq!(
            Value::from(vec![Scalar::Null, Scalar::from(0)]).to_string(),
            "[None, 0]"
        );
        let nested = Item::from(vec![Item::from(vec!["a"]), Item::from(true)]);
        assert_eq!(nested.to_string(), "[['a'], True]");
        assert_eq!(SubValue::from("a").repr(), "'a'");
        assert_eq!(SubValue::from("a").to_string(), "a");
    }

    #[test]
    fn truthiness() {
        assert!(!Scalar::Null.is_truthy());
        assert!(!Scalar::from(false).is_truthy());
        assert!(Scalar::from(-1).is_truthy());
        assert!(!Scalar::from(PyInt::from_ascii_digits(true, b"000")).is_truthy());
        assert!(!Scalar::default().is_truthy());
        assert!(Value::from(vec![()]).is_truthy());
        assert!(!SubValue::Items(vec![]).is_truthy());
        assert!(Item::from(vec![""]).is_truthy());
        assert!(!Value::default().is_truthy());
    }

    #[test]
    fn kinds() {
        assert_eq!(Kind::new(""), Kind::from("xyz"));
        assert_eq!(Kind::new("").to_string(), "");
        assert_eq!(Kind::new("IS").to_string(), "SI");
        assert_eq!(Kind::new("ISF"), Kind::FSI);
    }

    #[test]
    fn canonical_form() {
        let n = PyInt::from_ascii_digits(false, b"007");
        assert_eq!(n, PyInt::from(7u64));
        let z = PyInt::from_ascii_digits(true, b"000");
        assert_eq!(z, PyInt::from(0i64));
        assert!(!z.is_negative());
        assert_eq!(z.to_string(), "0");
    }

    #[test]
    fn ordering() {
        let mut v: Vec<PyInt> = [10i64, -3, 0, 2, -20, 999]
            .into_iter()
            .map(PyInt::from)
            .collect();
        v.push(PyInt::from_ascii_digits(
            false,
            b"123456789012345678901234567890",
        ));
        v.push(PyInt::from_ascii_digits(
            true,
            b"123456789012345678901234567890",
        ));
        v.sort();
        let s: Vec<String> = v.iter().map(ToString::to_string).collect();
        assert_eq!(
            s,
            [
                "-123456789012345678901234567890",
                "-20",
                "-3",
                "0",
                "2",
                "10",
                "999",
                "123456789012345678901234567890"
            ]
        );
    }

    #[test]
    fn to_i64_bounds() {
        assert_eq!(PyInt::from(i64::MAX).to_i64(), Some(i64::MAX));
        assert_eq!(
            PyInt::from_ascii_digits(true, b"9223372036854775809").to_i64(),
            None
        );
        assert_eq!(PyInt::from(i64::MIN).to_string(), "-9223372036854775808");
    }
}
