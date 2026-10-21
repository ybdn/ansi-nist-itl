//! Field dictionary (LIB-15, SPEC §13.6): structure of the tables, and
//! values checked by hand against the layout tables of the standard.

#![cfg(feature = "dictionary")]

use ansi_nist_itl::dictionary::{
    CharType, Condition, FieldKind, Format, Limit, Occurrence, Standard,
};

const STD: Standard = Standard::Update2015;

#[test]
fn record_types() {
    assert_eq!(
        STD.record_types(),
        [
            1, 2, 4, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 98, 99
        ]
    );
}

/// Every record type starts with its header and covers its numbers without
/// gap nor overlap; tagged types go up to 999.
#[test]
fn ranges_cover_each_type() {
    for &t in STD.record_types() {
        let fields = STD.fields(t);
        assert_eq!(fields[0].kind, FieldKind::Header, "{t}");
        let mut next = 1;
        for f in fields {
            assert_eq!(f.record_type, t);
            assert_eq!(f.tag, next, "{t}.{:03}", f.tag);
            assert!(f.last_tag >= f.tag);
            next = f.last_tag + 1;
        }
        // Binary records have a fixed list; the Type-1 ends at 1.018.
        let end = match t {
            1 => 19,
            4 => 10,
            7 => 4,
            8 => 9,
            _ => 1000,
        };
        assert_eq!(next, end, "{t}");
        for tag in 1..next {
            assert!(STD.field(t, tag).is_some(), "{t}.{tag:03}");
        }
    }
}

/// Within a record type, a mnemonic names one field.
#[test]
fn mnemonics_are_unique() {
    for &t in STD.record_types() {
        let mut seen: Vec<&str> = STD
            .fields(t)
            .iter()
            .map(|f| f.mnemonic)
            .filter(|m| !m.is_empty())
            .collect();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n, "{t}");
    }
}

/// Fields with content have an occurrence; the others none.
#[test]
fn occurrences() {
    for &t in STD.record_types() {
        for f in STD.fields(t) {
            let described = !matches!(
                f.kind,
                FieldKind::Reserved | FieldKind::UserDefined | FieldKind::Deprecated
            );
            assert_eq!(f.occurrence.is_some(), described, "{t}.{:03}", f.tag);
            if let FieldKind::Subfields(groups) = f.kind {
                assert!(!groups.is_empty(), "{t}.{:03}", f.tag);
                for g in groups {
                    assert!(g.value.is_some() != !g.items.is_empty(), "{t}.{:03}", f.tag);
                }
            }
        }
    }
}

fn value(t: u32, tag: u32) -> Format {
    match STD.field(t, tag).unwrap().kind {
        FieldKind::Value(f) => f,
        kind => panic!("{t}.{tag:03}: {kind:?}"),
    }
}

fn once(min: u32) -> Option<Occurrence> {
    Some(Occurrence {
        min: Limit::Count(min),
        max: Limit::Count(1),
    })
}

/// Read by hand in Table 34 (Type-1).
#[test]
fn type_1() {
    let tot = STD.field(1, 4).unwrap();
    assert_eq!(
        (tot.mnemonic, tot.condition, tot.occurrence),
        ("TOT", Condition::Mandatory, once(1))
    );
    assert_eq!(
        value(1, 4),
        Format {
            char_type: CharType::Alphabetic,
            min: Some(1),
            max: Some(16)
        }
    );
    assert_eq!(value(1, 5).char_type, CharType::LocalDate);
    assert_eq!(
        value(1, 11),
        Format {
            char_type: CharType::NumericSpecial,
            min: Some(5),
            max: Some(5)
        }
    );
    assert_eq!(value(1, 14).char_type, CharType::Gmt);
    let pry = STD.field(1, 6).unwrap();
    assert_eq!(
        (pry.mnemonic, pry.condition, pry.occurrence),
        ("PRY", Condition::Optional, once(0))
    );
    let FieldKind::Subfields(cnt) = STD.field(1, 3).unwrap().kind else {
        panic!()
    };
    let names: Vec<Vec<&str>> = cnt
        .iter()
        .map(|g| g.items.iter().map(|i| i.mnemonic).collect())
        .collect();
    assert_eq!(names, [["FRC", "CRC"], ["REC", "IDC"]]);
    assert_eq!(STD.field(1, 18).unwrap().mnemonic, "GNS");
    assert!(STD.field(1, 19).is_none());
}

/// Read by hand in Table 36 (Type-4, binary).
#[test]
fn type_4() {
    let mnemonics: Vec<&str> = STD.fields(4).iter().map(|f| f.mnemonic).collect();
    assert_eq!(
        mnemonics,
        [
            "LEN", "IDC", "IMP", "FGP", "ISR", "HLL", "VLL", "CGA", "DATA"
        ]
    );
    assert_eq!(
        value(4, 6),
        Format {
            char_type: CharType::Binary,
            min: Some(2),
            max: Some(2)
        }
    );
    assert_eq!(
        STD.field(4, 4).unwrap().occurrence,
        Some(Occurrence {
            min: Limit::Count(6),
            max: Limit::Count(6)
        })
    );
}

/// Read by hand in Table 104 (Type-14).
#[test]
fn type_14() {
    assert_eq!(
        value(14, 2),
        Format {
            char_type: CharType::Numeric,
            min: Some(1),
            max: Some(2)
        }
    );
    assert_eq!(
        value(14, 4),
        Format {
            char_type: CharType::Unicode,
            min: Some(1),
            max: None
        }
    );
    assert_eq!(
        value(14, 11),
        Format {
            char_type: CharType::Alphanumeric,
            min: Some(3),
            max: Some(5)
        }
    );
    let ppc = STD.field(14, 15).unwrap();
    assert_eq!(
        (ppc.condition, ppc.occurrence),
        (Condition::Dependent, once(0))
    );
    let FieldKind::Subfields(groups) = ppc.kind else {
        panic!()
    };
    assert_eq!(
        groups[0].occurrence,
        Occurrence {
            min: Limit::Count(1),
            max: Limit::Count(12)
        }
    );
    let los = &groups[0].items[1];
    assert_eq!(
        (los.mnemonic, los.format.char_type),
        ("LOS", CharType::Alphabetic)
    );
    let FieldKind::Subfields(aseg) = STD.field(14, 25).unwrap().kind else {
        panic!()
    };
    let hpo = aseg[0].items.iter().find(|i| i.mnemonic == "HPO").unwrap();
    assert_eq!(hpo.occurrence.max, Limit::DependsOn("NOP"));
    assert_eq!(STD.field(14, 19).unwrap().kind, FieldKind::Reserved);
    assert_eq!(STD.field(14, 999).unwrap().mnemonic, "DATA");
}

#[test]
fn ranges() {
    let udf = STD.field(17, 500).unwrap();
    assert_eq!(
        (udf.tag, udf.last_tag, udf.mnemonic, udf.kind),
        (200, 900, "UDF", FieldKind::UserDefined)
    );
    let legacy = STD.field(9, 7).unwrap();
    assert_eq!(
        (legacy.tag, legacy.last_tag, legacy.kind),
        (5, 12, FieldKind::Deprecated)
    );
    assert_eq!(STD.field(2, 500).unwrap().kind, FieldKind::UserDefined);
    assert_eq!(STD.field(7, 3).unwrap().kind, FieldKind::UserDefined);
}
