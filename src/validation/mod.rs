//! Validation of a message against the standard (LIB-16), behind the
//! `validation` feature.
//!
//! The rules come from the [`dictionary`](crate::dictionary): record types,
//! defined, reserved and deprecated fields, mandatory fields and items,
//! number of subfields and items, character types, sizes and dates. Two
//! rules are written by hand: the Type-1 holds only 7-bit ASCII (section
//! 5.6), and CNT (1.003) lists the records of the message (section 7.3.1).
//!
//! Not checked: the dependent conditions (`D`), the value constraints of
//! the tables (ranges, code lists), and the content of user-defined fields.
//! Sizes are counted in bytes, as the crate reads text (one byte, one
//! character).
//!
//! ```
//! use ansi_nist_itl::validation::{Level, Rule};
//! use ansi_nist_itl::{AsciiRecord, Defaults, Message};
//!
//! let mut msg = Message::with_defaults(&Defaults { date: Some("20200101".into()), tcn: Some("TCN1".into()) });
//! msg.type1_mut().unwrap().set("VER", "0502")?;
//! msg.set_tot("CAR")?;
//! let mut r14 = AsciiRecord::new(14)?;
//! r14.set("IDC", 1)?;
//! r14.set("IMP", "X")?;
//! msg.push(r14)?;
//! msg.to_bytes()?; // computes LEN and CNT
//!
//! let report = msg.validate();
//! assert!(!report.is_valid());
//! let imp = report.findings.iter().find(|f| f.rule == Rule::CharacterType).unwrap();
//! assert_eq!((imp.level, imp.position.tag.as_ref().map(|t| t.to_string())), (Level::Error, Some("3".into())));
//! assert!(report.findings.iter().any(|f| f.rule == Rule::MissingField && f.message.contains("SRC")));
//! # Ok::<(), ansi_nist_itl::Error>(())
//! ```

mod ebts;
// Generated: kept as the generator writes it.
#[rustfmt::skip]
mod ebts112;
mod inti;

use std::fmt;

use crate::dictionary::{
    CharType, Condition, FieldDef, FieldKind, Format, Limit, Occurrence, Standard, SubfieldGroup,
};
use crate::error::Position;
use crate::field::AnyField;
use crate::message::Message;
use crate::record::{AsciiRecord, Record};
use crate::value::{Item, ListElem, PyInt, Value};

/// Result of a validation.
///
/// ```
/// use ansi_nist_itl::dictionary::Standard;
/// use ansi_nist_itl::Message;
///
/// let report = Message::new().validate_against(Standard::Update2015);
/// assert_eq!(report.standard, Some(Standard::Update2015));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Validation {
    /// Tables used; `None` when the version of the message has none.
    pub standard: Option<Standard>,
    /// What does not follow the standard, in the order of the message.
    pub findings: Vec<Finding>,
}

impl Validation {
    /// No finding of level [`Level::Error`].
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// // Version 0400 of a new message: no tables, a warning only.
    /// assert!(Message::new().validate().is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        self.findings.iter().all(|f| f.level < Level::Error)
    }

    /// The findings of level [`Level::Error`].
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// assert_eq!(Message::new().validate().errors().count(), 0);
    /// ```
    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.level == Level::Error)
    }
}

/// Something that does not follow the standard.
///
/// ```
/// use ansi_nist_itl::validation::{Level, Rule};
/// use ansi_nist_itl::Message;
///
/// let report = Message::new().validate();
/// let f = &report.findings[0];
/// assert_eq!((f.rule, f.level), (Rule::Version, Level::Warning));
/// assert_eq!(f.to_string(), "1.002: no tables for version 0400");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Finding {
    /// The rule.
    pub rule: Rule,
    /// How serious it is.
    pub level: Level,
    /// The record (index and type) and the field (tag).
    pub position: Position,
    /// Index of the subfield, from 0.
    pub subfield: Option<usize>,
    /// Index of the item in its subfield, from 0.
    pub item: Option<usize>,
    /// The profile whose rule this is; `None` for a rule of the standard.
    pub profile: Option<Profile>,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Finding {
    /// `type.tag: message`, with the subfield and item when known, after
    /// the profile.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(p) = self.profile {
            write!(f, "{p}: ")?;
        }
        if let Some(t) = self.position.record_type {
            write!(f, "{t}")?;
            if let Some(tag) = &self.position.tag {
                write!(f, ".{:0>3}", tag.to_string())?;
            }
            if let Some(s) = self.subfield {
                write!(f, " subfield {}", s + 1)?;
            }
            if let Some(i) = self.item {
                write!(f, " item {}", i + 1)?;
            }
            f.write_str(": ")?;
        }
        f.write_str(&self.message)
    }
}

