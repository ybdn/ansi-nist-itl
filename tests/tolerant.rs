//! Tolerant mode (SPEC §13.2, LIB-13): every error of the strict mode
//! becomes a diagnostic with its recovery, and the parsing goes on.

use DiagnosticCode::*;
use ansi_nist_itl::{
    AnyField, Diagnostic, DiagnosticCode, Message, ParseOptions, Parsed, Severity, Value,
};

/// `rec(t, body, first)` of SPEC appendix C.
fn rec_with(t: u32, body: &[u8], first: u32) -> Vec<u8> {
    let head = format!("{t}.{first:03}:");
    (1..100_000)
        .map(|len| [format!("{head}{len}\x1d").as_bytes(), body].concat())
        .find(|r| r.starts_with(format!("{head}{}\x1d", r.len()).as_bytes()))
        .unwrap()
}

fn rec(t: u32, body: &[u8]) -> Vec<u8> {
    rec_with(t, body, 1)
}

fn t1_cnt(cnt: &[u8]) -> Vec<u8> {
    rec(
        1,
        &[
            &b"1.002:0400\x1d1.003:"[..],
            cnt,
            b"\x1d1.004:T\x1d1.005:20200101\x1d1.007:000\x1d1.008:000\x1d1.009:X\x1d1.011:00.00\x1d1.012:00.00\x1c",
        ]
        .concat(),
    )
}

fn t1() -> Vec<u8> {
    t1_cnt(b"1\x1f1\x1e2\x1f1")
}

fn r(t: u32, fields: &[u8]) -> Vec<u8> {
    rec(t, &[fields, b"\x1c"].concat())
}

fn tolerant(buffer: &[u8]) -> Parsed {
    Message::parse_with(buffer, ParseOptions::tolerant())
        .expect("the tolerant mode always succeeds")
}

fn codes(parsed: &Parsed) -> Vec<(DiagnosticCode, Severity)> {
    parsed
        .diagnostics
        .iter()
        .map(|d| (d.code, d.severity))
        .collect()
}

fn field(parsed: &Parsed, record: usize, tag: i32) -> Value {
    match parsed
        .message
        .record(record)
        .unwrap()
        .as_ascii()
        .unwrap()
        .field(tag)
    {
        Some(AnyField::Text(f)) => f.value(),
        other => panic!("not a text field: {other:?}"),
    }
}

/// The message of each diagnostic is the error of the strict mode.
fn same_message_as_strict(buffer: &[u8], parsed: &Parsed) {
    let strict = Message::parse(buffer).unwrap_err().to_string();
    assert!(
        parsed.diagnostics.iter().any(|d| d.message == strict),
        "{strict:?} not in {:?}",
        parsed.diagnostics
    );
}

#[test]
fn truncated_record_is_kept() {
    // C38: LEN too large at the end.
    let full = [t1(), r(2, b"2.002:1\x1d2.003:abc")].concat();
    let b = String::from_utf8(full)
        .unwrap()
        .replace("2.001:27", "2.001:28");
    let parsed = tolerant(b.as_bytes());
    assert_eq!(codes(&parsed), [(Truncated, Severity::Error)]);
    same_message_as_strict(b.as_bytes(), &parsed);
    assert_eq!(field(&parsed, 1, 3), Value::from("abc"));
    assert_eq!(parsed.message.len(), 2);
}

#[test]
fn len_too_small_extends_the_record() {
    // C36.
    let full = [t1(), r(2, b"2.002:1\x1d2.003:abc")].concat();
    let b = String::from_utf8(full)
        .unwrap()
        .replace("2.001:27", "2.001:15");
    let parsed = tolerant(b.as_bytes());
    assert_eq!(codes(&parsed), [(LenMismatch, Severity::Warning)]);
    assert_eq!(field(&parsed, 1, 3), Value::from("abc"));
}

#[test]
fn bad_field_is_skipped() {
    // C21 and C22.
    for fields in [
        &b"2.002:1\x1d\x1d2.004:a"[..],
        b"2.002:1\x1d2.:x\x1d2.004:a",
    ] {
        let b = [t1(), r(2, fields)].concat();
        let parsed = tolerant(&b);
        assert_eq!(codes(&parsed), [(BadField, Severity::Error)]);
        same_message_as_strict(&b, &parsed);
        assert_eq!(field(&parsed, 1, 4), Value::from("a"));
        assert!(parsed.diagnostics[0].position.offset.is_some());
    }
}

#[test]
fn field_of_another_type_is_kept() {
    // C20.
    let b = [t1(), r(2, b"2.002:1\x1d3.004:a")].concat();
    let parsed = tolerant(&b);
    assert_eq!(codes(&parsed), [(TagRecordMismatch, Severity::Warning)]);
    same_message_as_strict(&b, &parsed);
    assert_eq!(field(&parsed, 1, 4), Value::from("a"));
}

