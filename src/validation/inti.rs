//! Profile INT-I 5.03 of INTERPOL (April 7, 2011), on ANSI/NIST-ITL 1-2007.
//!
//! Its tables are short and its text is precise where the tables are not
//! (Table 8 gives DAI, ORI and TCN as numeric, section 6.1 describes them
//! otherwise): the rules are written by hand from the document, each with
//! its section. The formats of the Type-2 fields are described in prose
//! (section 7) and are not checked.

use super::{Here, Level, Rule, Validator, local_date, subfields};
use crate::field::AnyField;
use crate::message::Message;
use crate::record::AsciiRecord;

/// Use of a record type in a transaction (Table 9).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Use {
    /// `M`: at least one.
    Mandatory,
    /// `O`.
    Optional,
    /// `O*`: at least one of the record types so marked.
    OneOf,
    /// `-`: not allowed.
    No,
}

use Use::{Mandatory as M, No as N, OneOf as S, Optional as O};

/// Columns of Table 9.
const TYPES: [u32; 10] = [1, 2, 4, 7, 8, 9, 10, 13, 14, 15];

/// A transaction of section 6.1.4 with its row of Table 9, and the
/// reference notes of the row: 1 (Type-4 and Type-7 latents are obsolete),
/// 2 (at least one of the noted types, and the same), 3 (Type-14 images
/// also sent as Type-4).
struct Tot {
    code: &'static str,
    name: &'static str,
    uses: [Use; 10],
    note1: bool,
    note2: bool,
    note3: bool,
}

const fn tot(code: &'static str, name: &'static str, uses: [Use; 10]) -> Tot {
    Tot {
        code,
        name,
        uses,
        note1: false,
        note2: false,
        note3: false,
    }
}

const fn notes(t: Tot, note1: bool, note2: bool, note3: bool) -> Tot {
    Tot {
        note1,
        note2,
        note3,
        ..t
    }
}

/// Section 6.1.4 (names, as written) and Table 9 (record types).
const TOTS: [Tot; 23] = [
    tot("IRQ", "Image Request", [M, M, N, N, N, N, N, N, N, N]),
    notes(
        tot("IMR", "Image Response", [M, M, S, S, O, N, N, S, O, S]),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "CPS",
            "Criminal Print-to-Print Search",
            [M, M, O, O, O, N, N, N, O, N],
        ),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "NPS",
            "Non-Criminal Print-to-Print Search",
            [M, M, O, O, O, N, N, O, O, N],
        ),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "MPS",
            "Latent-to-Print Search",
            [M, M, O, O, N, O, N, O, N, N],
        ),
        true,
        false,
        false,
    ),
    notes(
        tot(
            "PMS",
            "Print-to-Latent Search",
            [M, M, O, O, O, N, N, N, O, O],
        ),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "MMS",
            "Latent-to-Latent Search",
            [M, M, O, O, N, O, N, O, N, N],
        ),
        true,
        false,
        false,
    ),
    tot("DBS", "Database Search", [M, M, N, N, N, N, N, N, N, N]),
    notes(
        tot("SRE", "Search Results", [M, M, O, O, O, N, O, O, O, O]),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "USA",
            "Add Latent to Unidentified Latents Collection",
            [M, M, O, O, N, N, N, O, N, N],
        ),
        false,
        true,
        false,
    ),
    tot(
        "USR",
        "Remove Latent from Unidentified Latents Collection",
        [M, M, N, N, N, N, N, N, N, N],
    ),
    notes(
        tot(
            "ATP",
            "Add To Print Collection",
            [M, M, S, O, O, N, N, N, O, S],
        ),
        false,
        false,
        true,
    ),
    notes(
        tot(
            "SUP",
            "Substitute Print(s) Into Existing Print-Collection",
            [M, M, S, N, N, N, N, N, O, S],
        ),
        false,
        false,
        true,
    ),
    tot(
        "DFP",
        "Delete From Print Collection",
        [M, M, N, N, N, N, N, N, N, N],
    ),
    tot(
        "DIP",
        "Disregard Individual Print(s) Update",
        [M, M, N, N, N, N, N, N, N, N],
    ),
    tot(
        "CPR",
        "Criminal Subject Photo Request",
        [M, M, N, N, N, N, N, N, N, N],
    ),
    tot("PHR", "Photo Response", [M, M, N, N, N, N, M, N, N, N]),
    // Section 6.1.4 names it "Add To Print Collection", for photos.
    notes(
        tot(
            "APC",
            "Add To Print Collection",
            [M, M, O, O, O, N, M, N, O, O],
        ),
        false,
        false,
        true,
    ),
    tot(
        "DPC",
        "Delete From Photo Collection",
        [M, M, N, N, N, N, N, N, N, N],
    ),
    tot(
        "CPP",
        "Criminal Photo-to-Photo Search",
        [M, M, N, N, N, N, M, N, N, N],
    ),
    notes(
        tot("UPR", "Update Request", [M, M, O, O, O, O, O, O, O, O]),
        true,
        false,
        true,
    ),
    tot(
        "NPP",
        "Non-Criminal Photo-to-Photo Search",
        [M, M, N, N, N, N, M, N, N, N],
    ),
    tot("ERR", "Error Message", [M, M, N, N, N, N, N, N, N, N]),
];