/// Level of a [`Finding`].
///
/// ```
/// use ansi_nist_itl::validation::Level;
///
/// assert!(Level::Warning < Level::Error);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Allowed, but not recommended or not checked further: deprecated
    /// records and fields, versions without tables.
    Warning,
    /// Against the standard.
    Error,
}

/// Rule of a [`Finding`].
///
/// ```
/// use ansi_nist_itl::validation::Rule;
///
/// assert_ne!(Rule::MissingField, Rule::MissingItem);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Rule {
    /// 1.002 is not a version with tables (warning).
    Version,
    /// A record type the edition does not define (error), or deprecated
    /// (Types 3, 5 and 6: warning).
    RecordType,
    /// A field number the edition does not define.
    UndefinedField,
    /// A field reserved for future use.
    ReservedField,
    /// A field kept for legacy data (warning).
    DeprecatedField,
    /// A mandatory field is absent.
    MissingField,
    /// Subfields or items where a single value is expected, or the
    /// reverse.
    Structure,
    /// Too few or too many subfields.
    SubfieldCount,
    /// More items than the subfield defines.
    ItemCount,
    /// A mandatory item is empty.
    MissingItem,
    /// A character outside the character type.
    CharacterType,
    /// Too short or too long.
    Size,
    /// Not a valid date, date and time, or GMT.
    Date,
    /// A character of the Type-1 outside 7-bit ASCII (section 5.6).
    Type1Ascii,
    /// CNT (1.003) does not list the records of the message.
    Content,
    /// A type of transaction (1.004) the profile does not define.
    TransactionType,
    /// Too few or too many records of a type for the transaction.
    RecordCount,
    /// A field the transaction does not use.
    FieldNotInTransaction,
    /// A value that does not have the form the profile requires.
    Format,
}

/// An application profile: rules of an organization on top of the
/// standard (LIB-17).
///
/// ```
/// use ansi_nist_itl::validation::Profile;
///
/// assert_eq!(Profile::Ebts112.to_string(), "EBTS 11.2");
/// assert_eq!(Profile::IntI503.to_string(), "INT-I 5.03");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Profile {
    /// FBI CJIS Electronic Biometric Transmission Specification 11.2
    /// (September 20, 2023), on ANSI/NIST-ITL 1-2011 Update 2015.
    Ebts112,
    /// INTERPOL Implementation 5.03 (April 7, 2011), on ANSI/NIST-ITL
    /// 1-2007 (version `0400`), the last one in traditional encoding.
    IntI503,
}

impl fmt::Display for Profile {
    /// Short name of the profile.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Profile::Ebts112 => "EBTS 11.2",
            Profile::IntI503 => "INT-I 5.03",
        })
    }
}

impl Message {
    /// Validates the message against an application profile: the rules of
    /// the standard the profile is built on (Update 2015 for EBTS 11.2; for
    /// INT-I 5.03, on 1-2007, only CNT), then those of the profile.
    ///
    /// ```
    /// use ansi_nist_itl::validation::{Profile, Rule};
    /// use ansi_nist_itl::Message;
    ///
    /// let mut msg = Message::new();
    /// msg.set_tot("XYZ")?;
    /// let report = msg.validate_profile(Profile::IntI503);
    /// let tot = report.findings.iter().find(|f| f.rule == Rule::TransactionType).unwrap();
    /// assert_eq!(tot.profile, Some(Profile::IntI503));
    /// assert_eq!(tot.to_string(), "INT-I 5.03: 1.004: XYZ is not a type of transaction of INT-I 5.03");
    /// # Ok::<(), ansi_nist_itl::Error>(())
    /// ```
    pub fn validate_profile(&self, profile: Profile) -> Validation {
        let mut v = match profile {
            Profile::Ebts112 => {
                let mut v = Validator::new(Some(Standard::Update2015));
                for (index, record) in self.records().iter().enumerate() {
                    v.record(Standard::Update2015, index, record);
                }
                v
            }
            Profile::IntI503 => Validator::new(None),
        };
        v.content(self);
        v.profile = Some(profile);
        match profile {
            Profile::Ebts112 => ebts::check(&mut v, self),
            Profile::IntI503 => inti::check(&mut v, self),
        }
        v.into_validation()
    }

