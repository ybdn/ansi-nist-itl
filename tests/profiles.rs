//! Application profiles EBTS 11.2 and INT-I 5.03 (LIB-17, SPEC §13.10).

#![cfg(feature = "validation")]

use ansi_nist_itl::validation::{Level, Profile, Rule, Validation};
use ansi_nist_itl::{AsciiRecord, BinaryRecord, Defaults, Field, ImageHeader, Message};

fn base(version: &str, tot: &str, tcn: &str) -> Message {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20230920".into()),
        tcn: Some(tcn.into()),
    });
    msg.type1_mut().unwrap().set("VER", version).unwrap();
    msg.set_tot(tot).unwrap();
    msg
}

fn fingerprint(idc: u8, fgp: u8) -> BinaryRecord {
    let mut r4 = BinaryRecord::new(4);
    r4.set_idc(idc);
    r4.set_image_header(&ImageHeader {
        imp: 1,
        fgp: [fgp, 255, 255, 255, 255, 255],
        isr: 0,
        hll: 800,
        vll: 750,
        compression: 1,
    })
    .unwrap();
    r4.set_image_data(b"wsq").unwrap();
    r4
}

/// A criminal tenprint submission (CAR) as EBTS 11.2 describes it.
fn ebts_car() -> Message {
    let mut msg = base("0502", "CAR", "TCN0000000001");
    let t1 = msg.type1_mut().unwrap();
    for (name, value) in [
        ("DAI", "WVIAFIS0Z"),
        ("ORI", "NY0303000"),
        ("NSR", "19.69"),
        ("NTR", "19.69"),
    ] {
        t1.set(name, value).unwrap();
    }
    t1.set("DOM", vec![vec!["NORAM", "EBTS 11.2"]]).unwrap();
    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    for (tag, value) in [
        (5, "N"),
        (18, "DOE,JOHN"),
        (20, "NY"),
        (22, "19790815"),
        (24, "M"),
        (25, "W"),
        (27, "510"),
        (29, "180"),
        (31, "BRO"),
        (32, "BLK"),
        (45, "20230919"),
        (73, "NY0303000"),
    ] {
        r2.add_field(Field::new(2, tag, value).unwrap()).unwrap();
    }
    r2.add_field(Field::new(2, 47, vec![vec!["20230919", "BURGLARY"]]).unwrap())
        .unwrap();
    msg.push(r2).unwrap();
    for i in 1..=2 {
        msg.push(fingerprint(i, i)).unwrap();
    }
    msg
}

/// A criminal print-to-print search (CPS) as INT-I 5.03 describes it.
fn inti_cps() -> Message {
    // 2300000042 % 23 = 19 (V).
    let mut msg = base("0400", "CPS", "2300000042V");
    let t1 = msg.type1_mut().unwrap();
    for (name, value) in [
        ("DAI", "ZZ/ALL"),
        ("ORI", "FR/SDPTS"),
        ("NSR", "19.69"),
        ("NTR", "19.69"),
    ] {
        t1.set(name, value).unwrap();
    }
    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    r2.add_field(Field::new(2, 3, "0400").unwrap()).unwrap();
    r2.add_field(Field::new(2, 30, "BELL/CHARLES/PETER").unwrap())
        .unwrap();
    msg.push(r2).unwrap();
    msg.push(fingerprint(1, 1)).unwrap();
    msg
}

fn read(mut msg: Message) -> Message {
    Message::parse(&msg.to_bytes().unwrap()).unwrap()
}

fn rules(v: &Validation) -> Vec<(Rule, Level, String)> {
    v.findings
        .iter()
        .map(|f| (f.rule, f.level, f.to_string()))
        .collect()
}

#[test]
fn conformant_transactions() {
    let ebts = read(ebts_car()).validate_profile(Profile::Ebts112);
    assert_eq!(rules(&ebts), []);
    let inti = read(inti_cps()).validate_profile(Profile::IntI503);
    assert_eq!(rules(&inti), []);
}

fn ebts(f: impl FnOnce(&mut Message)) -> Vec<(Rule, Level, String)> {
    let mut msg = ebts_car();
    f(&mut msg);
    rules(&read(msg).validate_profile(Profile::Ebts112))
}