/// The scalar text of a Type-1 field.
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
    match TOTS.iter().find(|t| t.code == code) {
        Some(tot) => records(v, msg, tot),
        None => v.error(
            Rule::TransactionType,
            Here::field(0, 1, 4),
            format!("{code} is not a type of transaction of INT-I 5.03"),
        ),
    }
    let mut first = true;
    for (index, record) in msg.records().iter().enumerate() {
        if let Some(r) = record.as_ascii().filter(|r| r.record_type() == 2) {
            type2(v, index, r, first);
            first = false;
        }
    }
}

/// Section 6.1: the Type-1 fields.
fn type1(v: &mut Validator, t1: &AsciiRecord) {
    // "The character set used for Type-1 fields shall contain only the
    // 7-bit ANSI code" (6.1).
    v.type1_ascii(0, t1);
    let mut format = |tag: u32, ok: bool, what: &str, value: &str| {
        if !ok {
            v.error(
                Rule::Format,
                Here::field(0, 1, tag),
                format!("{value:?} is not {what}"),
            );
        }
    };
    if let Some(ver) = text(t1, 2) {
        format(
            2,
            ver == "0400",
            "version 0400 (ANSI/NIST-ITL 1-2007, 6.1.2)",
            &ver,
        );
    }
    if let Some(dat) = text(t1, 5) {
        format(5, local_date(&dat, 8), "a date YYYYMMDD (6.1.5)", &dat);
    }
    if let Some(pry) = text(t1, 6) {
        let ok = pry.len() == 1 && pry.bytes().all(|b| (b'1'..=b'9').contains(&b));
        format(6, ok, "a priority from 1 to 9 (6.1.6)", &pry);
    }
    for tag in [7, 8] {
        if let Some(agency) = text(t1, tag) {
            format(
                tag,
                agency_identifier(&agency),
                "CC/agency (6.1.7)",
                &agency,
            );
        }
    }
    for tag in [9, 10] {
        if let Some(tcn) = text(t1, tag) {
            format(
                tag,
                control_number(&tcn),
                "YYSSSSSSSSA with its check character (6.1.9, Appendix 2)",
                &tcn,
            );
        }
    }
    for tag in [11, 12] {
        if let Some(res) = text(t1, tag) {
            let b = res.as_bytes();
            let ok = b.len() == 5
                && b.get(2) == Some(&b'.')
                && [0, 1, 3, 4]
                    .iter()
                    .all(|&i| b.get(i).is_some_and(u8::is_ascii_digit));
            format(tag, ok, "a resolution NN.NN (6.1.11)", &res);
        }
    }
    if let Some(gmt) = text(t1, 14) {
        let ok = gmt.len() == 15
            && gmt.ends_with('Z')
            && gmt.get(..14).is_some_and(|d| local_date(d, 14));
        format(14, ok, "a GMT YYYYMMDDhhmmssZ (6.1.14)", &gmt);
    }
}

/// `CC/agency`: a country code of two characters, then up to 32 (6.1.7).
fn agency_identifier(s: &str) -> bool {
    let Some((cc, agency)) = s.split_once('/') else {
        return false;
    };
    cc.len() == 2
        && cc.bytes().all(|b| b.is_ascii_alphanumeric())
        && (1..=32).contains(&agency.len())
        && agency.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
}

/// `YYSSSSSSSSA`: the check character is `YYSSSSSSSS` modulo 23, through
/// the table of Appendix 2 (`0` is `Z`, `1` to `22` are the letters
/// without `I`, `O` and `S`).
fn control_number(s: &str) -> bool {
    const LETTERS: &[u8; 23] = b"ZABCDEFGHJKLMNPQRTUVWXY";
    if s.len() != 11 || !s.is_ascii() {
        return false;
    }
    let (digits, check) = s.split_at(10);
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let n: u64 = digits.parse().unwrap_or(u64::MAX);
    LETTERS
        .get((n % 23) as usize)
        .is_some_and(|&c| check.as_bytes() == [c])
}