    /// Validates the message against the tables of its version (1.002);
    /// without tables for it, only the CNT is checked, with a
    /// [`Rule::Version`] warning.
    ///
    /// ```
    /// use ansi_nist_itl::Message;
    ///
    /// let report = Message::new().validate();
    /// assert_eq!(report.standard, None);
    /// ```
    pub fn validate(&self) -> Validation {
        let version = self
            .type1()
            .and_then(|r| r.get("VER").ok())
            .map(|v| v.to_string())
            .unwrap_or_default();
        match Standard::for_version(&version) {
            Some(standard) => self.validate_against(standard),
            None => {
                let mut v = Validator::new(None);
                v.push(
                    Rule::Version,
                    Level::Warning,
                    Here::field(0, 1, 2),
                    format!("no tables for version {version}"),
                );
                v.content(self);
                v.into_validation()
            }
        }
    }

    /// Validates the message against the tables of `standard`, whatever
    /// its version.
    ///
    /// ```
    /// use ansi_nist_itl::dictionary::Standard;
    /// use ansi_nist_itl::validation::Rule;
    /// use ansi_nist_itl::Message;
    ///
    /// let report = Message::new().validate_against(Standard::Update2015);
    /// assert!(report.findings.iter().all(|f| f.rule != Rule::Version));
    /// ```
    pub fn validate_against(&self, standard: Standard) -> Validation {
        let mut v = Validator::new(Some(standard));
        for (index, record) in self.records().iter().enumerate() {
            v.record(standard, index, record);
        }
        v.content(self);
        v.into_validation()
    }
}

/// Where a finding is.
#[derive(Clone, Copy)]
struct Here {
    index: usize,
    record_type: u32,
    tag: Option<u32>,
    subfield: Option<usize>,
    item: Option<usize>,
}

impl Here {
    fn record(index: usize, record_type: u32) -> Self {
        Here {
            index,
            record_type,
            tag: None,
            subfield: None,
            item: None,
        }
    }

    fn field(index: usize, record_type: u32, tag: u32) -> Self {
        Here {
            tag: Some(tag),
            ..Here::record(index, record_type)
        }
    }

    fn subfield(self, s: usize) -> Self {
        Here {
            subfield: Some(s),
            ..self
        }
    }

    fn item(self, i: usize) -> Self {
        Here {
            item: Some(i),
            ..self
        }
    }
}

struct Validator {
    standard: Option<Standard>,
    /// The profile whose rules are being checked.
    profile: Option<Profile>,
    findings: Vec<Finding>,
}

impl Validator {
    fn new(standard: Option<Standard>) -> Self {
        Validator {
            standard,
            profile: None,
            findings: Vec::new(),
        }
    }

    fn into_validation(self) -> Validation {
        Validation {
            standard: self.standard,
            findings: self.findings,
        }
    }

    fn push(&mut self, rule: Rule, level: Level, here: Here, message: String) {
        self.findings.push(Finding {
            rule,
            level,
            position: Position {
                record_index: Some(here.index),
                record_type: Some(here.record_type),
                tag: here.tag.map(PyInt::from),
                ..Position::default()
            },
            subfield: here.subfield,
            item: here.item,
            profile: self.profile,
            message,
        });
    }

    fn error(&mut self, rule: Rule, here: Here, message: String) {
        self.push(rule, Level::Error, here, message);
    }

