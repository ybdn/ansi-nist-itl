//! A value that the reading would cut is refused when written: a separator
//! (FS, GS, RS, US) in a value, or, in a record type with data, the data
//! label `999:` outside the data field. The crate never writes, without an
//! error, a file it cannot read back as written.

use ansi_nist_itl::encoding::CharacterEncoding;
use ansi_nist_itl::{AsciiRecord, BinaryField, ErrorKind, Field, Message, ParseOptions};

fn message_with(r: AsciiRecord) -> Message {
    let mut msg = Message::new();
    msg.set_tot("CAR").unwrap();
    msg.push(r).unwrap();
    msg
}

fn refused(mut msg: Message) -> String {
    let err = msg.to_bytes().unwrap_err();
    assert_eq!(err.kind(), &ErrorKind::Encoding, "{err}");
    let tag = err.position().tag.as_ref().map(ToString::to_string);
    format!("{}: {err}", tag.unwrap_or_default())
}

#[test]
fn separators_in_text_values() {
    for (value, byte) in [
        ("a\u{1c}b", 0x1C),
        ("a\u{1d}b", 0x1D),
        ("a\u{1e}b", 0x1E),
        ("a\u{1f}b", 0x1F),
    ] {
        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", 0).unwrap();
        // A scalar, an item, and a subfield of items.
        r.set("_18", value).unwrap();
        let message = refused(message_with(r));
        assert!(message.contains(&format!("0x{byte:02X}")), "{message}");
        assert!(message.starts_with("18: "), "{message}");

        let mut r = AsciiRecord::new(2).unwrap();
        r.set("IDC", 0).unwrap();
        r.set("_18", vec![vec!["x", value]]).unwrap();
        refused(message_with(r));
    }
    // A Type-1 value too.
    let mut msg = Message::new();
    msg.set_tot("C\u{1d}AR").unwrap();
    refused(msg);
}

#[test]
fn set_encoded_refuses_separators() {
    let mut r = AsciiRecord::new(2).unwrap();
    for encoding in [
        CharacterEncoding::Ascii,
        CharacterEncoding::Latin1,
        CharacterEncoding::Utf8,
    ] {
        let err = r.set_encoded("_18", "a\u{1c}b", encoding).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::Encoding);
    }
}

#[test]
fn data_label_outside_the_data_field() {
    // Type-14 has a data field: `999:` in another field would start it.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 20, "see 999:x").unwrap())
        .unwrap();
    assert!(refused(message_with(r)).contains("999:"));
    // So would the label of field 14.1999.
    let mut r = AsciiRecord::new(14).unwrap();
    r.set("IDC", 1).unwrap();
    r.add_field(Field::new(14, 1999, "x").unwrap()).unwrap();
    refused(message_with(r));
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
    refused(message_with(r));
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
    let text = String::from_utf8_lossy(&bytes).replace("14.020:ok", "14.020:o9");
    let damaged = text.replace("o9", "999:").into_bytes();
    let damaged = &damaged[..damaged.len() - 2];
    let parsed = Message::parse_with(damaged, ParseOptions::tolerant()).unwrap();
    assert_eq!(parsed.message.to_bytes_exact().unwrap(), damaged);
}

#[cfg(feature = "serde")]
#[test]
fn deserialized_values_are_checked_when_written() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", 0).unwrap();
    r.set("_18", "ab").unwrap();
    let msg = message_with(r);
    let json = serde_json::to_string(&msg)
        .unwrap()
        .replace("\"ab\"", "\"a\\u001db\"");
    let back: Message = serde_json::from_str(&json).unwrap();
    refused(back);
}
