//! Profile EBTS 11.2 of the FBI: the rules of Appendix B for the Type-1, of
//! Tables L-2 and L-3 for the records of each transaction, of Tables D-1,
//! E-1 and I-1 for the Type-2 fields of each transaction, of Table L-5 for
//! the fields allocated to other agencies, and of Table C-1 for the format
//! of the Type-2 fields.
//!
//! A cell of D-1, E-1 or I-1 with a reference note states a condition the
//! crate does not check: a finding on it is a warning.

use super::ebts112::{ALLOCATIONS, FIELDS, TOTS};
use super::{Here, Level, Rule, Validator, subfields};
use crate::dictionary::{CharType, Condition, Format};
use crate::field::AnyField;
use crate::message::Message;
use crate::record::{AsciiRecord, Record};

/// A type of transaction.
pub(super) struct Tot {
    pub(super) code: &'static str,
    pub(super) name: &'static str,
    /// Listed in D-1, E-1 or I-1.
    pub(super) type2: bool,
    /// Record type, minimum, maximum (`None`: unbounded).
    pub(super) records: &'static [(u32, u32, Option<u32>)],
    /// Type-2 field, minimum and maximum occurrences, reference notes.
    pub(super) fields: &'static [(u32, u32, Option<u32>, &'static [&'static str])],
}

/// A Type-2 field of Table C-1.
pub(super) struct Type2Field {
    pub(super) tag: u32,
    pub(super) mnemonic: &'static str,
    pub(super) name: &'static str,
    /// Format of a single value; `None` for a set of items.
    pub(super) format: Option<Format>,
    pub(super) max_occurrences: Option<u32>,
    pub(super) items: &'static [Type2Item],
}

/// An item of a set of Table C-1.
pub(super) struct Type2Item {
    pub(super) mnemonic: &'static str,
    pub(super) format: Format,
    pub(super) mandatory: bool,
}

/// The text of a field, as one value.
fn text(r: &AsciiRecord, tag: u32) -> Option<String> {
    match r.field(tag)? {
        AnyField::Text(f) => Some(f.value().to_string()),
        AnyField::Binary(_) => None,
    }
}

pub(super) fn check(v: &mut Validator, msg: &Message) {
    let Some(t1) = msg.type1() else {
        return;
    };
    type1(v, t1);
    let code = text(t1, 4).unwrap_or_default();
    let tot = TOTS.iter().find(|t| t.code == code);
    if tot.is_none() {
        v.error(
            Rule::TransactionType,
            Here::field(0, 1, 4),
            format!("{code} is not a type of transaction of EBTS 11.2"),
        );
    }
    if let Some(tot) = tot {
        records(v, msg, tot);
    }
    for (index, record) in msg.records().iter().enumerate() {
        if let Some(r) = record.as_ascii().filter(|r| r.record_type() == 2) {
            if let Some(tot) = tot.filter(|t| t.type2) {
                type2_fields(v, index, r, tot);
            }
            type2_formats(v, index, r);
        }
    }
}

/// Appendix B: the Type-1 fields the FBI constrains.
fn type1(v: &mut Validator, t1: &AsciiRecord) {
    let fixed = |char_type, n| Format {
        char_type,
        min: Some(n),
        max: Some(n),
    };
    for (tag, format) in [
        (7, fixed(CharType::Alphanumeric, 9)),
        (8, fixed(CharType::Alphanumeric, 9)),
        (
            9,
            Format {
                char_type: CharType::AlphanumericSpecial,
                min: Some(10),
                max: Some(40),
            },
        ),
    ] {
        if let Some(value) = text(t1, tag) {
            v.value(&value, format, Condition::Mandatory, Here::field(0, 1, tag));
        }
    }
    for tag in [11, 12] {
        if let Some(value) = text(t1, tag) {
            if !resolution(&value) {
                v.error(
                    Rule::Format,
                    Here::field(0, 1, tag),
                    format!("{value:?} is not a resolution NN.NN"),
                );
            }
        }
    }
    match t1.field(13) {
        None => v.error(
            Rule::MissingField,
            Here::field(0, 1, 13),
            "DOM is mandatory: NORAM and the version of EBTS".into(),
        ),
        Some(AnyField::Text(f)) => {
            let items = subfields(&f.value()).unwrap_or_default();
            let version = items.first().and_then(|s| s.get(1)).map(String::as_str);
            if !version.is_some_and(ebts_version) {
                v.push(
                    Rule::Format,
                    Level::Warning,
                    Here::field(0, 1, 13),
                    format!(
                        "the second item should be the version of EBTS (EBTS #.#), not {:?}",
                        version.unwrap_or_default()
                    ),
                );
            }
        }
        Some(AnyField::Binary(_)) => {}
    }
}

/// `NN.NN`.
fn resolution(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5 && b[2] == b'.' && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit())
}

/// `EBTS #.#`.
fn ebts_version(s: &str) -> bool {
    s.strip_prefix("EBTS ").is_some_and(|v| {
        let mut parts = v.split('.');
        let ok = |p: Option<&str>| {
            p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        };
        ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
    })
}

fn range(lo: u32, hi: Option<u32>) -> String {
    match hi {
        Some(hi) if hi == lo => lo.to_string(),
        Some(hi) => format!("{lo} to {hi}"),
        None => format!("at least {lo}"),
    }
}