    fn record(&mut self, standard: Standard, index: usize, record: &Record) {
        let t = record.record_type();
        let here = Here::record(index, t);
        if matches!(t, 3 | 5 | 6) {
            let message = format!("record type {t} is deprecated");
            self.push(Rule::RecordType, Level::Warning, here, message);
            return;
        }
        if !standard.record_types().contains(&t) {
            self.error(
                Rule::RecordType,
                here,
                format!("{standard} has no record type {t}"),
            );
            return;
        }
        // Binary records: their headers are checked when read and written.
        let Some(r) = record.as_ascii() else {
            return;
        };
        if t == 1 {
            self.type1_ascii(index, r);
        }
        for field in r.fields() {
            let Some(tag) = field.tag().to_i64().and_then(|n| u32::try_from(n).ok()) else {
                self.error(
                    Rule::UndefinedField,
                    here,
                    format!("field number {} out of range", field.tag()),
                );
                continue;
            };
            let here = Here::field(index, t, tag);
            let Some(def) = standard.field(t, tag) else {
                self.error(
                    Rule::UndefinedField,
                    here,
                    format!("{standard} has no field {t}.{tag:03}"),
                );
                continue;
            };
            self.field(def, field, here);
        }
        for def in standard.fields(t) {
            let mandatory = def.condition == Condition::Mandatory
                && def.kind != FieldKind::Header
                && matches!(def.occurrence, Some(Occurrence { min: Limit::Count(n), .. }) if n > 0);
            if mandatory && r.field(def.tag).is_none() {
                let name = if def.mnemonic.is_empty() {
                    def.name
                } else {
                    def.mnemonic
                };
                self.error(
                    Rule::MissingField,
                    Here::field(index, t, def.tag),
                    format!("mandatory field {name} is absent"),
                );
            }
        }
    }

    fn type1_ascii(&mut self, index: usize, r: &AsciiRecord) {
        for field in r.fields() {
            let AnyField::Text(f) = field else { continue };
            let texts = subfields(&f.value()).unwrap_or_default();
            if let Some(c) = texts
                .iter()
                .flatten()
                .flat_map(|t| t.chars())
                .find(|c| !c.is_ascii())
            {
                let tag = field
                    .tag()
                    .to_i64()
                    .and_then(|n| u32::try_from(n).ok())
                    .unwrap_or(0);
                self.error(
                    Rule::Type1Ascii,
                    Here::field(index, 1, tag),
                    format!("character U+{:04X} is not 7-bit ASCII", u32::from(c)),
                );
            }
        }
    }

    fn field(&mut self, def: &FieldDef, field: &AnyField, here: Here) {
        match def.kind {
            FieldKind::Header | FieldKind::UserDefined => {}
            FieldKind::Reserved => {
                self.error(
                    Rule::ReservedField,
                    here,
                    "field reserved for future use".into(),
                );
            }
            FieldKind::Deprecated => {
                let message = "field kept for legacy data only".into();
                self.push(Rule::DeprecatedField, Level::Warning, here, message);
            }
            FieldKind::Value(format) => match field {
                // Binary data (field 999).
                AnyField::Binary(_) => {}
                AnyField::Text(f) => match subfields(&f.value()) {
                    Some(sfs) if sfs.len() == 1 && sfs[0].len() == 1 => {
                        self.value(&sfs[0][0], format, Condition::Mandatory, here);
                    }
                    _ => self.error(Rule::Structure, here, "expected a single value".into()),
                },
            },
            FieldKind::Subfields(groups) => match field {
                AnyField::Binary(_) => {
                    self.error(
                        Rule::Structure,
                        here,
                        "expected subfields, found binary data".into(),
                    );
                }
                AnyField::Text(f) => match subfields(&f.value()) {
                    Some(sfs) => self.subfields(groups, &sfs, here),
                    None => self.error(Rule::Structure, here, "items nested in items".into()),
                },
            },
        }
    }

    fn subfields(&mut self, groups: &[SubfieldGroup], sfs: &[Vec<String>], here: Here) {
        let mut next = 0;
        for (g, group) in groups.iter().enumerate() {
            let left = sfs.len() - next;
            let last = g + 1 == groups.len();
            let count = match group.occurrence.max {
                Limit::Count(n) if !last => left.min(n as usize),
                _ => left,
            };
            if let Limit::Count(min) = group.occurrence.min {
                if count < min as usize {
                    self.error(
                        Rule::SubfieldCount,
                        here,
                        format!("{count} subfields, at least {min} expected"),
                    );
                }
            }
            if let (true, Limit::Count(max)) = (last, group.occurrence.max) {
                if count > max as usize {
                    self.error(
                        Rule::SubfieldCount,
                        here,
                        format!("{count} subfields, at most {max} expected"),
                    );
                }
            }
            for (s, items) in sfs.iter().enumerate().skip(next).take(count) {
                self.subfield(group, items, here.subfield(s));
            }
            next += count;
        }
    }

