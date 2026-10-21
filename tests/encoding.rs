//! Character encodings of field 1.015 (LIB-18, SPEC §13.8).

use ansi_nist_itl::encoding::{CharacterEncoding, decode, encode};
use ansi_nist_itl::{AsciiRecord, Defaults, ErrorKind, Message, Value};
use proptest::prelude::*;

const NAMES: [&str; 5] = ["Hélène Müller", "Øyvind Ærø", "李小龍", "محمد", "🙂 emoji"];

fn utf8_message() -> Message {
    let mut msg = Message::with_defaults(&Defaults {
        date: Some("20200101".into()),
        tcn: Some("T".into()),
    });
    msg.type1_mut()
        .unwrap()
        .set("DCS", vec![vec!["3", "UTF-8", "1.0"]])
        .unwrap();
    let mut r2 = AsciiRecord::new(2).unwrap();
    r2.set("IDC", 0).unwrap();
    r2.set_encoded("_18", NAMES.to_vec(), CharacterEncoding::Utf8)
        .unwrap();
    msg.push(r2).unwrap();
    msg
}

#[test]
fn utf8_through_writing_and_reading() {
    let bytes = utf8_message().to_bytes().unwrap();
    // The bytes on the wire are the UTF-8 of the names.
    let utf8 = NAMES.join("\u{1e}");
    assert!(bytes.windows(utf8.len()).any(|w| w == utf8.as_bytes()));

    let read = Message::parse(&bytes).unwrap();
    let encoding = read.character_encoding().unwrap();
    assert_eq!(encoding, CharacterEncoding::Utf8);
    let r2 = read.records()[1].as_ascii().unwrap();
    assert_eq!(
        r2.get_decoded("_18", encoding).unwrap(),
        Value::from(NAMES.to_vec())
    );
    // Without decoding, the text stays the Latin-1 view, as in nistitl.
    assert_eq!(
        r2.get("_18").unwrap().to_string().chars().count() > utf8.chars().count(),
        true
    );
}

#[test]
fn exact_writing_keeps_the_bytes() {
    let bytes = utf8_message().to_bytes().unwrap();
    assert_eq!(
        Message::parse(&bytes).unwrap().to_bytes_exact().unwrap(),
        bytes
    );
}

#[test]
fn declared_encodings() {
    let mut msg = Message::new();
    let cases = [
        ("0", Ok(CharacterEncoding::Ascii)),
        ("1", Ok(CharacterEncoding::Latin1)),
        ("2", Ok(CharacterEncoding::Utf16)),
        ("003", Ok(CharacterEncoding::Utf8)),
        ("4", Ok(CharacterEncoding::Utf32)),
        ("128", Ok(CharacterEncoding::UserDefined(128))),
        ("5", Err(ErrorKind::UnsupportedEncoding(Some(5)))),
        ("1000", Err(ErrorKind::UnsupportedEncoding(Some(1000)))),
        ("x", Err(ErrorKind::InvalidLiteral)),
    ];
    for (csi, expected) in cases {
        msg.type1_mut()
            .unwrap()
            .set("DCS", vec![vec![csi, "NAME"]])
            .unwrap();
        let got = msg.character_encoding().map_err(|e| e.kind().clone());
        assert_eq!(got, expected, "{csi}");
    }
}

#[test]
fn errors() {
    let unsupported = |r: ansi_nist_itl::Result<String>| r.unwrap_err().kind().clone();
    assert_eq!(
        unsupported(decode("x", CharacterEncoding::Utf16)),
        ErrorKind::UnsupportedEncoding(Some(2))
    );
    assert_eq!(
        unsupported(encode("x", CharacterEncoding::UserDefined(200))),
        ErrorKind::UnsupportedEncoding(Some(200))
    );
    assert_eq!(
        unsupported(decode("a\u{2}=3\u{3}", CharacterEncoding::Utf8)),
        ErrorKind::UnsupportedEncoding(None)
    );
    // Malformed UTF-8: a lone continuation byte.
    let err = decode("a\u{80}", CharacterEncoding::Utf8).unwrap_err();
    assert_eq!(
        (err.kind(), err.message()),
        (&ErrorKind::Decoding, "invalid UTF-8 at byte 1")
    );
    // Text set by the API, not a view of bytes.
    assert_eq!(
        decode("€", CharacterEncoding::Utf8).unwrap_err().kind(),
        &ErrorKind::Decoding
    );
    assert_eq!(
        encode("é", CharacterEncoding::Ascii).unwrap_err().kind(),
        &ErrorKind::Encoding
    );
    let mut r = AsciiRecord::new(2).unwrap();
    assert!(r.set_encoded("_3", "€", CharacterEncoding::Latin1).is_err());
    assert!(r.field(3).is_none());
}

proptest! {
    #[test]
    fn utf8_round_trip(text in "\\PC*") {
        let stored = encode(&text, CharacterEncoding::Utf8).unwrap();
        prop_assert!(stored.chars().all(|c| u32::from(c) <= 0xFF));
        prop_assert_eq!(decode(&stored, CharacterEncoding::Utf8).unwrap(), text);
    }

    #[test]
    fn latin1_is_the_identity(text in "[\\x00-\\x01\\x03-\\u{ff}]*") {
        prop_assert_eq!(encode(&text, CharacterEncoding::Latin1).unwrap(), text.clone());
        prop_assert_eq!(decode(&text, CharacterEncoding::Latin1).unwrap(), text);
    }

    /// Any Latin-1 view decodes or fails, without panic.
    #[test]
    fn any_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let view: String = bytes.iter().map(|&b| char::from(b)).collect();
        let utf8 = decode(&view, CharacterEncoding::Utf8);
        prop_assert_eq!(utf8.is_ok(), std::str::from_utf8(&bytes).is_ok() && !bytes.contains(&2));
    }
}