#[test]
fn duplicate_replaces_the_first() {
    // C18.
    let b = [t1(), r(2, b"2.002:1\x1d2.003:a\x1d2.003:b")].concat();
    let parsed = tolerant(&b);
    assert_eq!(codes(&parsed), [(DuplicateTag, Severity::Warning)]);
    same_message_as_strict(&b, &parsed);
    assert_eq!(field(&parsed, 1, 3), Value::from("b"));
    // The second Type-1 of C30 is merged into the first.
    let b = [t1_cnt(b"1\x1f0"), t1_cnt(b"1\x1f0")].concat();
    let parsed = tolerant(&b);
    assert!(codes(&parsed).iter().all(|&(c, _)| c == DuplicateTag));
    assert_eq!(parsed.message.len(), 1);
}

#[test]
fn bad_data_tag_reads_the_record_as_text() {
    // C58.
    let b = [
        t1_cnt(b"1\x1f1\x1e14\x1f1"),
        rec(14, b"14.002:1\x1dX999:abc\x1c"),
    ]
    .concat();
    let parsed = tolerant(&b);
    assert_eq!(
        codes(&parsed),
        [(BadDataTag, Severity::Error), (BadField, Severity::Error)]
    );
    same_message_as_strict(&b, &parsed);
    assert_eq!(
        parsed.message.record(1).unwrap().idc().unwrap(),
        Value::from(1)
    );
}

#[test]
fn invalid_numbers_stay_text() {
    // C08, C115–C117.
    for idc in [
        b"A".to_vec(),
        vec![b'A'; 300],
        b"1\x00".to_vec(),
        [&[b'1'; 4301][..], b"\x00A"].concat(),
    ] {
        let b = [t1(), r(2, &[&b"2.002:"[..], &idc].concat())].concat();
        let parsed = tolerant(&b);
        let found = codes(&parsed);
        assert!(
            found.contains(&(InvalidNumber, Severity::Warning)),
            "{found:?}"
        );
        same_message_as_strict(&b, &parsed);
        let read = parsed.message.record(1).unwrap().idc().unwrap();
        assert_eq!(read.to_string().len(), idc.len());
    }
}

#[test]
fn unknown_type_is_read_without_aliases() {
    // C28 and C29.
    for (cnt, record) in [
        (&b"1\x1f1\x1e50\x1f1"[..], r(50, b"50.002:1\x1d50.999:x")),
        (b"1\x1f1\x1e4\x1f1", r(4, b"4.002:1")),
    ] {
        let b = [t1_cnt(cnt), record].concat();
        let parsed = tolerant(&b);
        assert_eq!(codes(&parsed), [(UnknownRecordType, Severity::Warning)]);
        same_message_as_strict(&b, &parsed);
        let r = parsed.message.record(1).unwrap().as_ascii().unwrap();
        assert!(r.fields().iter().all(|f| f.alias().is_empty()));
        assert!(matches!(r.field(2), Some(AnyField::Text(_))));
    }
}

#[test]
fn unknown_binary_record_stops() {
    // C35, C62, C63, C64.
    let cases: [Vec<u8>; 4] = [
        [t1_cnt(b"1\x1f0"), vec![0; 5]].concat(),
        [
            t1_cnt(b"1\x1f0"),
            9u32.to_be_bytes().to_vec(),
            b"\x01data".to_vec(),
        ]
        .concat(),
        [
            t1_cnt(b"1\x1f1\x1e4\x1f3"),
            0u32.to_be_bytes().to_vec(),
            b"\x03xxxx".to_vec(),
        ]
        .concat(),
        [
            t1_cnt(b"1\x1f1\x1e4\x1f1"),
            3u32.to_be_bytes().to_vec(),
            b"\x01data".to_vec(),
        ]
        .concat(),
    ];
    for b in cases {
        let parsed = tolerant(&b);
        assert_eq!(codes(&parsed)[0], (UnknownBinaryRecord, Severity::Fatal));
        assert!(b.ends_with(parsed.message.trailing()));
        assert!(!parsed.message.trailing().is_empty());
        // The bytes read come back.
        assert_eq!(parsed.message.to_bytes_exact().unwrap(), b);
    }
}

#[test]
fn binary_record_of_a_tagged_type_is_kept() {
    // C65.
    let b = [t1(), 9u32.to_be_bytes().to_vec(), b"\x01data".to_vec()].concat();
    let parsed = tolerant(&b);
    assert_eq!(codes(&parsed), [(UnexpectedBinaryRecord, Severity::Error)]);
    same_message_as_strict(&b, &parsed);
    let r = parsed.message.record(1).unwrap().as_binary().unwrap();
    assert_eq!((r.record_type(), r.value()), (2, &b"data"[..]));
}

