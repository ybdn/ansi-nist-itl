//! `Serialize` and `Deserialize` on the model (SPEC §13.4, LIB-23), behind
//! the `serde` feature.
//!
//! Values take their Python form (`Field::value()`), binary data base64 in
//! human-readable formats. Deserialization goes through the public API: it
//! makes the same checks and gives the same messages.

use std::fmt;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

use crate::bytes::Bytes;
use crate::field::{AnyField, BinaryField, DEFAULT_FORMAT, Field};
use crate::message::Message;
use crate::record::{AsciiRecord, BinaryRecord, Record};
use crate::subfield::SubField;
use crate::value::{Item, Kind, ListElem, PyInt, Scalar, SubValue, Value};

/// Key of an integer beyond `i64`: `{"int": "-123…"}`.
const INT_KEY: &str = "int";

// ---------------------------------------------------------------- base64

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 of RFC 4648, with padding.
fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                let index = (n >> shift & 0x3F) as usize;
                out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Value of a base64 digit.
fn base64_digit(c: u8) -> Option<u32> {
    let v = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(v))
}

/// Reads canonical base64: padded, standard alphabet, unused bits at zero.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if text.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let chunks = text.chunks(4);
    let last = chunks.len().saturating_sub(1);
    for (i, chunk) in chunks.enumerate() {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && i != last) {
            return None;
        }
        let mut n = 0u32;
        for &c in chunk.get(..4 - pad)? {
            n = n << 6 | base64_digit(c)?;
        }
        n <<= 6 * pad as u32;
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        if bytes
            .get(3 - pad..)
            .is_some_and(|rest| rest.iter().any(|&b| b != 0))
        {
            return None;
        }
        out.extend_from_slice(bytes.get(..3 - pad)?);
    }
    Some(out)
}

// ---------------------------------------------------------------- Bytes

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&base64_encode(self))
        } else {
            serializer.serialize_bytes(self)
        }
    }
}

struct BytesVisitor;

impl<'de> Visitor<'de> for BytesVisitor {
    type Value = Bytes;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("base64 text or bytes")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Bytes, E> {
        base64_decode(v)
            .map(Bytes::from)
            .ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &"canonical base64"))
    }

    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Bytes, E> {
        Ok(Bytes::from(v))
    }

    fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Bytes, E> {
        Ok(Bytes::from(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Bytes, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1 << 16));
        while let Some(b) = seq.next_element::<u8>()? {
            out.push(b);
        }
        Ok(Bytes::from(out))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(BytesVisitor)
        } else {
            deserializer.deserialize_byte_buf(BytesVisitor)
        }
    }
}

// ---------------------------------------------------------------- values

impl Serialize for PyInt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.to_i64() {
            Some(n) => serializer.serialize_i64(n),
            None => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry(INT_KEY, &self.to_string())?;
                map.end()
            }
        }
    }
}

/// `-?[0-9]+` as a `PyInt`.
fn parse_int(text: &str) -> Option<PyInt> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| PyInt::from_ascii_digits(negative, digits.as_bytes()))
}

/// Reads `{"int": "-123…"}`.
fn visit_int_map<'de, A: MapAccess<'de>>(mut map: A) -> Result<PyInt, A::Error> {
    let Some(key) = map.next_key::<String>()? else {
        return Err(de::Error::missing_field(INT_KEY));
    };
    if key != INT_KEY {
        return Err(de::Error::unknown_field(&key, &[INT_KEY]));
    }
    let text: String = map.next_value()?;
    if map.next_key::<String>()?.is_some() {
        return Err(de::Error::custom(
            "an integer object has only the key `int`",
        ));
    }
    parse_int(&text)
        .ok_or_else(|| de::Error::invalid_value(de::Unexpected::Str(&text), &"decimal digits"))
}

struct PyIntVisitor;

impl<'de> Visitor<'de> for PyIntVisitor {
    type Value = PyInt;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an integer, or {\"int\": \"digits\"}")
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<PyInt, E> {
        Ok(PyInt::from(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<PyInt, E> {
        Ok(PyInt::from(v))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<PyInt, A::Error> {
        visit_int_map(map)
    }
}

impl<'de> Deserialize<'de> for PyInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PyIntVisitor)
    }
}

impl Serialize for Scalar {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Scalar::Null => serializer.serialize_unit(),
            Scalar::Bool(b) => serializer.serialize_bool(*b),
            Scalar::Int(n) => n.serialize(serializer),
            Scalar::Text(s) => serializer.serialize_str(s),
        }
    }
}