    fn subfield(&mut self, group: &SubfieldGroup, items: &[String], here: Here) {
        if let Some(format) = group.value {
            match items {
                [one] => self.value(one, format, Condition::Mandatory, here),
                _ => self.error(Rule::Structure, here, "expected a single value".into()),
            }
            return;
        }
        if items.len() > group.items.len() {
            self.error(
                Rule::ItemCount,
                here,
                format!(
                    "{} items, at most {} expected",
                    items.len(),
                    group.items.len()
                ),
            );
        }
        for (i, def) in group.items.iter().enumerate() {
            let here = here.item(i);
            match items.get(i).filter(|t| !t.is_empty()) {
                Some(text) => self.value(text, def.format, def.condition, here),
                None if def.condition == Condition::Mandatory => {
                    let name = if def.mnemonic.is_empty() {
                        def.name
                    } else {
                        def.mnemonic
                    };
                    self.error(
                        Rule::MissingItem,
                        here,
                        format!("mandatory item {name} is empty"),
                    );
                }
                None => {}
            }
        }
    }

    fn value(&mut self, text: &str, format: Format, condition: Condition, here: Here) {
        if text.is_empty() && condition != Condition::Mandatory {
            return;
        }
        let size = text.chars().count();
        let date = match format.char_type {
            CharType::LocalDate => Some(local_date(text, 8)),
            CharType::LocalDateTime => Some(text.len() == 14 && local_date(text, 14)),
            CharType::Gmt => Some(
                text.len() == 15
                    && text.ends_with('Z')
                    && text.get(..14).is_some_and(|d| local_date(d, 14)),
            ),
            _ => None,
        };
        if let Some(ok) = date {
            if !ok {
                let what = match format.char_type {
                    CharType::LocalDate => "a date YYYYMMDD",
                    CharType::LocalDateTime => "a date and time YYYYMMDDhhmmss",
                    _ => "a GMT YYYYMMDDhhmmssZ",
                };
                self.error(Rule::Date, here, format!("{text:?} is not {what}"));
            }
            return;
        }
        if let Some(c) = text.chars().find(|&c| !allowed(format.char_type, c)) {
            self.error(
                Rule::CharacterType,
                here,
                format!("character {c:?} is not of type {}", format.char_type.code()),
            );
        }
        if let Some(min) = format.min.filter(|&m| size < m as usize) {
            self.error(
                Rule::Size,
                here,
                format!("{size} characters, at least {min} expected"),
            );
        }
        if let Some(max) = format.max.filter(|&m| size > m as usize) {
            self.error(
                Rule::Size,
                here,
                format!("{size} characters, at most {max} expected"),
            );
        }
    }

    /// CNT (1.003): its first subfield counts the records, then one pair
    /// (type, IDC) per record, in order. An empty CNT, not computed yet,
    /// is not checked.
    fn content(&mut self, msg: &Message) {
        let Some(AnyField::Text(cnt)) = msg.type1().and_then(|r| r.field(3)) else {
            return;
        };
        let here = Here::field(0, 1, 3);
        let Some(sfs) = subfields(&cnt.value()) else {
            self.error(Rule::Content, here, "CNT holds nested items".into());
            return;
        };
        // Empty until the message is written, which computes it.
        if sfs.iter().flatten().all(|t| t.is_empty()) {
            return;
        }
        let records = msg.records();
        let expected: Vec<Vec<String>> =
            std::iter::once(vec!["1".to_owned(), (records.len() - 1).to_string()])
                .chain(records.iter().skip(1).map(|r| {
                    let idc = match r {
                        Record::Ascii(a) => a.idc().map(|v| v.to_string()).unwrap_or_default(),
                        Record::Binary(b) => b.idc().to_string(),
                    };
                    vec![r.record_type().to_string(), idc]
                }))
                .collect();
        let normalize = |sf: &Vec<String>| -> Vec<String> {
            sf.iter()
                .map(|s| {
                    let trimmed = s.trim_start_matches('0');
                    if trimmed.is_empty() && !s.is_empty() {
                        "0".into()
                    } else {
                        trimmed.into()
                    }
                })
                .collect()
        };
        if sfs.len() != expected.len() {
            self.error(
                Rule::Content,
                here,
                format!("{} subfields for {} records", sfs.len(), records.len()),
            );
        }
        for (s, (got, want)) in sfs.iter().zip(&expected).enumerate() {
            if normalize(got) != normalize(want) {
                self.error(
                    Rule::Content,
                    here.subfield(s),
                    format!("{} where the message has {}", got.join(" "), want.join(" ")),
                );
            }
        }
    }
}