fn r2(msg: &mut Message) -> &mut AsciiRecord {
    msg.record_mut(1).unwrap().as_ascii_mut().unwrap()
}

#[test]
fn ebts_type1() {
    assert_eq!(
        ebts(|m| m.type1_mut().unwrap().set("ORI", "NY030300").unwrap()),
        [(
            Rule::Size,
            Level::Error,
            "EBTS 11.2: 1.008: 8 characters, at least 9 expected".into()
        )]
    );
    assert_eq!(
        ebts(|m| m.type1_mut().unwrap().set("NTR", "500").unwrap()),
        [
            // The standard (NS 5..5) and the profile (NN.NN).
            (
                Rule::Size,
                Level::Error,
                "1.012: 3 characters, at least 5 expected".into()
            ),
            (
                Rule::Format,
                Level::Error,
                "EBTS 11.2: 1.012: \"500\" is not a resolution NN.NN".into()
            ),
        ]
    );
    assert_eq!(
        ebts(|m| m.type1_mut().unwrap().set("DOM", vec![vec!["NORAM", "11.2"]]).unwrap()),
        [(
            Rule::Format,
            Level::Warning,
            "EBTS 11.2: 1.013: the second item should be the version of EBTS (EBTS #.#), not \"11.2\"".into()
        )]
    );
    let found = ebts(|m| m.set_tot("XYZ").unwrap());
    assert_eq!(
        found,
        [(
            Rule::TransactionType,
            Level::Error,
            "EBTS 11.2: 1.004: XYZ is not a type of transaction of EBTS 11.2".into()
        )]
    );
}

#[test]
fn ebts_records() {
    assert_eq!(
        ebts(|m| {
            for i in 3..=15 {
                m.push(fingerprint(i, i)).unwrap();
            }
        }),
        [(
            Rule::RecordCount,
            Level::Error,
            "EBTS 11.2: 1.004: CAR (CRIMINAL TENPRINT SUBMISSION (ANSWER REQUIRED)) holds 0 to 14 records of type 4, found 15".into()
        )]
    );
    let mut r99 = AsciiRecord::new(99).unwrap();
    r99.set("IDC", 9).unwrap();
    let found = ebts(|m| m.push(r99).unwrap());
    assert!(
        found.contains(&(
            Rule::RecordCount,
            Level::Error,
            "EBTS 11.2: 1.004: CAR holds no record of type 99".into()
        )),
        "{found:?}"
    );
}

#[test]
fn ebts_type2() {
    assert_eq!(
        ebts(|m| {
            r2(m).remove_field(18);
        }),
        [(
            Rule::MissingField,
            Level::Error,
            "EBTS 11.2: 2.018: CAR requires NAM".into()
        )]
    );
    // FBI (2.014): 0 to 5, with reference note 5.
    assert_eq!(
        ebts(|m| r2(m).set("_14", vec!["1"; 6]).unwrap()),
        [(
            Rule::SubfieldCount,
            Level::Warning,
            "EBTS 11.2: 2.014: CAR allows 0 to 5 occurrences, found 6 (reference note 5)".into()
        )]
    );
    // LCN (2.012) is not used by CAR.
    assert_eq!(
        ebts(|m| r2(m).set("_12", "MX-12345678").unwrap()),
        [(
            Rule::FieldNotInTransaction,
            Level::Error,
            "EBTS 11.2: 2.012: CAR does not use this field".into()
        )]
    );
    // 2.150 is allocated to California.
    assert_eq!(
        ebts(|m| r2(m).set("_150", "x").unwrap()),
        [(
            Rule::FieldNotInTransaction,
            Level::Warning,
            "EBTS 11.2: 2.150: field allocated to CA USE, not checked".into()
        )]
    );
}

#[test]
fn ebts_type2_formats() {
    assert_eq!(
        ebts(|m| r2(m).set("_29", "18O").unwrap()),
        [(
            Rule::CharacterType,
            Level::Error,
            "EBTS 11.2: 2.029 subfield 1: character 'O' is not of type N".into()
        )]
    );
    assert_eq!(
        ebts(|m| r2(m).set("_47", vec![vec!["20230919", ""]]).unwrap()),
        [(
            Rule::MissingItem,
            Level::Error,
            "EBTS 11.2: 2.047 subfield 1 item 2: mandatory item AOL is empty".into()
        )]
    );
}