/// Reads a scalar, or a list of items when `nested` (SPEC §13.4).
struct ItemVisitor {
    nested: bool,
}

impl<'de> Visitor<'de> for ItemVisitor {
    type Value = Item;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.nested {
            "null, a boolean, an integer, a string or a list"
        } else {
            "null, a boolean, an integer or a string"
        })
    }

    fn visit_unit<E: de::Error>(self) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Null))
    }

    fn visit_none<E: de::Error>(self) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Null))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Bool(v)))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Int(PyInt::from(v))))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Int(PyInt::from(v))))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Text(v.to_owned())))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Item, E> {
        Ok(Item::Scalar(Scalar::Text(v)))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Item, A::Error> {
        visit_int_map(map).map(|n| Item::Scalar(Scalar::Int(n)))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Item, A::Error> {
        if !self.nested {
            return Err(de::Error::invalid_type(de::Unexpected::Seq, &self));
        }
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Item>()? {
            items.push(item);
        }
        Ok(Item::List(items))
    }
}

impl<'de> Deserialize<'de> for Scalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match deserializer.deserialize_any(ItemVisitor { nested: false })? {
            Item::Scalar(s) => Ok(s),
            Item::List(_) => Err(de::Error::custom("expected a scalar")),
        }
    }
}

/// Writes a scalar, or a list of `T`.
fn serialize_list<S: Serializer, T: Serialize>(
    serializer: S,
    list: &[T],
) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(list.len()))?;
    for elem in list {
        seq.serialize_element(elem)?;
    }
    seq.end()
}

impl Serialize for Item {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Item::Scalar(s) => s.serialize(serializer),
            Item::List(items) => serialize_list(serializer, items),
        }
    }
}

impl<'de> Deserialize<'de> for Item {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ItemVisitor { nested: true })
    }
}

impl Serialize for ListElem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ListElem::Scalar(s) => s.serialize(serializer),
            ListElem::List(items) => serialize_list(serializer, items),
        }
    }
}

impl<'de> Deserialize<'de> for ListElem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Item::deserialize(deserializer)? {
            Item::Scalar(s) => ListElem::Scalar(s),
            Item::List(items) => ListElem::List(items),
        })
    }
}

impl Serialize for SubValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            SubValue::Scalar(s) => s.serialize(serializer),
            SubValue::Items(items) => serialize_list(serializer, items),
        }
    }
}

impl<'de> Deserialize<'de> for SubValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Item::deserialize(deserializer)? {
            Item::Scalar(s) => SubValue::Scalar(s),
            Item::List(items) => SubValue::Items(items),
        })
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Scalar(s) => s.serialize(serializer),
            Value::List(elems) => serialize_list(serializer, elems),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Item::deserialize(deserializer)? {
            Item::Scalar(s) => Value::Scalar(s),
            Item::List(items) => Value::List(
                items
                    .into_iter()
                    .map(|item| match item {
                        Item::Scalar(s) => ListElem::Scalar(s),
                        Item::List(items) => ListElem::List(items),
                    })
                    .collect(),
            ),
        })
    }
}

impl Serialize for Kind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|s| Kind::new(&s))
    }
}

// ---------------------------------------------------------------- fields

fn is_fsi(kind: &Kind) -> bool {
    *kind == Kind::FSI
}

fn is_si(kind: &Kind) -> bool {
    *kind == Kind::SI
}

fn is_default_format(format: &str) -> bool {
    format == DEFAULT_FORMAT
}

fn is_true(b: &bool) -> bool {
    *b
}

/// A field as written: with its record number when alone, without it in a
/// record.
#[derive(Serialize)]
struct FieldOut<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<u32>,
    tag: &'a PyInt,
    #[serde(skip_serializing_if = "str::is_empty")]
    alias: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<Kind>,
    #[serde(skip_serializing_if = "is_default_format")]
    format: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<&'a Bytes>,
}

impl<'a> FieldOut<'a> {
    fn text(f: &'a Field, record: Option<u32>) -> Self {
        FieldOut {
            record,
            tag: f.tag(),
            alias: f.alias(),
            kind: Some(f.kind()).filter(|k| !is_fsi(k)),
            format: f.format(),
            value: Some(f.value()),
            bytes: None,
        }
    }

