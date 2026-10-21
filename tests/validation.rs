//! Validation against the standard (LIB-16, SPEC §13.9).

#![cfg(feature = "validation")]

use ansi_nist_itl::dictionary::Standard;
use ansi_nist_itl::validation::{Level, Rule, Validation};
use ansi_nist_itl::{AsciiRecord, BinaryRecord, Defaults, Field, ImageHeader, Message, Value};

/// A message that follows Update 2015.
fn conformant() -> Message {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("TCN2020".into()),
    });
    let t1 = msg.type1_mut().unwrap();
    t1.set("VER", "0502").unwrap();
    t1.set("DAI", "DAI000000").unwrap();
    t1.set("ORI", "ORI000000").unwrap();
    t1.set("NSR", "19.69").unwrap();
    t1.set("NTR", "19.69").unwrap();
    t1.set("GMT", "20200101120000Z").unwrap();
    t1.set("DCS", vec![vec!["3", "UTF-8", "1.0"]]).unwrap();
    msg.set_tot("CAR").unwrap();

    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    r2.add_field(Field::new(2, 18, "DOE").unwrap()).unwrap();
    msg.push(r2).unwrap();

    let mut r4 = BinaryRecord::new(4);
    r4.set_idc(1);
    r4.set_image_header(&ImageHeader {
        imp: 1,
        fgp: [1, 255, 255, 255, 255, 255],
        isr: 1,
        hll: 800,
        vll: 750,
        compression: 1,
    })
    .unwrap();
    r4.set_image_data(b"wsq").unwrap();
    msg.push(r4).unwrap();

    let mut r14 = AsciiRecord::new(14).unwrap();
    for (name, value) in [
        ("IDC", "2"),
        ("IMP", "1"),
        ("SRC", "AGENCY"),
        ("FCD", "20200101"),
        ("HLL", "800"),
        ("VLL", "750"),
        ("SLC", "1"),
        ("THPS", "500"),
        ("TVPS", "500"),
        ("CGA", "WSQ20"),
        ("BPX", "8"),
        ("FGP", "2"),
    ] {
        r14.set(name, value).unwrap();
    }
    r14.set("_15", vec![vec!["FV1", "NA", "0", "800", "0", "750"]])
        .unwrap();
    r14.set_bytes("DATA", b"wsq".to_vec()).unwrap();
    msg.push(r14).unwrap();
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
fn conformant_message() {
    let report = read(conformant()).validate();
    assert_eq!(report.standard, Some(Standard::Update2015));
    assert_eq!(rules(&report), []);
    assert!(report.is_valid());
}

#[test]
fn versions() {
    let mut msg = conformant();
    msg.type1_mut().unwrap().set("VER", "0400").unwrap();
    let report = read(msg.clone()).validate();
    assert_eq!(report.standard, None);
    assert_eq!(
        rules(&report),
        [(
            Rule::Version,
            Level::Warning,
            "1.002: no tables for version 0400".into()
        )]
    );
    // Forced: the tables apply, and VER is still a valid N 4..4.
    assert!(read(msg).validate_against(Standard::Update2015).is_valid());
}

/// The first finding of a message where `f` changed one thing.
fn finding(f: impl FnOnce(&mut Message)) -> (Rule, Level, String) {
    let mut msg = conformant();
    f(&mut msg);
    let report = read(msg).validate();
    assert_eq!(report.findings.len(), 1, "{:?}", rules(&report));
    rules(&report).remove(0)
}

fn r14(msg: &mut Message) -> &mut AsciiRecord {
    msg.record_mut(3).unwrap().as_ascii_mut().unwrap()
}

#[test]
fn fields() {
    assert_eq!(
        finding(|m| {
            r14(m).remove_field(4);
        }),
        (
            Rule::MissingField,
            Level::Error,
            "14.004: mandatory field SRC is absent".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m).set("_19", "x").unwrap()),
        (
            Rule::ReservedField,
            Level::Error,
            "14.019: field reserved for future use".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m).set("_1000", "x").unwrap()),
        (
            Rule::UndefinedField,
            Level::Error,
            "14.1000: Update 2015 has no field 14.1000"
                .replace("Update 2015", &Standard::Update2015.to_string())
        ),
    );
    let mut msg = conformant();
    let mut r10 = AsciiRecord::new(10).unwrap();
    r10.set("IDC", 3).unwrap();
    r10.set("_22", "legacy").unwrap();
    msg.push(r10).unwrap();
    let report = read(msg).validate();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == Rule::DeprecatedField && f.level == Level::Warning)
    );
}