/// Tables L-2 and L-3: the records of each type the transaction holds.
fn records(v: &mut Validator, msg: &Message, tot: &Tot) {
    let mut types: Vec<u32> = msg.records().iter().map(Record::record_type).collect();
    types.sort_unstable();
    types.dedup();
    let count = |t: u32| {
        msg.records()
            .iter()
            .filter(|r| r.record_type() == t)
            .count()
    };
    let here = Here::field(0, 1, 4);
    for &(t, lo, hi) in tot.records {
        let n = count(t);
        if n < lo as usize || hi.is_some_and(|hi| n > hi as usize) {
            v.error(
                Rule::RecordCount,
                here,
                format!(
                    "{} ({}) holds {} records of type {t}, found {n}",
                    tot.code,
                    tot.name,
                    range(lo, hi)
                ),
            );
        }
    }
    for t in types {
        if !tot.records.iter().any(|&(rt, ..)| rt == t) {
            v.error(
                Rule::RecordCount,
                here,
                format!("{} holds no record of type {t}", tot.code),
            );
        }
    }
}

/// The agency a field number is allocated to, in Table L-5, when it is
/// not the FBI.
fn allocated(tag: u32) -> Option<&'static str> {
    ALLOCATIONS
        .iter()
        .find(|&&(lo, hi, user)| (lo..=hi).contains(&tag) && user != "FBI USE")
        .map(|&(.., user)| user)
}

/// Tables D-1, E-1 and I-1: the Type-2 fields of the transaction, and how
/// many times each one appears.
fn type2_fields(v: &mut Validator, index: usize, r: &AsciiRecord, tot: &Tot) {
    let level = |notes: &[&str]| {
        if notes.is_empty() {
            Level::Error
        } else {
            Level::Warning
        }
    };
    let noted = |notes: &[&str]| {
        if notes.is_empty() {
            String::new()
        } else {
            format!(" (reference note {})", notes.join(", "))
        }
    };
    for field in r.fields() {
        let Some(tag) = field.tag().to_i64().and_then(|n| u32::try_from(n).ok()) else {
            continue;
        };
        let here = Here::field(index, 2, tag);
        match tot.fields.iter().find(|&&(t, ..)| t == tag) {
            Some(&(_, lo, hi, notes)) => {
                let n = match field {
                    AnyField::Text(f) => subfields(&f.value()).map_or(1, |s| s.len()),
                    AnyField::Binary(_) => 1,
                };
                if n < lo as usize || hi.is_some_and(|hi| n > hi as usize) {
                    v.push(
                        Rule::SubfieldCount,
                        level(notes),
                        here,
                        format!(
                            "{} allows {} occurrences, found {n}{}",
                            tot.code,
                            range(lo, hi),
                            noted(notes)
                        ),
                    );
                }
            }
            None => match allocated(tag) {
                Some(user) => v.push(
                    Rule::FieldNotInTransaction,
                    Level::Warning,
                    here,
                    format!("field allocated to {user}, not checked"),
                ),
                None => v.error(
                    Rule::FieldNotInTransaction,
                    here,
                    format!("{} does not use this field", tot.code),
                ),
            },
        }
    }
    for &(tag, lo, _, notes) in tot.fields {
        if lo > 0 && r.field(tag).is_none() {
            let name = FIELDS
                .iter()
                .find(|f| f.tag == tag)
                .map_or("", |f| f.mnemonic);
            v.push(
                Rule::MissingField,
                level(notes),
                Here::field(index, 2, tag),
                format!("{} requires {name}{}", tot.code, noted(notes)),
            );
        }
    }
}

/// Table C-1: the format of each Type-2 field.
fn type2_formats(v: &mut Validator, index: usize, r: &AsciiRecord) {
    for field in r.fields() {
        let AnyField::Text(f) = field else { continue };
        let Some(tag) = f.tag().to_i64().and_then(|n| u32::try_from(n).ok()) else {
            continue;
        };
        let Some(def) = FIELDS.iter().find(|d| d.tag == tag) else {
            continue;
        };
        let here = Here::field(index, 2, tag);
        let Some(sfs) = subfields(&f.value()) else {
            v.error(Rule::Structure, here, "items nested in items".into());
            continue;
        };
        if let Some(max) = def.max_occurrences.filter(|&m| sfs.len() > m as usize) {
            v.error(
                Rule::SubfieldCount,
                here,
                format!(
                    "{} ({}) has at most {max} occurrences, found {}",
                    def.mnemonic,
                    def.name,
                    sfs.len()
                ),
            );
        }
        for (s, items) in sfs.iter().enumerate() {
            let here = here.subfield(s);
            match def.format {
                Some(format) => match items.as_slice() {
                    [one] => v.value(one, format, Condition::Mandatory, here),
                    _ => v.error(Rule::Structure, here, "expected a single value".into()),
                },
                None => {
                    if items.len() > def.items.len() {
                        v.error(
                            Rule::ItemCount,
                            here,
                            format!(
                                "{} items, at most {} expected",
                                items.len(),
                                def.items.len()
                            ),
                        );
                    }
                    for (i, item) in def.items.iter().enumerate() {
                        match items.get(i).filter(|t| !t.is_empty()) {
                            Some(t) => v.value(t, item.format, Condition::Mandatory, here.item(i)),
                            None if item.mandatory => v.error(
                                Rule::MissingItem,
                                here.item(i),
                                format!("mandatory item {} is empty", item.mnemonic),
                            ),
                            None => {}
                        }
                    }
                }
            }
        }
    }
}