    fn binary(f: &'a BinaryField, record: Option<u32>) -> Self {
        FieldOut {
            record,
            tag: f.tag(),
            alias: f.alias(),
            kind: None,
            format: f.format(),
            value: None,
            bytes: Some(f.bytes()),
        }
    }

    fn any(field: &'a AnyField, record: Option<u32>) -> Self {
        match field {
            AnyField::Text(f) => FieldOut::text(f, record),
            AnyField::Binary(f) => FieldOut::binary(f, record),
        }
    }
}

impl Serialize for Field {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FieldOut::text(self, Some(self.record())).serialize(serializer)
    }
}

impl Serialize for BinaryField {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FieldOut::binary(self, Some(self.record())).serialize(serializer)
    }
}

impl Serialize for AnyField {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FieldOut::any(self, Some(self.record())).serialize(serializer)
    }
}

/// A field as read; `record` is required when alone, optional in a record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldIn {
    record: Option<u32>,
    tag: PyInt,
    #[serde(default)]
    alias: String,
    kind: Option<Kind>,
    format: Option<String>,
    /// `null` is a value (`None`), not a missing key.
    #[serde(default, deserialize_with = "some_value")]
    value: Option<Value>,
    bytes: Option<Bytes>,
}

fn some_value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

impl FieldIn {
    /// The field, built by the API, in a record of type `record` if known.
    fn build<E: de::Error>(self, record: Option<u32>) -> Result<AnyField, E> {
        let record = self
            .record
            .or(record)
            .ok_or_else(|| E::missing_field("record"))?;
        let format = self.format.as_deref().unwrap_or(DEFAULT_FORMAT);
        let tag = Scalar::Int(self.tag);
        let field = match (self.value, self.bytes) {
            (Some(value), None) => {
                let kind = self.kind.unwrap_or(Kind::FSI);
                Field::with(record, tag, value, kind, format, self.alias).map(AnyField::Text)
            }
            (None, Some(bytes)) => {
                if self.kind.is_some() {
                    return Err(E::custom("a binary field has no `kind`"));
                }
                BinaryField::with(record, tag, bytes, format, self.alias).map(AnyField::Binary)
            }
            (Some(_), Some(_)) => {
                return Err(E::custom("a field has `value` or `bytes`, not both"));
            }
            (None, None) => return Err(E::custom("missing field `value` or `bytes`")),
        };
        field.map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for AnyField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FieldIn::deserialize(deserializer)?.build(None)
    }
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match AnyField::deserialize(deserializer)? {
            AnyField::Text(f) => Ok(f),
            AnyField::Binary(_) => Err(de::Error::custom("expected a text field (`value`)")),
        }
    }
}

impl<'de> Deserialize<'de> for BinaryField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match AnyField::deserialize(deserializer)? {
            AnyField::Binary(f) => Ok(f),
            AnyField::Text(_) => Err(de::Error::custom("expected a binary field (`bytes`)")),
        }
    }
}

#[derive(Serialize)]
struct SubFieldOut {
    #[serde(skip_serializing_if = "is_si")]
    kind: Kind,
    value: SubValue,
}

impl Serialize for SubField {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SubFieldOut {
            kind: self.kind(),
            value: self.value(),
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubFieldIn {
    kind: Option<Kind>,
    value: SubValue,
}

impl<'de> Deserialize<'de> for SubField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let sf = SubFieldIn::deserialize(deserializer)?;
        SubField::new(sf.value, sf.kind.unwrap_or(Kind::SI)).map_err(de::Error::custom)
    }
}

// ---------------------------------------------------------------- records

/// The fields of a record, without their record number.
struct FieldsOut<'a>(&'a [AnyField]);

impl Serialize for FieldsOut<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for field in self.0 {
            seq.serialize_element(&FieldOut::any(field, None))?;
        }
        seq.end()
    }
}

#[derive(Serialize)]
struct AsciiOut<'a> {
    #[serde(rename = "type")]
    record_type: u32,
    #[serde(skip_serializing_if = "is_true")]
    autosort: bool,
    fields: FieldsOut<'a>,
}

impl Serialize for AsciiRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AsciiOut {
            record_type: self.record_type(),
            autosort: self.autosort(),
            fields: FieldsOut(self.fields()),
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct BinaryOut<'a> {
    #[serde(rename = "type")]
    record_type: u32,
    idc: u8,
    #[serde(skip_serializing_if = "str::is_empty")]
    format: &'a str,
    bytes: &'a Bytes,
}

