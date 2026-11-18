//! Writing refuses what the reading would cut or misread: a separator (FS,
//! GS, RS, US) in a value, a binary field other than the data field holding
//! one, the data label `999:` outside the data field of a record type with
//! data, and a tag format whose label the reading does not read back. The
//! data field is written last. The property tests (`tests/properties.rs`)
//! define the domain where a message is refused or read back as written;
//! `docs/CONFORMANCE.md` lists what lies outside it.

use ansi_nist_itl::encoding::CharacterEncoding;
use ansi_nist_itl::{
    AsciiRecord, BinaryField, ErrorKind, Field, Kind, Message, ParseOptions, Record,
};

fn message_with(r: AsciiRecord) -> Message {
    let mut msg = Message::new();
    msg.set_tot("CAR").unwrap();
    msg.push(r).unwrap();
    msg
}

/// The error of `to_bytes`, of kind `kind`, as `index type.tag: message`.
fn refused(mut msg: Message, kind: ErrorKind) -> String {
    let err = msg.to_bytes().unwrap_err();
    assert_eq!(err.kind(), &kind, "{err}");
    let p = err.position();
    format!(
        "{} {}.{}: {err}",
        p.record_index.map_or(String::new(), |i| i.to_string()),
        p.record_type.map_or(String::new(), |t| t.to_string()),
        p.tag.as_ref().map_or(String::new(), ToString::to_string),
    )
}

#[test]
fn separators_in_text_values() {
    for byte in 0x1Cu8..=0x1F {
        let value = format!("a{}b", char::from(byte));
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", 0).unwrap();
        r.set("_18", value.as_str()).unwrap();
        assert_eq!(
            refused(message_with(r), ErrorKind::Encoding),
            format!("1 2.18: the separator byte 0x{byte:02X} cannot be written in a value")
        );
        // In an item.
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", 0).unwrap();
        r.set("_18", vec![vec!["x", value.as_str()]]).unwrap();
        refused(message_with(r), ErrorKind::Encoding);
    }
    // In the Type-1.
    let mut msg = Message::new();
    msg.set_tot("C\u{1d}AR").unwrap();
    assert!(refused(msg, ErrorKind::Encoding).starts_with("0 1.4: "));
}

/// The IDC goes into the CNT of the Type-1, written first: the error names
/// the IDC.
#[test]
fn separator_in_an_idc() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", "a\u{1d}b").unwrap();
    let msg = message_with(r);
    let copy = msg.clone();
    assert!(refused(msg, ErrorKind::Encoding).starts_with("1 2.2: "));
    let err = copy.to_bytes_exact().unwrap_err();
    assert_eq!(err.position().record_index, Some(1));
}

#[test]
fn set_encoded_refuses_separators_with_the_same_message() {
    let mut r = AsciiRecord::new(2).unwrap();
    for encoding in [
        CharacterEncoding::Ascii,
        CharacterEncoding::Latin1,
        CharacterEncoding::Utf8,
    ] {
        let err = r.set_encoded("_18", "a\u{1c}b", encoding).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::Encoding);
        assert_eq!(
            err.to_string(),
            "the separator byte 0x1C cannot be written in a value"
        );
    }
}

#[test]
fn data_label_outside_the_data_field() {
    // Type-14 has a data field: `999:` in another field would start it.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 20, "see 999:x").unwrap())
        .unwrap();
    assert_eq!(
        refused(message_with(r), ErrorKind::DataLabel),
        "1 14.20: 999: cannot be written outside the data field: it would be read as its label"
    );
    // So would the label of field 14.1999, or a binary field holding it.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 1999, "x").unwrap()).unwrap();
    refused(message_with(r), ErrorKind::DataLabel);
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(BinaryField::new(14, 998, b"a999:b".to_vec()).unwrap())
        .unwrap();
    r.set_bytes("DATA", b"img".to_vec()).unwrap();
    refused(message_with(r), ErrorKind::DataLabel);
    // Type-2 has none: `999:` is ordinary text there.
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", 0).unwrap();
    r.add_field(Field::new(2, 20, "see 999:x").unwrap())
        .unwrap();
    let mut msg = message_with(r);
    let bytes = msg.to_bytes().unwrap();
    assert_eq!(
        Message::parse(&bytes).unwrap().to_bytes_exact().unwrap(),
        bytes
    );
}