#[test]
fn truncated_binary_record_is_kept() {
    // C61.
    let full = [
        t1_cnt(b"1\x1f1\x1e4\x1f3"),
        23u32.to_be_bytes().to_vec(),
        b"\x03".to_vec(),
        vec![1; 15],
        b"IMG".to_vec(),
    ]
    .concat();
    let b = &full[..full.len() - 2];
    let parsed = tolerant(b);
    assert_eq!(codes(&parsed), [(Truncated, Severity::Error)]);
    same_message_as_strict(b, &parsed);
    let r = parsed.message.record(1).unwrap().as_binary().unwrap();
    assert_eq!(r.value().len(), 16);
}

#[test]
fn unterminated_record_extends_to_the_end() {
    // C41.
    let b = [t1(), b"2.001:15\x1d2.002:1abcde".to_vec()].concat();
    let parsed = tolerant(&b);
    let found = codes(&parsed);
    assert_eq!(found[0], (Unterminated, Severity::Error));
    same_message_as_strict(&b, &parsed);
    assert_eq!(parsed.message.len(), 2);
}

#[test]
fn trailing_bytes_are_kept() {
    // C33, C34.
    let parsed = tolerant(b"1.00");
    assert!(codes(&parsed).contains(&(TrailingBytes, Severity::Warning)));
    assert_eq!(parsed.message.trailing(), b"1.00");
    let b = [t1_cnt(b"1\x1f0"), b"\x00".to_vec()].concat();
    let parsed = tolerant(&b);
    assert_eq!(codes(&parsed), [(TrailingBytes, Severity::Warning)]);
    assert_eq!(parsed.message.trailing(), b"\x00");
    assert_eq!(parsed.message.to_bytes_exact().unwrap(), b);
}

#[test]
fn cnt_mismatch_is_a_warning() {
    // C45, C47, C49, C50, C51, C66.
    for b in [
        [t1_cnt(b"1\x1f1\x1e9\x1f7"), r(2, b"2.002:1")].concat(),
        t1_cnt(b"1\x1f1\x1e2\x1f1"),
        rec(1, b"1.002:0400\x1d1.003:1\x1c"),
        [t1_cnt(b"1\x1f1\x1eX\x1f1"), r(2, b"2.002:1")].concat(),
        t1_cnt(b"2\x1f0"),
    ] {
        let parsed = tolerant(&b);
        assert_eq!(codes(&parsed), [(CntMismatch, Severity::Warning)], "{b:?}");
        same_message_as_strict(&b, &parsed);
    }
}

#[test]
fn missing_type1() {
    // C31, C32, C48.
    for b in [
        r(2, b"2.002:1"),
        Vec::new(),
        [rec(1, b"1.002:0400\x1c"), r(2, b"2.002:1")].concat(),
    ] {
        let parsed = tolerant(&b);
        assert_eq!(codes(&parsed), [(MissingType1, Severity::Error)]);
        same_message_as_strict(&b, &parsed);
        assert_eq!(parsed.message.record(0).unwrap().record_type(), 1);
    }
    let parsed = tolerant(&r(2, b"2.002:1"));
    assert_eq!(parsed.message.len(), 2);
    assert_eq!(
        parsed.message.record(1).unwrap().idc().unwrap(),
        Value::from(1)
    );
}

#[test]
fn quirks_are_reported() {
    let t1 = t1_cnt(b"1\x1f1\x1e14\x1f1");
    for (fields, code) in [
        (
            &b"14.002:1\x1d14.020:see 999:x\x1d14.999:DATA\x1c"[..],
            DataTagInText,
        ),
        (b"14.002:1\x1d14.020:ok\x1d14.999:DATAZ", MissingFinalFs),
        (
            b"14.002:1\x1d14.999:x\x1d14.020:ok\x1d14.999:DATA\x1c",
            DataNotLast,
        ),
    ] {
        let b = [t1.clone(), rec(14, fields)].concat();
        let strict = Message::parse(&b).unwrap();
        let parsed = tolerant(&b);
        // E3 also makes the fields after the label part of the data (E5).
        assert_eq!(codes(&parsed)[0], (code, Severity::Warning));
        assert!(codes(&parsed).iter().all(|&(_, s)| s == Severity::Warning));
        assert_eq!(parsed.message, strict);
        let strict = Message::parse_with(&b, ParseOptions::strict()).unwrap();
        assert!(strict.diagnostics.is_empty());
    }
}

#[test]
fn strict_accepts_the_same_message() {
    let mut msg = Message::new();
    msg.set_tot("TEST").unwrap();
    let b = msg.to_bytes().unwrap();
    let parsed = tolerant(&b);
    assert!(parsed.diagnostics.is_empty());
    assert_eq!(parsed.message, Message::parse(&b).unwrap());
}

#[test]
fn diagnostics_display() {
    let parsed = tolerant(b"");
    let d: &Diagnostic = &parsed.diagnostics[0];
    assert_eq!(
        d.to_string(),
        "Error (MissingType1): object of type 'NoneType' has no len()"
    );
}