fn inti(f: impl FnOnce(&mut Message)) -> Vec<(Rule, Level, String)> {
    let mut msg = inti_cps();
    f(&mut msg);
    rules(&read(msg).validate_profile(Profile::IntI503))
}

#[test]
fn inti_type1() {
    assert_eq!(
        inti(|m| m.type1_mut().unwrap().set("TCN", "2300000042T").unwrap()),
        [(
            Rule::Format,
            Level::Error,
            "INT-I 5.03: 1.009: \"2300000042T\" is not YYSSSSSSSSA with its check character (6.1.9, Appendix 2)".into()
        )]
    );
    assert_eq!(
        inti(|m| m.type1_mut().unwrap().set("DAI", "FRA/X").unwrap()),
        [(
            Rule::Format,
            Level::Error,
            "INT-I 5.03: 1.007: \"FRA/X\" is not CC/agency (6.1.7)".into()
        )]
    );
    assert_eq!(
        inti(|m| m.type1_mut().unwrap().set("VER", "0500").unwrap()),
        [(
            Rule::Format,
            Level::Error,
            "INT-I 5.03: 1.002: \"0500\" is not version 0400 (ANSI/NIST-ITL 1-2007, 6.1.2)".into()
        )]
    );
}

#[test]
fn inti_records() {
    let mut r14 = AsciiRecord::new(14).unwrap();
    r14.set("IDC", 2).unwrap();
    assert_eq!(
        inti(|m| {
            m.remove(2).unwrap();
            m.push(r14).unwrap();
        }),
        [(
            Rule::RecordCount,
            Level::Error,
            "INT-I 5.03: 1.004: CPS: Type-14 images must also be sent as Type-4 records (Table 9, note 3)".into()
        )]
    );
    let mut r10 = AsciiRecord::new(10).unwrap();
    r10.set("IDC", 3).unwrap();
    assert_eq!(
        inti(|m| m.push(r10).unwrap()),
        [(
            Rule::RecordCount,
            Level::Error,
            "INT-I 5.03: 1.004: CPS (Criminal Print-to-Print Search) allows no record of type 10"
                .into()
        )]
    );
    // IMR: at least one of the types 4, 7, 13 and 15.
    assert_eq!(
        inti(|m| {
            m.set_tot("IMR").unwrap();
            m.remove(2).unwrap();
        }),
        [(
            Rule::RecordCount,
            Level::Error,
            "INT-I 5.03: 1.004: IMR requires a record of one of the types 4, 7, 13, 15".into()
        )]
    );
    // MPS: Type-4 latents are obsolete.
    assert_eq!(
        inti(|m| m.set_tot("MPS").unwrap()),
        [(
            Rule::RecordCount,
            Level::Warning,
            "INT-I 5.03: 1.004: MPS: Type-4 and Type-7 latents are obsolete, use Type-13 (Table 9, note 1)".into()
        )]
    );
}

#[test]
fn inti_type2() {
    assert_eq!(
        inti(|m| {
            r2(m).remove_field(3);
        }),
        [(
            Rule::MissingField,
            Level::Error,
            "INT-I 5.03: 2.003: fields 2.001 to 2.003 are mandatory (7.1)".into()
        )]
    );
    assert_eq!(
        inti(|m| r2(m).set("_100", "x").unwrap()),
        [(
            Rule::ReservedField,
            Level::Error,
            "INT-I 5.03: 2.100: fields 2.086 to 2.199 are reserved for INT-I (7)".into()
        )]
    );
    assert_eq!(
        inti(|m| r2(m).set("_30", "B\u{c9}LL/CHARLES").unwrap()),
        [(
            Rule::CharacterType,
            Level::Error,
            "INT-I 5.03: 2.030: character U+00C9: the first Type-2 record holds 7-bit ASCII only (7)".into()
        )]
    );
}