#[test]
fn binary_fields() {
    // The data field holds any byte.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.set_bytes("DATA", b"\x1c\x1d\x1e\x1f999:".to_vec())
        .unwrap();
    let mut msg = message_with(r);
    let bytes = msg.to_bytes().unwrap();
    let mut read = Message::parse(&bytes).unwrap();
    assert_eq!(read.to_bytes().unwrap(), bytes);
    // Another binary field is cut at a separator.
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", 0).unwrap();
    r.add_field(BinaryField::new(2, 998, b"a\x1db".to_vec()).unwrap())
        .unwrap();
    refused(message_with(r), ErrorKind::Encoding);
}

/// A field read, then changed to a binary value holding a separator: the
/// exact writing checks it too.
#[test]
fn exact_writing_checks_a_changed_field() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", 0).unwrap();
    r.set("_20", "ok").unwrap();
    let mut msg = message_with(r);
    let bytes = msg.to_bytes().unwrap();
    let mut read = Message::parse(&bytes).unwrap();
    let r2 = read.record_mut(1).unwrap().as_ascii_mut().unwrap();
    r2.remove_field(20).unwrap();
    r2.add_field(BinaryField::new(2, 20, b"a\x1db".to_vec()).unwrap())
        .unwrap();
    let err = read.to_bytes_exact().unwrap_err();
    assert_eq!(err.kind(), &ErrorKind::Encoding);
    assert_eq!(err.position().record_index, Some(1));
}

/// A field added after the reading is written before the data field, which
/// stays last; the record read keeps its bytes.
#[test]
fn data_field_stays_last() {
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.set_bytes("DATA", b"img".to_vec()).unwrap();
    let mut msg = message_with(r);
    let bytes = msg.to_bytes().unwrap();
    let mut read = Message::parse(&bytes).unwrap();
    assert_eq!(read.to_bytes_exact().unwrap(), bytes);
    let r14 = read.record_mut(1).unwrap().as_ascii_mut().unwrap();
    r14.set("_20", "late").unwrap();
    let exact = read.to_bytes_exact().unwrap();
    let back = Message::parse(&exact).unwrap();
    let r14 = back.record(1).unwrap().as_ascii().unwrap();
    assert_eq!(r14.get("_20").unwrap().to_string(), "late");
    assert_eq!(r14.get_bytes("DATA").unwrap(), b"img");

    // Built by the API, a binary field added after the data field.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.set_bytes("DATA", b"img".to_vec()).unwrap();
    r.add_field(BinaryField::new(14, 998, b"x".to_vec()).unwrap())
        .unwrap();
    let bytes = message_with(r).to_bytes().unwrap();
    let back = Message::parse(&bytes).unwrap();
    let Record::Ascii(r14) = back.record(1).unwrap() else {
        panic!("tagged")
    };
    assert_eq!(r14.get_bytes("DATA").unwrap(), b"img");
}

/// A tag format whose label the reading does not read back is refused when
/// the field is built, by the API and by deserialization.
#[test]
fn tag_formats() {
    for format in [
        "%d.%03d\u{1d}:",
        "%d.%03d:\u{1e}",
        "%d.%03d:\u{1f}",
        "%d.%03d:\u{1c}",
        "%d-%03d:",
        "%d.%d;",
        "%d.%5d:",
    ] {
        let err = Field::with(2, 20, "x", Kind::FSI, format, "").unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::BadFormat, "{format}");
        let err = BinaryField::with(14, 999, b"x".to_vec(), format, "").unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::BadFormat, "{format}");
    }
    let err = Field::new(2, -5, "x").unwrap_err();
    assert_eq!(err.kind(), &ErrorKind::BadFormat);
}

