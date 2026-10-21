//! Serialization with `serde` (SPEC §13.4, LIB-23): the JSON form, the round
//! trip, and the checks of the deserialization.

#![cfg(feature = "serde")]

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, BinaryRecord, Defaults, Field, ImageHeader, Item, Kind,
    Message, PyInt, Scalar, SubField, Value,
};
use serde_json::json;

fn defaults() -> Defaults {
    Defaults {
        date: Some("20200101".into()),
        tcn: Some("TCN".into()),
    }
}

/// Types 1, 2, 14 and 4, with every optional key of the form.
fn sample() -> Message {
    let mut msg = Message::with_defaults(&defaults());
    msg.set_tot("CAR").unwrap();

    let mut r2 = AsciiRecord::with(2, true, false).unwrap();
    r2.set("IDC", 0).unwrap();
    r2.add_field(Field::new(2, 18, vec![vec!["DOE", "JOHN"]]).unwrap())
        .unwrap();
    r2.add_field(Field::with(2, 20, vec!["A", "B"], "S", "%d.%d:", "NAMES").unwrap())
        .unwrap();
    r2.add_field(Field::new(2, "123456789012345678901234567890", Value::from(())).unwrap())
        .unwrap();
    msg.push(r2).unwrap();

    let mut r14 = AsciiRecord::new(14).unwrap();
    r14.set("IDC", 1).unwrap();
    r14.set_bytes("DATA", b"\xff\xa0\x1c".to_vec()).unwrap();
    msg.push(r14).unwrap();

    let mut r4 = BinaryRecord::new(4);
    r4.set_idc(2);
    r4.set_image_header(&ImageHeader {
        fgp: [1, 255, 255, 255, 255, 255],
        ..Default::default()
    })
    .unwrap();
    r4.set_image_data(b"wsq").unwrap();
    r4.unpack("!B6BBHHB").unwrap();
    msg.push(r4).unwrap();
    msg
}

#[test]
fn json_form() {
    let mut msg = sample();
    msg.to_bytes().unwrap(); // LEN and CNT computed
    let expected = json!({ "records": [
        { "type": 1, "fields": [
            { "tag": 1, "alias": "LEN", "value": 123 },
            { "tag": 2, "alias": "VER", "value": "0400" },
            { "tag": 3, "alias": "CNT", "value": [[1, 3], [2, 0], [14, 1], [4, 2]] },
            { "tag": 4, "alias": "TOT", "value": "CAR" },
            { "tag": 5, "alias": "DAT", "value": "20200101" },
            { "tag": 7, "alias": "DAI", "value": "000" },
            { "tag": 8, "alias": "ORI", "value": "000" },
            { "tag": 9, "alias": "TCN", "value": "TCN" },
            { "tag": 11, "alias": "NSR", "value": "00.00" },
            { "tag": 12, "alias": "NTR", "value": "00.00" },
        ] },
        { "type": 2, "autosort": false, "fields": [
            { "tag": 1, "alias": "LEN", "value": 75 },
            { "tag": 2, "alias": "IDC", "value": 0 },
            { "tag": 18, "value": [["DOE", "JOHN"]] },
            { "tag": 20, "alias": "NAMES", "kind": "S", "format": "%d.%d:", "value": ["A", "B"] },
            { "tag": { "int": "123456789012345678901234567890" }, "value": null },
        ] },
        { "type": 14, "fields": [
            { "tag": 1, "alias": "LEN", "value": 30 },
            { "tag": 2, "alias": "IDC", "value": 1 },
            { "tag": 999, "alias": "DATA", "bytes": "/6Ac" },
        ] },
        { "type": 4, "idc": 2, "format": "!B6BBHHB", "bytes": "AAH//////wAAAAAAAHdzcQ==" },
    ] });
    let got = serde_json::to_value(&msg).unwrap();
    assert_eq!(
        got,
        expected,
        "{}",
        serde_json::to_string_pretty(&got).unwrap()
    );
}

#[test]
fn round_trip() {
    let mut msg = sample();
    let json = serde_json::to_string(&msg).unwrap();
    let mut back: Message = serde_json::from_str(&json).unwrap();
    assert!(back == msg);
    assert_eq!(back.to_bytes().unwrap(), msg.to_bytes().unwrap());
    assert_eq!(back.summary().unwrap(), msg.summary().unwrap());
    assert_eq!(
        serde_json::to_string(&back).unwrap(),
        serde_json::to_string(&msg).unwrap()
    );
}