/// Table 9: the records of each type the transaction holds.
fn records(v: &mut Validator, msg: &Message, tot: &Tot) {
    let count = |t: u32| {
        msg.records()
            .iter()
            .filter(|r| r.record_type() == t)
            .count()
    };
    let here = Here::field(0, 1, 4);
    let mut one_of = Vec::new();
    for (&t, &u) in TYPES.iter().zip(&tot.uses) {
        let n = count(t);
        match u {
            Use::Mandatory if n == 0 => v.error(
                Rule::RecordCount,
                here,
                format!("{} ({}) requires a record of type {t}", tot.code, tot.name),
            ),
            Use::No if n > 0 => v.error(
                Rule::RecordCount,
                here,
                format!("{} ({}) allows no record of type {t}", tot.code, tot.name),
            ),
            Use::OneOf => one_of.push((t, n)),
            _ => {}
        }
    }
    if tot.note2 {
        one_of.extend([4, 7, 13].map(|t| (t, count(t))));
    }
    if !one_of.is_empty() && one_of.iter().all(|&(_, n)| n == 0) {
        let types: Vec<String> = one_of.iter().map(|(t, _)| t.to_string()).collect();
        v.error(
            Rule::RecordCount,
            here,
            format!(
                "{} requires a record of one of the types {}",
                tot.code,
                types.join(", ")
            ),
        );
    }
    if (tot.note1 || tot.note2) && (count(4) > 0 || count(7) > 0) {
        v.push(
            Rule::RecordCount,
            Level::Warning,
            here,
            format!(
                "{}: Type-4 and Type-7 latents are obsolete, use Type-13 (Table 9, note 1)",
                tot.code
            ),
        );
    }
    if tot.note3 && count(14) > 0 && count(4) == 0 {
        v.error(
            Rule::RecordCount,
            here,
            format!(
                "{}: Type-14 images must also be sent as Type-4 records (Table 9, note 3)",
                tot.code
            ),
        );
    }
}

/// Section 7: fields 2.001 to 2.003 are mandatory, 2.086 to 2.199
/// reserved; the first Type-2 record holds 7-bit ASCII only.
fn type2(v: &mut Validator, index: usize, r: &AsciiRecord, first: bool) {
    for tag in 1..=3 {
        if r.field(tag).is_none() {
            v.error(
                Rule::MissingField,
                Here::field(index, 2, tag),
                "fields 2.001 to 2.003 are mandatory (7.1)".into(),
            );
        }
    }
    for field in r.fields() {
        let Some(tag) = field.tag().to_i64().and_then(|n| u32::try_from(n).ok()) else {
            continue;
        };
        let here = Here::field(index, 2, tag);
        if (86..=199).contains(&tag) {
            v.error(
                Rule::ReservedField,
                here,
                "fields 2.086 to 2.199 are reserved for INT-I (7)".into(),
            );
        }
        if !first {
            continue;
        }
        let AnyField::Text(f) = field else { continue };
        let texts = subfields(&f.value()).unwrap_or_default();
        if let Some(c) = texts
            .iter()
            .flatten()
            .flat_map(|t| t.chars())
            .find(|c| !c.is_ascii())
        {
            v.error(
                Rule::CharacterType,
                here,
                format!(
                    "character U+{:04X}: the first Type-2 record holds 7-bit ASCII only (7)",
                    u32::from(c)
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_character() {
        // Computed apart: 1100000001 % 23 = 0 (Z), 1100000002 % 23 = 1 (A),
        // 2312345681 % 23 = 17 (T: after R, no S).
        assert!(control_number("0000000000Z"));
        assert!(control_number("1100000001Z"));
        assert!(control_number("1100000002A"));
        assert!(control_number("2312345681T"));
        assert!(!control_number("2312345681S"));
        assert!(!control_number("110000000Z") && !control_number("11000000011Z"));
        assert!(!control_number("é10000001Z"));
    }

    #[test]
    fn agencies() {
        assert!(agency_identifier("ZZ/ALL") && agency_identifier("FR/SDPTS LYON"));
        assert!(
            !agency_identifier("FRA/X") && !agency_identifier("FR/") && !agency_identifier("FR")
        );
    }

    #[test]
    fn table_9() {
        // At most one O* group per transaction, and notes on the right columns.
        for t in &TOTS {
            assert_eq!(t.uses[0], Use::Mandatory, "{}", t.code);
            assert_eq!(t.uses[1], Use::Mandatory, "{}", t.code);
            if t.note3 {
                assert_ne!(t.uses[8], Use::No, "{}", t.code);
            }
        }
        let codes: Vec<&str> = TOTS.iter().map(|t| t.code).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 23);
    }
}