impl Serialize for BinaryRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        BinaryOut {
            record_type: self.record_type(),
            idc: self.idc(),
            format: self.format(),
            bytes: self.bytes(),
        }
        .serialize(serializer)
    }
}

impl Serialize for Record {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Record::Ascii(r) => r.serialize(serializer),
            Record::Binary(r) => r.serialize(serializer),
        }
    }
}

/// A record as read: its keys tell the form, tagged or binary, as the API
/// builds both whatever the type.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordIn {
    #[serde(rename = "type")]
    record_type: u32,
    autosort: Option<bool>,
    fields: Option<Vec<FieldIn>>,
    idc: Option<u8>,
    format: Option<String>,
    bytes: Option<Bytes>,
}

impl RecordIn {
    fn build<E: de::Error>(self) -> Result<Record, E> {
        let t = self.record_type;
        let binary = self.idc.is_some() || self.format.is_some() || self.bytes.is_some();
        match self.fields {
            Some(_) if binary => Err(E::custom(
                "a record has `fields` (tagged) or `idc` and `bytes` (binary), not both",
            )),
            Some(fields) => {
                let autosort = self.autosort.unwrap_or(true);
                let mut r = AsciiRecord::with(t, false, autosort).map_err(E::custom)?;
                for field in fields {
                    r.add_field(field.build::<E>(Some(t))?).map_err(E::custom)?;
                }
                Ok(Record::Ascii(r))
            }
            None if binary => {
                if self.autosort.is_some() {
                    return Err(E::custom("a binary record has no `autosort`"));
                }
                let mut r = BinaryRecord::new(t);
                r.set_idc(self.idc.ok_or_else(|| E::missing_field("idc"))?);
                r.set_value(self.bytes.ok_or_else(|| E::missing_field("bytes"))?);
                if let Some(format) = self.format.as_deref().filter(|f| !f.is_empty()) {
                    r.set_format(format).map_err(E::custom)?;
                }
                Ok(Record::Binary(r))
            }
            None => Err(E::custom(
                "missing field `fields` (tagged record) or `idc` and `bytes` (binary record)",
            )),
        }
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        RecordIn::deserialize(deserializer)?.build()
    }
}

impl<'de> Deserialize<'de> for AsciiRecord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Record::deserialize(deserializer)? {
            Record::Ascii(r) => Ok(r),
            Record::Binary(r) => Err(de::Error::custom(format!(
                "expected a tagged record, not type {}",
                r.record_type()
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for BinaryRecord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Record::deserialize(deserializer)? {
            Record::Binary(r) => Ok(r),
            Record::Ascii(r) => Err(de::Error::custom(format!(
                "expected a binary record, not type {}",
                r.record_type()
            ))),
        }
    }
}

// ---------------------------------------------------------------- message

#[derive(Serialize)]
struct MessageOut<'a> {
    records: &'a [Record],
}

impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        MessageOut {
            records: self.records(),
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageIn {
    records: Vec<Record>,
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut records = MessageIn::deserialize(deserializer)?.records.into_iter();
        let mut msg = match records.next() {
            Some(Record::Ascii(r)) if r.record_type() == 1 => Message::from_type1(r),
            _ => return Err(de::Error::custom("the first record must be the Type-1")),
        };
        for record in records {
            msg.push(record).map_err(de::Error::custom)?;
        }
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        // RFC 4648 §10.
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(plain.as_bytes()), encoded);
            assert_eq!(base64_decode(encoded).unwrap(), plain.as_bytes());
        }
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(base64_decode(&base64_encode(&all)).unwrap(), all);
    }

    #[test]
    fn base64_refuses_non_canonical() {
        for text in [
            "Zg", "Zg=", "Zh==", "Zm9=", "Z===", "Zg==Zg==", "Zm9v\n", "Zm-v", "====",
        ] {
            assert_eq!(base64_decode(text), None, "{text:?}");
        }
    }

    #[test]
    fn int_text() {
        assert_eq!(parse_int("-0").unwrap().to_string(), "0");
        assert_eq!(parse_int("007").unwrap().to_string(), "7");
        for text in ["", "-", "+1", " 1", "1.0", "1_0"] {
            assert!(parse_int(text).is_none(), "{text:?}");
        }
    }
}