#[test]
fn round_trip_of_a_parsed_message() {
    let mut msg = Message::parse(&sample().to_bytes().unwrap()).unwrap();
    let mut back: Message = serde_json::from_value(serde_json::to_value(&msg).unwrap()).unwrap();
    let bytes = msg.to_bytes().unwrap();
    assert_eq!(back.to_bytes().unwrap(), bytes);
    // No origin: the exact writing is the writing.
    assert_eq!(back.to_bytes_exact().unwrap(), bytes);
}

#[test]
fn values() {
    let cases = [
        (Value::from(()), json!(null)),
        (Value::from(true), json!(true)),
        (Value::from(-7), json!(-7)),
        (Value::from("07"), json!("07")),
        (Value::from(vec!["a", "b"]), json!(["a", "b"])),
        (
            Value::from(vec![vec!["a", "b"], vec!["c"]]),
            json!([["a", "b"], ["c"]]),
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(serde_json::to_value(&value).unwrap(), expected);
        assert_eq!(serde_json::from_value::<Value>(expected).unwrap(), value);
    }
    let nested = Item::List(vec![Item::from("a"), Item::List(vec![Item::from(1)])]);
    assert_eq!(serde_json::to_value(&nested).unwrap(), json!(["a", [1]]));
    assert_eq!(
        serde_json::from_value::<Item>(json!(["a", [1]])).unwrap(),
        nested
    );
}

#[test]
fn integers() {
    let big: PyInt = serde_json::from_value(json!({ "int": "-0099999999999999999999" })).unwrap();
    assert_eq!(big.to_string(), "-99999999999999999999");
    assert_eq!(
        serde_json::to_value(&big).unwrap(),
        json!({ "int": "-99999999999999999999" })
    );
    let max: PyInt = serde_json::from_value(json!(u64::MAX)).unwrap();
    assert_eq!(
        serde_json::to_value(&max).unwrap(),
        json!({ "int": "18446744073709551615" })
    );
    assert_eq!(
        serde_json::from_value::<Scalar>(json!({ "int": "5" })).unwrap(),
        Scalar::from(5)
    );
    for bad in [
        json!(1.5),
        json!({ "int": "1e3" }),
        json!({ "int": 3 }),
        json!({ "num": "3" }),
    ] {
        assert!(
            serde_json::from_value::<PyInt>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(serde_json::from_value::<Scalar>(json!(1.5)).is_err());
    assert!(serde_json::from_value::<Scalar>(json!(["a"])).is_err());
}

#[test]
fn fields_and_subfields() {
    let f = Field::with(2, 3, vec!["a"], "SI", "%d.%03d:", "").unwrap();
    let json = json!({ "record": 2, "tag": 3, "kind": "SI", "value": ["a"] });
    assert_eq!(serde_json::to_value(&f).unwrap(), json);
    assert_eq!(serde_json::from_value::<Field>(json).unwrap(), f);

    let b = BinaryField::new(10, 999, b"ab".to_vec()).unwrap();
    let json = json!({ "record": 10, "tag": 999, "bytes": "YWI=" });
    assert_eq!(serde_json::to_value(&b).unwrap(), json);
    assert_eq!(
        serde_json::from_value::<BinaryField>(json.clone()).unwrap(),
        b
    );
    assert!(matches!(
        serde_json::from_value::<AnyField>(json).unwrap(),
        AnyField::Binary(_)
    ));

    let sf = SubField::new(vec!["x", "y"], "I").unwrap();
    let json = json!({ "kind": "I", "value": ["x", "y"] });
    assert_eq!(serde_json::to_value(&sf).unwrap(), json);
    assert_eq!(serde_json::from_value::<SubField>(json).unwrap(), sf);
    assert_eq!(
        serde_json::to_value(SubField::new("x", Kind::SI).unwrap()).unwrap(),
        json!({ "value": "x" })
    );
}

/// The message of a deserialization error.
fn error<T: serde::de::DeserializeOwned + std::fmt::Debug>(json: serde_json::Value) -> String {
    serde_json::from_value::<T>(json).unwrap_err().to_string()
}

#[test]
fn checks_of_the_api() {
    let type1 = json!({ "type": 1, "fields": [] });
    assert_eq!(
        error::<Message>(json!({ "records": [type1, type1] })),
        "CANNOT_ADD_TYPE1: Cannot add a type 1 record: it must be the first record"
    );
    assert_eq!(
        error::<Message>(json!({ "records": [{ "type": 2, "fields": [] }] })),
        "the first record must be the Type-1"
    );
    assert_eq!(
        error::<Message>(json!({ "records": [] })),
        "the first record must be the Type-1"
    );
    assert_eq!(
        error::<AsciiRecord>(json!({ "type": 2, "fields": [
            { "tag": 3, "value": "a" }, { "tag": 3, "value": "b" }
        ] })),
        "BAD_TAG_DUPLICATE: Tag 3 already defined in record 2"
    );
    assert_eq!(
        error::<AsciiRecord>(json!({ "type": 2, "fields": [
            { "tag": 3, "alias": "X", "value": "a" }, { "tag": 4, "alias": "X", "value": "b" }
        ] })),
        "BAD_ALIAS_DUPLICATE: Alias 'X' already defined in record 2"
    );
    assert_eq!(
        error::<AsciiRecord>(
            json!({ "type": 2, "fields": [{ "record": 9, "tag": 4, "value": "" }] })
        ),
        "BAD_RECORD_NUMBER: Bad record number 9 for tag 4 in record 2"
    );
    assert_eq!(
        error::<Field>(json!({ "record": 2, "tag": 3, "kind": "S", "value": "x" })),
        "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)"
    );
    assert_eq!(
        error::<SubField>(json!({ "kind": "I", "value": "x" })),
        "BAD_SUBFIELD_VALUE: Subfield cannot have a value, only items"
    );
    assert!(
        error::<Field>(json!({ "record": 2, "tag": 3, "format": "%d:", "value": "" }))
            .contains("format")
    );
    // As with the API, an unknown type fails at its first field.
    assert!(serde_json::from_value::<AsciiRecord>(json!({ "type": 23, "fields": [] })).is_ok());
    assert!(
        error::<AsciiRecord>(json!({ "type": 23, "fields": [{ "tag": 3, "value": "a" }] }))
            .contains("23")
    );
}

#[test]
fn checks_of_the_form() {
    for (json, needle) in [
        (json!({ "tag": 3, "value": "a" }), "missing field `record`"),
        (
            json!({ "record": 2, "tag": 3 }),
            "missing field `value` or `bytes`",
        ),
        (
            json!({ "record": 2, "tag": 3, "value": "a", "bytes": "" }),
            "not both",
        ),
        (
            json!({ "record": 2, "tag": 3, "value": "a", "extra": 1 }),
            "unknown field `extra`",
        ),
        (
            json!({ "record": 10, "tag": 999, "kind": "F", "bytes": "" }),
            "no `kind`",
        ),
        (
            json!({ "record": 10, "tag": 999, "bytes": "YWI" }),
            "canonical base64",
        ),
        (json!({ "record": 2, "tag": 3.5, "value": "a" }), "integer"),
    ] {
        let err = error::<AnyField>(json.clone());
        assert!(err.contains(needle), "{json}: {err}");
    }
    for (json, needle) in [
        (
            json!({ "type": 4, "idc": 1, "bytes": "", "fields": [] }),
            "not both",
        ),
        (json!({ "type": 4, "bytes": "" }), "missing field `idc`"),
        (json!({ "type": 4, "idc": 1 }), "missing field `bytes`"),
        (json!({ "type": 4, "idc": 256, "bytes": "" }), "u8"),
        (
            json!({ "type": 4, "idc": 1, "format": "!Z", "bytes": "" }),
            "format",
        ),
        (
            json!({ "type": 4, "idc": 1, "autosort": true, "bytes": "" }),
            "no `autosort`",
        ),
        (json!({ "type": 2 }), "missing field `fields`"),
    ] {
        let err = error::<ansi_nist_itl::Record>(json.clone());
        assert!(err.contains(needle), "{json}: {err}");
    }
    // The tolerant mode reads records whose encoding does not match the
    // type: the API builds them, and so does the deserialization.
    let r: ansi_nist_itl::Record =
        serde_json::from_value(json!({ "type": 9, "idc": 1, "bytes": "" })).unwrap();
    assert!(r.as_binary().is_some());
    assert!(error::<BinaryRecord>(json!({ "type": 2, "fields": [] })).contains("binary"));
    assert!(error::<AsciiRecord>(json!({ "type": 4, "idc": 1, "bytes": "" })).contains("tagged"));
}