#[test]
fn values() {
    assert_eq!(
        finding(|m| r14(m).set("IMP", "X").unwrap()),
        (
            Rule::CharacterType,
            Level::Error,
            "14.003: character 'X' is not of type N".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m).set("CGA", "WS").unwrap()),
        (
            Rule::Size,
            Level::Error,
            "14.011: 2 characters, at least 3 expected".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m).set("FCD", "20200230").unwrap()),
        (
            Rule::Date,
            Level::Error,
            "14.005: \"20200230\" is not a date YYYYMMDD".into()
        )
    );
    assert_eq!(
        finding(|m| m.type1_mut().unwrap().set("GMT", "20200101120000").unwrap()),
        (
            Rule::Date,
            Level::Error,
            "1.014: \"20200101120000\" is not a GMT YYYYMMDDhhmmssZ".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m).set("HLL", vec!["800", "1"]).unwrap()),
        (
            Rule::Structure,
            Level::Error,
            "14.006: expected a single value".into()
        )
    );
}

#[test]
fn subfields_and_items() {
    // 1 to 12 sets of 6 items for PPC.
    let set = vec!["FV1", "NA", "0", "800", "0", "750"];
    assert_eq!(
        finding(|m| r14(m).set("_15", vec![set.clone(); 13]).unwrap()),
        (
            Rule::SubfieldCount,
            Level::Error,
            "14.015: 13 subfields, at most 12 expected".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m)
            .set("_15", vec![vec!["FV1", "NA", "0", "800", "0", "750", "x"]])
            .unwrap()),
        (
            Rule::ItemCount,
            Level::Error,
            "14.015 subfield 1: 7 items, at most 6 expected".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m)
            .set("_15", vec![vec!["FV1", "", "0", "800", "0", "750"]])
            .unwrap()),
        (
            Rule::MissingItem,
            Level::Error,
            "14.015 subfield 1 item 2: mandatory item LOS is empty".into()
        )
    );
    assert_eq!(
        finding(|m| r14(m)
            .set("_15", vec![vec!["FV1", "N4", "0", "800", "0", "750"]])
            .unwrap()),
        (
            Rule::CharacterType,
            Level::Error,
            "14.015 subfield 1 item 2: character '4' is not of type A".into()
        )
    );
}

#[test]
fn type1_is_ascii() {
    let mut msg = conformant();
    msg.type1_mut().unwrap().set("ORI", "ORI\u{e9}").unwrap();
    // Both the rule of section 5.6 and the type ANS of 1.008.
    assert_eq!(
        rules(&read(msg).validate()),
        [
            (
                Rule::Type1Ascii,
                Level::Error,
                "1.008: character U+00E9 is not 7-bit ASCII".into()
            ),
            (
                Rule::CharacterType,
                Level::Error,
                "1.008: character '\u{e9}' is not of type ANS".into()
            ),
        ]
    );
}

#[test]
fn record_types() {
    let mut msg = conformant();
    let mut r6 = BinaryRecord::new(6);
    r6.set_idc(4);
    r6.set_image_header(&ImageHeader::default()).unwrap();
    msg.push(r6).unwrap();
    let report = read(msg).validate();
    assert_eq!(
        rules(&report),
        [(
            Rule::RecordType,
            Level::Warning,
            "6: record type 6 is deprecated".into()
        )]
    );
    assert!(report.is_valid());
}

#[test]
fn content() {
    // CNT is checked as it is; writing would compute it again.
    let mut msg = read(conformant());
    msg.type1_mut()
        .unwrap()
        .set(
            "CNT",
            vec![
                vec!["1", "3"],
                vec!["2", "0"],
                vec!["4", "1"],
                vec!["14", "7"],
            ],
        )
        .unwrap();
    let report = msg.validate();
    assert_eq!(
        rules(&report),
        [(
            Rule::Content,
            Level::Error,
            "1.003 subfield 4: 14 7 where the message has 14 2".into()
        )]
    );
    let _ = Value::from(0);
}