/// A file read, even damaged, is written back exactly: the bytes read are
/// kept, whatever they hold.
#[test]
fn exact_writing_keeps_what_was_read() {
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 20, "ok").unwrap()).unwrap();
    r.set_bytes("DATA", b"img".to_vec()).unwrap();
    let mut msg = message_with(r);
    let bytes = msg.to_bytes().unwrap();
    // `999:` inside 14.020 (quirk E3), and a truncated end.
    let text = String::from_utf8_lossy(&bytes).replace("14.020:ok", "14.020:999:");
    let damaged = text.into_bytes();
    let damaged = &damaged[..damaged.len() - 2];
    let parsed = Message::parse_with(damaged, ParseOptions::tolerant()).unwrap();
    assert_eq!(parsed.message.to_bytes_exact().unwrap(), damaged);
}

#[cfg(feature = "serde")]
#[test]
fn deserialization_is_checked() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", 0).unwrap();
    r.set("_18", "ab").unwrap();
    let msg = message_with(r);
    // A value: refused when written.
    let json = serde_json::to_string(&msg)
        .unwrap()
        .replace("\"ab\"", "\"a\\u001db\"");
    let back: Message = serde_json::from_str(&json).unwrap();
    refused(back, ErrorKind::Encoding);
    // A tag format: refused when read.
    let field =
        serde_json::json!({ "record": 2, "tag": 3, "format": "%d.%03d:\u{1c}", "value": "" });
    let err = serde_json::from_value::<Field>(field).unwrap_err();
    assert!(err.to_string().contains("format"), "{err}");
}

/// A binary record of a tagged type would be read as a tagged one: both
/// writings refuse it.
#[test]
fn binary_record_of_a_tagged_type() {
    for t in [2, 10] {
        let mut b = ansi_nist_itl::BinaryRecord::new(t);
        b.set_value(b"zz".to_vec());
        let mut m = Message::new();
        m.push(b).unwrap();
        assert_eq!(
            m.to_bytes_exact().unwrap_err().kind(),
            &ErrorKind::TypeMismatch
        );
        assert_eq!(m.to_bytes().unwrap_err().kind(), &ErrorKind::TypeMismatch);
    }
}

/// A record built, never read, written exactly: its fields are checked.
#[test]
fn exact_writing_checks_a_new_record() {
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 20, "x999:").unwrap()).unwrap();
    let m = message_with(r);
    let err = m.to_bytes_exact().unwrap_err();
    assert_eq!(err.kind(), &ErrorKind::DataLabel);
    assert_eq!(err.position().record_index, Some(1));
}

/// The first field of a record takes its length: its label is checked too.
#[test]
fn first_field_label() {
    let mut r = AsciiRecord::with(14, false, true).unwrap();
    r.add_field(Field::new(14, 1999, "").unwrap()).unwrap();
    r.add_field(Field::new(14, 2, 1).unwrap()).unwrap();
    assert_eq!(
        refused(message_with(r), ErrorKind::DataLabel),
        "1 14.1999: 999: cannot be written outside the data field: it would be read as its label"
    );
}

/// A text field at the tag of the data field would be read as the data, and
/// take in the fields after it.
#[test]
fn text_field_at_the_data_tag() {
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 999, "t").unwrap()).unwrap();
    r.add_field(Field::new(14, 1000, "z").unwrap()).unwrap();
    let m = message_with(r);
    assert_eq!(
        m.to_bytes_exact().unwrap_err().kind(),
        &ErrorKind::DataLabel
    );
    assert_eq!(
        refused(m, ErrorKind::DataLabel),
        "1 14.999: field 999 holds the data of its record: it must be binary"
    );
}
