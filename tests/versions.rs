//! Traditional versions 0200 to 0500 (LIB-19, PRD §9.1): read as any other
//! version, since `nistitl` never looks at 1.002.
//!
//! Messages are built by the crate (DATA-01), each with the record types of
//! its version, then read, rewritten and written back exactly.

use ansi_nist_itl::{
    AsciiRecord, BinaryRecord, Defaults, Field, ImageHeader, Message, Record, SignatureHeader,
    Value, Warning,
};

/// Each version with the record types it introduced or kept: 0200
/// (ANSI/NIST-CSL 1-1993), 0300 (ANSI/NIST-ITL 1-2000), 0400 (1-2007),
/// 0500 (1-2011).
const VERSIONS: [(&str, &[u32]); 4] = [
    ("0200", &[2, 3, 4, 5, 6, 7, 8, 9]),
    ("0300", &[2, 4, 7, 8, 9, 10, 13, 14, 15, 16]),
    ("0400", &[2, 4, 9, 10, 13, 14, 15, 16, 17, 99]),
    (
        "0500",
        &[2, 4, 9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21, 98, 99],
    ),
];

/// Types with a DATA field at 999.
const DATA_TYPES: [u32; 10] = [10, 13, 14, 15, 16, 17, 19, 20, 21, 99];

fn binary(record_type: u32, idc: u8) -> Record {
    let mut r = BinaryRecord::new(record_type);
    r.set_idc(idc);
    match record_type {
        3..=6 => {
            r.set_image_header(&ImageHeader {
                imp: 0,
                fgp: [idc, 255, 255, 255, 255, 255],
                isr: 1,
                hll: 2,
                vll: 2,
                compression: 1,
            })
            .unwrap();
            r.set_image_data(b"\xff\xa0\xff\xa8wsq").unwrap();
        }
        8 => {
            r.set_signature_header(&SignatureHeader {
                sig: 0,
                srt: 0,
                isr: 0,
                hll: 2,
                vll: 1,
            })
            .unwrap();
            r.set_image_data(b"\x00\xff").unwrap();
        }
        _ => r.set_value(b"user-defined".to_vec()),
    }
    r.into()
}

fn tagged(record_type: u32, idc: u32) -> Record {
    let mut r = AsciiRecord::new(record_type).unwrap();
    r.set("IDC", idc).unwrap();
    r.add_field(Field::new(record_type, 20, vec![vec!["DOE", "JOHN"], vec!["X"]]).unwrap())
        .unwrap();
    if DATA_TYPES.contains(&record_type) {
        r.set_bytes("DATA", b"\x00\x1c\x1d\xff".to_vec()).unwrap();
    }
    r.into()
}

fn build(version: &str, types: &[u32]) -> Vec<u8> {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("19950101".into()),
        tcn: Some(format!("TCN{version}")),
    });
    msg.set_tot("CAR").unwrap();
    let type1 = msg.type1_mut().unwrap();
    type1.set("VER", version).unwrap();
    type1.set("NSR", "19.69").unwrap();
    type1.set("NTR", "19.69").unwrap();
    for (i, &t) in types.iter().enumerate() {
        let idc = u8::try_from(i + 1).unwrap();
        msg.push(if (3..=8).contains(&t) {
            binary(t, idc)
        } else {
            tagged(t, u32::from(idc))
        })
        .unwrap();
    }
    msg.to_bytes().unwrap()
}

#[test]
fn read_every_version() {
    for (version, types) in VERSIONS {
        let bytes = build(version, types);
        let msg = Message::parse(&bytes).unwrap();
        let type1 = msg.type1().unwrap();
        // 1.002 stays text: the leading zero is kept.
        assert_eq!(type1.get("VER").unwrap(), Value::from(version), "{version}");
        let read: Vec<u32> = msg.records().iter().map(Record::record_type).collect();
        assert_eq!(read[0], 1, "{version}");
        assert_eq!(&read[1..], types, "{version}");
        for record in msg.records_of(4) {
            let header = record.as_binary().unwrap().image_header().unwrap();
            assert_eq!((header.hll, header.vll), (2, 2), "{version}");
        }
    }
}

#[test]
fn rewrite_every_version() {
    for (version, types) in VERSIONS {
        let bytes = build(version, types);
        let mut msg = Message::parse(&bytes).unwrap();
        assert_eq!(msg.to_bytes().unwrap(), bytes, "{version}");
        assert!(
            msg.summary().unwrap().contains(&format!(
                " 1.002: VER                           : {version}"
            )),
            "{version}"
        );
    }
}

#[test]
fn write_every_version_exactly() {
    for (version, types) in VERSIONS {
        let bytes = build(version, types);
        let msg = Message::parse(&bytes).unwrap();
        assert_eq!(msg.to_bytes_exact().unwrap(), bytes, "{version}");
    }
}

/// The deprecation of Types 3, 5 and 6 does not depend on 1.002: `nistitl`
/// warns for them in every version.
#[test]
fn deprecation_ignores_the_version() {
    let (version, types) = VERSIONS[0];
    let bytes = build(version, types);
    let (_, warnings) = Message::parse_with_warnings(&bytes).unwrap();
    assert_eq!(
        warnings,
        [3, 5, 6].map(|record_type| Warning::Deprecated { record_type })
    );
}

/// No version-specific rule: a record type newer than the declared version
/// is read all the same.
#[test]
fn no_rule_per_version() {
    let bytes = build("0200", &[2, 17, 18, 98]);
    let msg = Message::parse(&bytes).unwrap();
    assert_eq!(msg.len(), 5);
}
