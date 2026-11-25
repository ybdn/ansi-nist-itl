//! Validation against the standard (LIB-16).

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

/// The findings on field `t.tag` of a record of type `t` holding `value`.
fn on_field<S: AsRef<str>>(t: u32, tag: u32, value: Vec<Vec<S>>) -> Vec<String> {
    let mut msg = conformant();
    let mut r = AsciiRecord::new(t).unwrap();
    r.set("IDC", 9).unwrap();
    let value: Vec<Vec<&str>> = value
        .iter()
        .map(|sf| sf.iter().map(AsRef::as_ref).collect())
        .collect();
    r.add_field(Field::new(t, tag, value).unwrap()).unwrap();
    msg.push(r).unwrap();
    let prefix = format!("{t}.{tag:03}");
    read(msg)
        .validate()
        .findings
        .iter()
        .map(ToString::to_string)
        .filter(|f| f.starts_with(&prefix))
        .collect()
}

/// Section 7.7.12.2: after NOP, HPO and VPO repeat as pairs, NOP times
/// (14.025 ASEG, Table 104; 10.033 FEC; 20.016 SEG without points).
#[test]
fn points_repeat_as_pairs() {
    let three = vec!["2", "3", "10", "10", "20", "10", "20", "20"];
    assert_eq!(on_field(14, 25, vec![three]), Vec::<String>::new());
    let fec = vec!["LEFTEYEBROW", "3", "1", "2", "3", "4", "5", "6"];
    assert_eq!(on_field(10, 33, vec![fec]), Vec::<String>::new());
    assert_eq!(
        on_field(20, 16, vec![vec!["1", "0", ""]]),
        Vec::<String>::new()
    );
    // NOP and the pairs are absent together.
    assert_eq!(
        on_field(
            20,
            16,
            vec![vec!["1", "0", "", "1", "1", "2", "2", "3", "3"]]
        ),
        ["20.016 subfield 1: NOP is empty, 3 points follow"]
    );
    assert_eq!(
        on_field(
            14,
            25,
            vec![vec!["2", "4", "10", "10", "20", "10", "20", "20"]]
        ),
        ["14.025 subfield 1: NOP is 4, 3 points follow"]
    );
    assert_eq!(
        on_field(14, 25, vec![vec!["2", "3", "10", "10", "20", "10", "20"]]),
        [
            "14.025 subfield 1: 5 items after NOP, not whole HPO VPO pairs",
            "14.025 subfield 1: 2 points, at least 3 expected",
            "14.025 subfield 1: NOP is 3, 2 points follow",
        ]
    );
    // The format of each coordinate still applies, at its own position.
    assert_eq!(
        on_field(
            14,
            25,
            vec![vec!["2", "3", "10", "10", "20", "10", "20", "X"]]
        ),
        ["14.025 subfield 1 item 8: character 'X' is not of type N"]
    );
}

/// Section 7.7.12.1 and its note 71: an EFS path is one item
/// `x1,y1-x2,y2-…`, each vertex "an (X,Y) pair of non-negative integers"; a
/// closed path (9.300 ROI) has at least 3 vertices.
#[test]
fn efs_path_in_one_item() {
    let roi = |rop: &str| {
        vec![vec![
            "983".to_string(),
            "983".into(),
            "75".into(),
            "100".into(),
            rop.into(),
        ]]
    };
    assert_eq!(
        on_field(9, 300, roi("0,0-100,0-0,100")),
        Vec::<String>::new()
    );
    assert_eq!(
        on_field(9, 300, roi("0,0-100,0")),
        ["9.300 subfield 1 item 5: 2 vertices, at least 3 expected"]
    );
    for vertex in ["7", "1,2,3", "1.5,2", "+3,2", "1 ,2", "1,"] {
        let rop = format!("0,0-100,0-{vertex}");
        assert_eq!(
            on_field(9, 300, roi(&rop)),
            [format!(
                "9.300 subfield 1 item 5: vertex {vertex:?} is not x,y, two non-negative integers"
            )]
        );
    }
    assert_eq!(
        on_field(9, 300, roi("0,0-100,0-0,123456")),
        ["9.300 subfield 1 item 5: 6 characters, at most 5 expected"]
    );
}

/// Section 7.7.12.1 and Table 42, note 115: 9.373 RPS is the open path of the
/// EFS, at least 2 vertices, each coordinate up to 5 characters.
#[test]
fn efs_open_path() {
    assert_eq!(
        on_field(9, 373, vec![vec!["0,0-10,10"]]),
        Vec::<String>::new()
    );
    assert_eq!(
        on_field(9, 373, vec![vec!["0,0"]]),
        ["9.373 subfield 1: 1 vertices, at least 2 expected"]
    );
}

/// Section 7.7.13: a list is one item, its values separated by `|`
/// (11.035 PPY: SIL, up to 600000 values).
#[test]
fn lists_in_one_item() {
    assert_eq!(
        on_field(11, 35, vec![vec!["1|2|3", "1"]]),
        Vec::<String>::new()
    );
    assert_eq!(
        on_field(11, 35, vec![vec!["1|X", "1"]]),
        ["11.035 subfield 1 item 1: character 'X' is not of type NS"]
    );
}

/// 18.018: "user-defined information items (there may be multiple items)":
/// any number, any content. The table marks them M↑, but conformant BioCTS
/// samples of NIST leave them empty: an empty one is a warning only.
#[test]
fn user_defined_items_are_free() {
    let udp = vec![vec!["a", "b", "c"], vec!["d"]];
    assert_eq!(on_field(18, 18, udp), Vec::<String>::new());
    assert_eq!(
        on_field(18, 18, vec![vec!["", ""]]),
        [
            "18.018 subfield 1 item 1: user-defined item is empty",
            "18.018 subfield 1 item 2: user-defined item is empty",
        ]
    );
    let mut msg = conformant();
    let mut r18 = AsciiRecord::new(18).unwrap();
    r18.set("IDC", 9).unwrap();
    r18.add_field(Field::new(18, 18, vec![vec![""]]).unwrap())
        .unwrap();
    msg.push(r18).unwrap();
    let report = read(msg).validate();
    let udp: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.to_string().starts_with("18.018"))
        .collect();
    assert_eq!(udp.len(), 1);
    assert_eq!(
        (udp[0].rule, udp[0].level),
        (Rule::MissingItem, Level::Warning)
    );
}

/// 10.011: a value of Table 19 except WSQ20 (note 151), so `JP2` and `JP2L`.
#[test]
fn face_compression_jpeg_2000() {
    for cga in ["JP2", "JP2L", "JPEGB"] {
        assert_eq!(
            on_field(10, 11, vec![vec![cga]]),
            Vec::<String>::new(),
            "{cga}"
        );
    }
}