/// A field as subfields of items, as written: a value is one subfield of
/// one item. `None` for items nested in items, which have no writing.
fn subfields(value: &Value) -> Option<Vec<Vec<String>>> {
    match value {
        Value::Scalar(s) => Some(vec![vec![s.to_string()]]),
        Value::List(elems) => elems
            .iter()
            .map(|e| match e {
                ListElem::Scalar(s) => Some(vec![s.to_string()]),
                ListElem::List(items) => items
                    .iter()
                    .map(|i| match i {
                        Item::Scalar(s) => Some(s.to_string()),
                        Item::List(_) => None,
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// A character of a character type (section 5.5). The special characters
/// of `ANS`, `AS` and `NS` are those the field allows: any printable ASCII
/// character is accepted.
fn allowed(t: CharType, c: char) -> bool {
    let special = c.is_ascii_graphic() && !c.is_ascii_alphanumeric() || c == ' ';
    match t {
        CharType::Alphabetic => c.is_ascii_alphabetic(),
        CharType::Alphanumeric => c.is_ascii_alphanumeric(),
        CharType::AlphanumericSpecial => c.is_ascii_alphanumeric() || special,
        CharType::AlphabeticSpecial => c.is_ascii_alphabetic() || special,
        CharType::Numeric => c.is_ascii_digit(),
        CharType::NumericSpecial => c.is_ascii_digit() || special,
        CharType::Hexadecimal => c.is_ascii_digit() || ('A'..='F').contains(&c),
        CharType::Base64 => c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='),
        _ => true,
    }
}

/// `YYYYMMDD` (`len` 8) or `YYYYMMDDhhmmss` (`len` 14). A date may end
/// with zeros when only the year, or year and month, are known (Annex
/// B.1.4).
fn local_date(text: &str, len: usize) -> bool {
    if text.len() != len || !text.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let n = |range: std::ops::Range<usize>| {
        text.get(range)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(99)
    };
    let (year, month, day) = (n(0..4), n(4..6), n(6..8));
    let date = match (month, day) {
        (0, 0) => true,
        (1..=12, 0) => true,
        (1..=12, d) => d <= days_in_month(year, month),
        _ => false,
    };
    date && (len == 8 || n(8..10) <= 23 && n(10..12) <= 59 && n(12..14) <= 59)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert!(local_date("20240229", 8));
        assert!(!local_date("20230229", 8));
        assert!(!local_date("21000229", 8));
        assert!(local_date("20000229", 8));
        assert!(local_date("20230000", 8) && local_date("20231100", 8));
        assert!(!local_date("20230015", 8) && !local_date("20231301", 8));
        assert!(local_date("20230115235959", 14));
        assert!(!local_date("20230115240000", 14));
        assert!(!local_date("2023011", 8) && !local_date("2023-1-1", 8));
    }

    #[test]
    fn characters() {
        assert!(allowed(CharType::Hexadecimal, 'F') && !allowed(CharType::Hexadecimal, 'f'));
        assert!(allowed(CharType::NumericSpecial, '.') && !allowed(CharType::NumericSpecial, 'a'));
        assert!(
            allowed(CharType::AlphanumericSpecial, ' ')
                && !allowed(CharType::AlphanumericSpecial, '\u{e9}')
        );
        assert!(allowed(CharType::Unicode, '\u{e9}'));
        assert!(!allowed(CharType::Alphabetic, ' '));
    }
}
