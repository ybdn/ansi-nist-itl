//! Edge cases of SPEC appendix C, checked against `nistitl` 0.6 (§15.2).
//!
//! One test per case: same values read, same bytes written, or same text
//! for the error.

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, BinaryRecord, Error, ErrorKind, Field, Item, ListElem,
    Message, NistError, PyInt, Scalar, SubField, SubValue, Value,
};

/// `str()` of the expected exception.
fn raises<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
    r.unwrap_err().to_string()
}

/// `Field(2, 3, value).NIST`.
fn nist(value: impl Into<Value>) -> Vec<u8> {
    Field::new(2, 3, value).unwrap().to_bytes().unwrap()
}

// Helpers of appendix C.

/// `rec(t, body, first)`: tagged record `t` whose LEN is computed; `first`
/// is the tag of its first field.
fn rec_with(t: u32, body: &[u8], first: u32) -> Vec<u8> {
    for l in 1..100_000usize {
        let mut s = format!("{t}.{first:03}:{l}").into_bytes();
        s.push(0x1D);
        s.extend_from_slice(body);
        if s.len() == l {
            return s;
        }
    }
    unreachable!("no length fits")
}

fn rec(t: u32, body: &[u8]) -> Vec<u8> {
    rec_with(t, body, 1)
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// `T1(cnt)`.
fn t1_cnt(cnt: &[u8]) -> Vec<u8> {
    rec(
        1,
        &cat(&[
            b"1.002:0400\x1d1.003:",
            cnt,
            b"\x1d1.004:T\x1d1.005:20200101\x1d1.007:000\x1d1.008:000\x1d1.009:X\x1d1.011:00.00\x1d1.012:00.00\x1c",
        ]),
    )
}

/// `T1()`.
fn t1() -> Vec<u8> {
    t1_cnt(b"1\x1f1\x1e2\x1f1")
}

/// `R(t, fields)`.
fn r(t: u32, fields: &[u8]) -> Vec<u8> {
    rec(t, &cat(&[fields, b"\x1c"]))
}

/// `T14(fields, data)`.
fn t14(fields: &[u8], data: &[u8]) -> Vec<u8> {
    rec(14, &cat(&[fields, b"\x1d14.999:", data, b"\x1c"]))
}

fn parse(buffer: &[u8]) -> Result<Message, Error> {
    Message::parse(buffer)
}

/// `parse(buffer)[i]`, a tagged record.
fn ascii(msg: &Message, i: usize) -> &AsciiRecord {
    msg.record(i).unwrap().as_ascii().unwrap()
}

/// `parse(buffer)[1].IDC`.
fn idc(buffer: &[u8]) -> Value {
    parse(buffer).unwrap().record(1).unwrap().idc().unwrap()
}

/// `parse(buffer)[1][tag].value` of a text field.
fn text(buffer: &[u8], tag: i32) -> Value {
    match ascii(&parse(buffer).unwrap(), 1).field(tag) {
        Some(AnyField::Text(f)) => f.value(),
        other => panic!("not a text field: {other:?}"),
    }
}

/// `parse(buffer)[1][tag].value` of a binary field.
fn data(buffer: &[u8], tag: i32) -> Vec<u8> {
    match ascii(&parse(buffer).unwrap(), 1).field(tag) {
        Some(AnyField::Binary(f)) => f.value().to_vec(),
        other => panic!("not a binary field: {other:?}"),
    }
}

fn rewrite(buffer: &[u8]) -> Vec<u8> {
    parse(buffer).unwrap().to_bytes().unwrap()
}

fn replace(buffer: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let i = buffer
        .windows(from.len())
        .position(|w| w == from)
        .expect("pattern");
    cat(&[&buffer[..i], to, &buffer[i + from.len()..]])
}

/// `T1(b'1\x1f1\x1e14\x1f1')`.
fn t1_14() -> Vec<u8> {
    t1_cnt(b"1\x1f1\x1e14\x1f1")
}

/// `T1(b'1\x1f1\x1e4\x1f3') + Type-4 of LEN 23, IDC 3`.
fn type4() -> Vec<u8> {
    cat(&[
        &t1_cnt(b"1\x1f1\x1e4\x1f3"),
        &23u32.to_be_bytes(),
        b"\x03",
        &[1; 15],
        b"IMG",
    ])
}

// Reading numbers (§3.1, §10.8).

#[test]
fn c01_negative_idc() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e2\x1f-1"), &r(2, b"2.002:-1")]);
    assert_eq!(idc(&b), Value::from(-1));
}

#[test]
fn c02_idc_with_spaces() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"2.002: 1 ")])), Value::from(1));
}

#[test]
fn c03_idc_with_underscore() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"2.002:0_1")])), Value::from(1));
}

#[test]
fn c04_idc_with_nbsp() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"2.002:1\xa0")])), Value::from(1));
}

#[test]
fn c05_idc_with_plus() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"2.002:+1")])), Value::from(1));
}

#[test]
fn c06_empty_idc() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"2.002:")])), Value::from(""));
}

#[test]
fn c07_idc_with_subfields() {
    let v = idc(&cat(&[&t1(), &r(2, b"2.002:1\x1e2")]));
    assert_eq!(v, Value::from(vec!["1", "2"]));
    assert_eq!(v.repr(), "['1', '2']");
}

#[test]
fn c08_alphabetic_idc() {
    let e = parse(&cat(&[&t1(), &r(2, b"2.002:A")])).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::InvalidLiteral);
    assert_eq!(e.to_string(), "invalid literal for int() with base 10: 'A'");
}

#[test]
fn c09_missing_idc_accepted() {
    assert_eq!(parse(&cat(&[&t1(), &r(2, b"2.003:a")])).unwrap().len(), 2);
}

#[test]
fn c10_missing_idc_by_key() {
    let msg = parse(&cat(&[&t1(), &r(2, b"2.003:a")])).unwrap();
    assert_eq!(
        raises(msg.by_key(2, 1)),
        "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <IDC> while trying to retrieve a field in record of type 2"
    );
}

#[test]
fn c11_idc_of_30_digits() {
    let v = idc(&cat(&[
        &t1(),
        &r(2, b"2.002:123456789012345678901234567890"),
    ]));
    assert_eq!(v.repr(), "123456789012345678901234567890");
    assert!(matches!(v, Value::Scalar(Scalar::Int(_))));
}

#[test]
fn c12_ver_stays_text() {
    let msg = parse(&t1_cnt(b"1\x1f0")).unwrap();
    assert_eq!(ascii(&msg, 0).get("VER").unwrap(), Value::from("0400"));
}

#[test]
fn c13_ver_with_subfields() {
    let msg = parse(&rec(1, b"1.002:04\x1e00\x1d1.003:1\x1f0\x1c")).unwrap();
    assert_eq!(
        ascii(&msg, 0).get("VER").unwrap(),
        Value::from(vec!["04", "00"])
    );
}

#[test]
fn c14_utf8_read_as_latin1() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.005:\xc3\xa9")]);
    assert_eq!(text(&b, 5), Value::from("\u{c3}\u{a9}"));
}

// Reading the structure (§10.7–10.9).

#[test]
fn c15_unsorted_fields_rewritten() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.005:b\x1d2.003:a")]);
    assert_ne!(rewrite(&b), b);
}

#[test]
fn c16_tag_2_03_rewritten() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.03:a")]);
    let out = rewrite(&b);
    assert_eq!(&out[out.len() - 14..], b"002:1\x1d2.003:a\x1c");
}

#[test]
fn c17_tag_with_leading_zeros() {
    assert_eq!(idc(&cat(&[&t1(), &r(2, b"002.0002:1")])), Value::from(1));
}

#[test]
fn c18_duplicate_simple_value() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.003:a\x1d2.003:b")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "BAD_TAG_DUPLICATE: Tag 3 already defined in record 2"
    );
    let p = e.position();
    assert_eq!(p.record_index, Some(1));
    assert_eq!(p.record_type, Some(2));
    assert_eq!(p.tag, Some(PyInt::from(3)));
    let field = t1().len() + b"2.001:27\x1d2.002:1\x1d2.003:a\x1d".len();
    assert_eq!(&b[field..field + 6], b"2.003:");
    assert_eq!(p.offset, Some(field));
}

#[test]
fn c19_duplicate_with_subfields() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.003:a\x1eb\x1d2.003:c\x1ed")]);
    assert_eq!(text(&b, 3), Value::from(vec!["c", "d"]));
}

#[test]
fn c20_field_of_another_type() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d3.004:a")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "BAD_RECORD_NUMBER: Illegal record number in tag name 3.004:"
    );
    let p = e.position();
    assert_eq!((p.record_index, p.record_type), (Some(1), Some(2)));
    assert_eq!(p.tag, Some(PyInt::from(4)));
    assert_eq!(&b[p.offset.unwrap()..p.offset.unwrap() + 6], b"3.004:");
}

#[test]
fn c21_empty_field_between_two_gs() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d\x1d2.004:a")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.to_string(), "BAD_RECORD: Invalid record data");
    let p = e.position();
    assert_eq!((p.record_index, p.record_type), (Some(1), Some(2)));
    let at = p.offset.unwrap();
    assert_eq!(&b[at - 1..=at], b"\x1d\x1d");
}

#[test]
fn c22_tag_without_digits() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.:a")]);
    assert_eq!(raises(parse(&b)), "BAD_RECORD: Invalid record data");
}

#[test]
fn c23_empty_field() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.005:")]);
    assert_eq!(text(&b, 5), Value::from(""));
}

#[test]
fn c24_empty_subfield() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.005:a\x1e\x1eb")]);
    assert_eq!(text(&b, 5), Value::from(vec!["a", "", "b"]));
}

#[test]
fn c25_items() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.005:a\x1fb\x1ec")]);
    let v = text(&b, 5);
    assert_eq!(v.repr(), "[['a', 'b'], 'c']");
}

#[test]
fn c26_first_field_not_len() {
    let b = cat(&[&t1(), &rec_with(2, b"2.002:1\x1c", 5)]);
    assert_eq!(
        ascii(&parse(&b).unwrap(), 1).summary(),
        " 2.002: IDC                           : 1\n 2.005:                               : 17"
    );
}

#[test]
fn c27_first_field_not_len_rewritten() {
    let b = cat(&[&t1(), &rec_with(2, b"2.002:1\x1c", 5)]);
    let out = rewrite(&b);
    assert_eq!(&out[out.len() - 17..], b"2.005:17\x1d2.002:1\x1c");
}

#[test]
fn c28_unknown_tagged_type() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e50\x1f1"), &r(50, b"50.002:1")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::UnknownRecordType(PyInt::from(50)));
    assert_eq!(e.to_string(), "50");
}

#[test]
fn c29_type_4_tagged() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e4\x1f1"), &r(4, b"4.002:1")]);
    assert_eq!(raises(parse(&b)), "4");
}

#[test]
fn c30_two_type1() {
    let b = cat(&[&t1_cnt(b"1\x1f0"), &t1_cnt(b"1\x1f0")]);
    assert_eq!(
        raises(parse(&b)),
        "BAD_TAG_DUPLICATE: Tag 1 already defined in record 1"
    );
}

#[test]
fn c31_no_type1_first() {
    let e = parse(&r(2, b"2.002:1")).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::MissingField);
    assert_eq!(e.to_string(), "object of type 'NoneType' has no len()");
}

#[test]
fn c32_empty_buffer() {
    assert_eq!(raises(parse(b"")), "object of type 'NoneType' has no len()");
}

#[test]
fn c33_buffer_of_4_bytes() {
    assert_eq!(
        raises(parse(b"1.00")),
        "NIST_TOO_LONG: NIST buffer too long (extra bytes)"
    );
}

#[test]
fn c34_one_extra_byte() {
    let b = cat(&[&t1_cnt(b"1\x1f0"), b"\x00"]);
    assert_eq!(
        raises(parse(&b)),
        "NIST_TOO_LONG: NIST buffer too long (extra bytes)"
    );
}

#[test]
fn c35_five_extra_bytes() {
    let b = cat(&[&t1_cnt(b"1\x1f0"), &[0; 5]]);
    assert_eq!(
        raises(parse(&b)),
        "BAD_CONTENT: Could not recognize binary record, bad content or bad record"
    );
}

// Reading LEN (§10.9).

/// `T1() + R(2, b'2.002:1\x1d2.003:abc')`, of LEN 27.
fn len27() -> Vec<u8> {
    cat(&[&t1(), &r(2, b"2.002:1\x1d2.003:abc")])
}

#[test]
fn c36_len_too_small() {
    let b = replace(&len27(), b"2.001:27", b"2.001:15");
    assert_eq!(
        raises(parse(&b)),
        "NIST_TOO_LONG: NIST buffer too long (extra bytes)"
    );
}

#[test]
fn c37_len_too_large_plus_one_byte() {
    let b = cat(&[&replace(&len27(), b"2.001:27", b"2.001:28"), b"x"]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.to_string(), "BAD_RECORD: Invalid record data");
    let p = e.position();
    // The LEN takes the `x` into the Type-2, where it is a second record.
    assert_eq!((p.record_index, p.record_type), (Some(1), Some(2)));
    assert_eq!(p.offset, Some(b.len() - 1));
}

#[test]
fn c38_len_too_large_at_end() {
    let b = replace(&len27(), b"2.001:27", b"2.001:28");
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 2"
    );
    let p = e.position();
    assert_eq!((p.record_index, p.record_type), (Some(1), Some(2)));
    assert_eq!(p.offset, Some(t1().len()));
}

/// The records of C39 and C40.
fn two_records(extra: impl Fn(usize, usize) -> usize) -> Vec<u8> {
    let a = r(2, b"2.002:1\x1d2.003:a");
    let b = r(2, b"2.002:2\x1d2.003:b");
    let from = format!("2.001:{}", a.len());
    let to = format!("2.001:{}", extra(a.len(), b.len()));
    cat(&[
        &t1_cnt(b"1\x1f2\x1e2\x1f1\x1e2\x1f2"),
        &replace(&a, from.as_bytes(), to.as_bytes()),
        &b,
    ])
}

#[test]
fn c39_len_over_next_record() {
    assert_eq!(
        raises(parse(&two_records(|a, _| a + 3))),
        "BAD_RECORD: Invalid record data"
    );
}

#[test]
fn c40_len_over_whole_next_record() {
    assert_eq!(
        raises(parse(&two_records(|a, b| a + b))),
        "BAD_TAG_DUPLICATE: Tag 1 already defined in record 2"
    );
}

#[test]
fn c41_last_record_without_fs_nor_999() {
    let b = cat(&[&t1(), b"2.001:15\x1d2.002:1abcde"]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::EmptySequence);
    assert_eq!(e.to_string(), "min() iterable argument is empty");
}

#[test]
fn c42_last_record_without_fs_len_too_large() {
    let full = cat(&[&t1(), &r(2, b"2.002:1")]);
    assert_eq!(
        raises(parse(&full[..full.len() - 1])),
        "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 2"
    );
}

// Reading CNT (§10.9–10.10).

#[test]
fn c43_wrong_type_only() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e9\x1f1"), &r(2, b"2.002:1")]);
    assert_eq!(parse(&b).unwrap().len(), 2);
}

#[test]
fn c44_wrong_idc_only() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e2\x1f7"), &r(2, b"2.002:1")]);
    assert_eq!(parse(&b).unwrap().len(), 2);
}

#[test]
fn c45_wrong_type_and_idc() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e9\x1f7"), &r(2, b"2.002:1")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "BAD_CONTENT: Bad CNT tag in record 1 (bad record type or bad IDC for subfield #1)"
    );
    let p = e.position();
    assert_eq!(p.record_index, Some(1));
    assert_eq!(p.tag, Some(PyInt::from(3)));
    assert_eq!(p.offset, None);
}

#[test]
fn c46_wrong_announced_count() {
    let b = cat(&[&t1_cnt(b"1\x1f5\x1e2\x1f1"), &r(2, b"2.002:1")]);
    assert_eq!(parse(&b).unwrap().len(), 2);
}

#[test]
fn c47_announced_record_missing() {
    assert_eq!(
        raises(parse(&t1_cnt(b"1\x1f1\x1e2\x1f1"))),
        "BAD_CONTENT: Bad CNT tag in record 1 (different number of records)"
    );
}

#[test]
fn c48_missing_cnt() {
    let b = cat(&[&rec(1, b"1.002:0400\x1c"), &r(2, b"2.002:1")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::MissingField);
    assert_eq!(e.to_string(), "object of type 'NoneType' has no len()");
}

#[test]
fn c49_simple_cnt() {
    assert_eq!(
        raises(parse(&rec(1, b"1.002:0400\x1d1.003:1\x1c"))),
        "BAD_CONTENT: Bad CNT tag in record 1 (different number of records)"
    );
}

#[test]
fn c50_non_numeric_cnt_type() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1eX\x1f1"), &r(2, b"2.002:1")]);
    assert_eq!(
        raises(parse(&b)),
        "invalid literal for int() with base 10: 'X'"
    );
}

#[test]
fn c51_first_subfield_of_type_2() {
    assert_eq!(
        raises(parse(&t1_cnt(b"2\x1f0"))),
        "BAD_CONTENT: Bad CNT tag in record 1 (bad record type for subfield #0)"
    );
}

// Reading the binary field 999 (§10.9).

/// `T14(b'14.002:1\x1d14.020:ok', data)`.
fn t14_ok(data: &[u8]) -> Vec<u8> {
    cat(&[&t1_14(), &t14(b"14.002:1\x1d14.020:ok", data)])
}

#[test]
fn c52_data_with_fs_and_gs() {
    assert_eq!(data(&t14_ok(b"\x1c\x1d\x00data"), 999), b"\x1c\x1d\x00data");
}

#[test]
fn c53_data_rewritten_identically() {
    let b = t14_ok(b"\x1c\x1d\x00data");
    assert_eq!(rewrite(&b), b);
}

#[test]
fn c54_999_in_text_value() {
    let b = cat(&[&t1_14(), &t14(b"14.002:1\x1d14.020:see 999:x", b"DATA")]);
    assert_eq!(
        ascii(&parse(&b).unwrap(), 1).summary(),
        "14.001: LEN                           : 48\n14.002: IDC                           : 1\n14.999: DATA                          : <buffer, size=13>"
    );
}

#[test]
fn c55_last_byte_not_fs() {
    let mut b = t14_ok(b"DATA");
    *b.last_mut().unwrap() = b'Z';
    assert_eq!(data(&b, 999), b"DATA");
}

#[test]
fn c56_999_not_last() {
    let b = cat(&[
        &t1_14(),
        &t14(b"14.002:1\x1d14.999:x\x1d14.020:ok", b"DATA"),
    ]);
    assert_eq!(
        ascii(&parse(&b).unwrap(), 1).summary(),
        "14.001: LEN                           : 50\n14.002: IDC                           : 1\n14.999: DATA                          : <buffer, size=23>"
    );
}

#[test]
fn c57_empty_data() {
    let b = cat(&[&t1_14(), &rec(14, b"14.002:1\x1d14.999:\x1c")]);
    assert_eq!(data(&b, 999), b"");
}

#[test]
fn c58_malformed_999_label() {
    let b = cat(&[&t1_14(), &rec(14, b"14.002:1\x1dX999:abc\x1c")]);
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "BAD_TAG_FORMAT: Illegal format for tag 999 (X999:)"
    );
    let p = e.position();
    assert_eq!((p.record_index, p.record_type), (Some(1), Some(14)));
    let at = p.offset.unwrap();
    assert_eq!(&b[at..at + 5], b"X999:");
}

#[test]
fn c59_type_2_with_2_999() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x1d2.999:abc")]);
    assert_eq!(text(&b, 999), Value::from("abc"));
}

// Reading binary records (§10.9).

#[test]
fn c60_type_4_rewritten_identically() {
    let b = type4();
    let mut msg = parse(&b).unwrap();
    assert_eq!(msg.record(1).unwrap().idc().unwrap(), Value::from(3));
    assert_eq!(msg.to_bytes().unwrap(), b);
}

#[test]
fn c61_type_4_truncated() {
    let full = type4();
    let b = &full[..full.len() - 2];
    let e = parse(b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "NIST_TOO_SHORT: NIST buffer too short (missing bytes) when parsing record 1"
    );
    let p = e.position();
    assert_eq!(p.record_index, Some(1));
    assert_eq!(p.offset, Some(t1_cnt(b"1\x1f1\x1e4\x1f3").len()));
}

#[test]
fn c62_no_cnt_entry() {
    let b = cat(&[&t1_cnt(b"1\x1f0"), &9u32.to_be_bytes(), b"\x01data"]);
    let e = parse(&b).unwrap_err();
    assert_eq!(
        e.to_string(),
        "BAD_CONTENT: Could not recognize binary record, bad content or bad record"
    );
    let source = std::error::Error::source(&e).unwrap();
    assert_eq!(source.to_string(), "list index out of range");
}

#[test]
fn c63_binary_len_zero() {
    let b = cat(&[
        &t1_cnt(b"1\x1f1\x1e4\x1f3"),
        &0u32.to_be_bytes(),
        b"\x03xxxx",
    ]);
    assert_eq!(
        raises(parse(&b)),
        "BAD_CONTENT: Could not recognize binary record, bad content or bad record"
    );
}

#[test]
fn c64_binary_len_3() {
    let b = cat(&[
        &t1_cnt(b"1\x1f1\x1e4\x1f1"),
        &3u32.to_be_bytes(),
        b"\x01data",
    ]);
    assert_eq!(
        raises(parse(&b)),
        "BAD_CONTENT: Could not recognize binary record, bad content or bad record"
    );
}

#[test]
fn c65_cnt_announces_type_2() {
    let b = cat(&[&t1(), &9u32.to_be_bytes(), b"\x01data"]);
    assert_eq!(
        raises(parse(&b)),
        "UNKNOWN_ATTRIBUTE: Bad attribute name <value> while trying to define a field in record of type 2"
    );
}

#[test]
fn c66_cnt_entry_without_item() {
    let b = cat(&[&t1_cnt(b"1\x1f1\x1e4"), &9u32.to_be_bytes(), b"\x01data"]);
    assert_eq!(
        raises(parse(&b)),
        "BAD_CONTENT: Bad CNT tag in record 1 (bad number of values for subfield #1)"
    );
}

// Writing (§5–§10).

#[test]
fn c67_record_reduced_to_len() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.remove_field(2);
    assert_eq!(r.to_bytes().unwrap(), b"2.001:9\x1d\x1c");
}

#[test]
fn c68_record_without_fields() {
    let mut r = AsciiRecord::with(2, false, true).unwrap();
    let e = r.to_bytes().unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::IndexOutOfRange);
    assert_eq!(e.to_string(), "list index out of range");
}

#[test]
fn c69_first_field_2_005() {
    let mut r = AsciiRecord::with(2, false, true).unwrap();
    r.add_field(Field::new(2, 5, "a").unwrap()).unwrap();
    assert_eq!(r.to_bytes().unwrap(), b"2.005:9\x1d\x1c");
}

#[test]
fn c70_binary_fields_last_in_insertion_order() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.add_field(BinaryField::new(2, 999, b"B").unwrap())
        .unwrap()
        .add_field(Field::new(2, 5, "z").unwrap())
        .unwrap()
        .add_field(BinaryField::new(2, 998, b"A").unwrap())
        .unwrap();
    assert_eq!(
        r.to_bytes().unwrap(),
        b"2.001:41\x1d2.002:0\x1d2.005:z\x1d2.999:B\x1d2.998:A\x1c"
    );
}

#[test]
fn c71_text_idc_in_cnt() {
    let mut msg = Message::new();
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("IDC", "07").unwrap();
    msg.push(r).unwrap();
    assert_eq!(
        msg.cnt().unwrap().to_bytes().unwrap(),
        b"1.003:1\x1f1\x1e2\x1f07"
    );
}

#[test]
fn c72_tag_1000() {
    let f = Field::new(2, 1000, "x").unwrap();
    assert_eq!(f.to_bytes().unwrap(), b"2.1000:x");
}

#[test]
fn c73_none_field() {
    assert_eq!(nist(()), b"2.003:");
}

#[test]
fn c74_zero_field() {
    assert_eq!(nist(0), b"2.003:0");
}

#[test]
fn c75_true_field() {
    assert_eq!(nist(true), b"2.003:True");
}

#[test]
fn c76_empty_list() {
    let f = Field::new(2, 3, Vec::<&str>::new()).unwrap();
    assert_eq!(f.to_bytes().unwrap(), b"2.003:");
    assert_eq!(f.value(), Value::from(""));
    assert_eq!(f.len(), 0);
}

#[test]
fn c77_list_of_empty_list() {
    let f = Field::new(2, 3, vec![Vec::<&str>::new()]).unwrap();
    assert_eq!(f.to_bytes().unwrap(), b"2.003:");
    assert_eq!(f.value(), Value::from(vec![""]));
    assert_eq!(f.len(), 1);
}

#[test]
fn c78_none_subfield() {
    let value = vec![ListElem::from(()), ListElem::from("a")];
    assert_eq!(nist(value), b"2.003:\x1ea");
}

#[test]
fn c79_none_item() {
    let value = vec![vec![Item::from("a"), Item::from(())]];
    assert_eq!(nist(value), b"2.003:a\x1fNone");
}

#[test]
fn c80_single_item() {
    let f = Field::new(2, 3, vec![vec!["a"]]).unwrap();
    assert_eq!(f.to_bytes().unwrap(), b"2.003:a");
    assert_eq!(f.value(), Value::from(vec![vec!["a"]]));
    assert_eq!(f.value().to_string(), "[['a']]");
}

#[test]
fn c81_non_latin1_character() {
    let f = Field::new(2, 3, "\u{20ac}").unwrap();
    let e = f.to_bytes().unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::Encoding);
    assert_eq!(
        e.to_string(),
        "'latin-1' codec can't encode character '\\u20ac' in position 6: ordinal not in range(256)"
    );
}

#[test]
fn c82_binary_idc_300() {
    // `set_idc` takes a `u8`: 300 cannot be built (see the `compile_fail`
    // example of `BinaryRecord`); 255 is written.
    let mut r = BinaryRecord::new(4);
    r.set_idc(255);
    assert_eq!(r.to_bytes().unwrap(), b"\x00\x00\x00\x05\xff");
}

#[test]
fn c83_len_fixed_point() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("_3", "x".repeat(80)).unwrap();
    assert_eq!(&r.to_bytes().unwrap()[..10], b"2.001:105\x1d");
}

// Text summary (§3.3, §8.9, §9.6).

#[test]
fn c84_string_repr() {
    let mut r = AsciiRecord::new(2).unwrap();
    let value = vec![
        ListElem::from(vec!["a'b", "c\"d"]),
        ListElem::from("x\ty\u{a0}\u{ad}\\"),
    ];
    r.add_field(Field::new(2, 3, value).unwrap()).unwrap();
    r.add_field(Field::new(2, 7, ()).unwrap()).unwrap();
    assert_eq!(
        r.to_string(),
        " 2.001: LEN                           : 0\n 2.002: IDC                           : 0\n 2.003:                               : [[\"a'b\", 'c\"d'], 'x\\ty\\xa0\\xad\\\\']\n 2.007:                               : None"
    );
}

#[test]
fn c85_size_with_thousands_separator() {
    let mut r = BinaryRecord::new(4);
    r.set_value(vec![b'x'; 1_234_567]);
    assert_eq!(
        r.summary().unwrap(),
        " 4.001: LEN                           : 1234572\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=1,234,567>"
    );
}

#[test]
fn c86_default_binary_record() {
    assert_eq!(
        BinaryRecord::new(4).summary().unwrap(),
        " 4.001: LEN                           : 5\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=0>"
    );
}

#[test]
fn c87_pack_little_endian() {
    let mut r = BinaryRecord::new(4);
    r.pack("<HH", &[1.into(), 2.into()], b"IMG").unwrap();
    assert_eq!(
        r.to_bytes().unwrap(),
        b"\x00\x00\x00\x0c\x01\x01\x00\x02\x00IMG"
    );
    assert_eq!(
        r.summary().unwrap(),
        " 4.001: LEN                           : 12\n 4.002: IDC                           : 1\n 4.---:                               : <buffer, size=7>\n                                      -> 1\n                                      -> 2"
    );
}

#[test]
fn c88_unpack_too_long() {
    let mut r = BinaryRecord::new(4);
    let e = r.unpack("!LLLLLLL").unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::Struct);
    assert_eq!(e.to_string(), "unpack requires a buffer of 28 bytes");
}

// Message (§10.2–10.5).

#[test]
fn c89_type1_by_key() {
    let msg = Message::new();
    assert_eq!(
        raises(msg.by_key(1, 0)),
        "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <IDC> while trying to retrieve a field in record of type 1"
    );
}

#[test]
fn c90_delete_missing_key() {
    let mut msg = Message::new();
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    let e = msg.remove_by_key(2, 9).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
    assert_eq!(
        e.to_string(),
        "list indices must be integers or slices, not tuple"
    );
}

#[test]
fn c91_type1_cannot_be_deleted() {
    // `del msg[-2]` has no equivalent: indices are `usize`. Both ways to
    // reach the Type-1 are refused.
    let mut msg = Message::new();
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    let type1 = msg.record(0).unwrap().clone();
    for e in [
        msg.remove(0).unwrap_err(),
        msg.remove_record(&type1).unwrap_err(),
    ] {
        assert_eq!(e.nist_error(), Some(NistError::CannotDeleteType1));
    }
    assert_eq!(msg.len(), 2);
}

#[test]
fn c92_same_record_twice() {
    let mut msg = Message::new();
    let r = AsciiRecord::new(2).unwrap();
    msg.push(r.clone()).unwrap();
    msg.push(r).unwrap();
    assert_eq!(msg.len(), 3);
}

#[test]
fn c93_same_idc_twice() {
    let mut msg = Message::new();
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    msg.push(AsciiRecord::new(2).unwrap()).unwrap();
    assert_eq!(msg.len(), 3);
}

#[test]
fn c94_default_tot() {
    assert_eq!(Message::new().tot().unwrap(), Value::from(""));
}

#[test]
fn c95_default_tcn_length() {
    let msg = Message::new();
    let tcn = msg.type1().unwrap().get("TCN").unwrap();
    assert_eq!(tcn.to_string().len(), 32);
}

#[test]
fn c96_unknown_record_type() {
    let e = AsciiRecord::new(100).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::UnknownRecordType(PyInt::from(100)));
    assert_eq!(e.to_string(), "100");
}

// Access by alias (§8.5–8.6).

/// `[f.tag for f in r._fields]`.
fn tags(r: &AsciiRecord) -> Vec<String> {
    r.fields().iter().map(|f| f.tag().to_string()).collect()
}

#[test]
fn c97_len_set_then_written() {
    let mut r = AsciiRecord::new(2).unwrap();
    r.set("LEN", 99).unwrap();
    assert_eq!(r.to_bytes().unwrap(), b"2.001:17\x1d2.002:0\x1c");
}

#[test]
fn c98_data_on_type_2() {
    let mut r = AsciiRecord::new(2).unwrap();
    let e = r.set_bytes("DATA", b"x").unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::TypeMismatch);
    assert_eq!(
        e.to_string(),
        "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
    );
}

#[test]
fn c99_underscore_plus() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.set("_+7", "v").unwrap();
    assert_eq!(tags(&r), ["1", "2", "7"]);
}

#[test]
fn c100_underscore_inner_underscore() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.set("_0_7", "v").unwrap();
    assert_eq!(tags(&r), ["1", "2", "7"]);
}

#[test]
fn c101_underscore_minus() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.set("_-7", "v").unwrap();
    assert_eq!(tags(&r), ["1", "2", "-7"]);
}

#[test]
fn c102_underscore_alone() {
    let mut r = AsciiRecord::new(10).unwrap();
    assert_eq!(
        raises(r.set("_", "v")),
        "UNKNOWN_ATTRIBUTE: Bad attribute name <_> while trying to define a field in record of type 10"
    );
}

#[test]
fn c103_alias_before_assignment() {
    let r = AsciiRecord::new(10).unwrap();
    assert_eq!(
        raises(r.get("SRC")),
        "UNKNOWN_ATTRIBUTE: Unkown or bad attribute <SRC> while trying to retrieve a field in record of type 10"
    );
}

#[test]
fn c104_underscore_999_creates_binary_field() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.set_bytes("_999", b"img").unwrap();
    let f = r.field(999).unwrap();
    assert!(matches!(f, AnyField::Binary(_)));
    assert_eq!(f.alias(), "DATA");
}

#[test]
fn c105_data_set_again() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.set_bytes("DATA", b"a").unwrap();
    r.set_bytes("DATA", b"b").unwrap();
    assert_eq!(r.get_bytes("_999").unwrap(), b"b");
}

#[test]
fn c106_src_alias_on_tag_3() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.add_field(Field::with(10, 3, "x", "FSI", "%d.%03d:", "SRC").unwrap())
        .unwrap();
    r.set("SRC", "y").unwrap();
    let fields: Vec<(String, &str, Value)> = r
        .fields()
        .iter()
        .map(|f| match f {
            AnyField::Text(f) => (f.tag().to_string(), f.alias(), f.value()),
            AnyField::Binary(_) => unreachable!(),
        })
        .collect();
    assert_eq!(
        fields,
        [
            ("1".to_owned(), "LEN", Value::from(0)),
            ("2".to_owned(), "IDC", Value::from(0)),
            ("3".to_owned(), "SRC", Value::from("y")),
        ]
    );
}

#[test]
fn c107_explicit_alias_kept() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.add_field(Field::with(10, 4, "x", "FSI", "%d.%03d:", "MINE").unwrap())
        .unwrap();
    assert_eq!(r.field(4).unwrap().alias(), "MINE");
}

#[test]
fn c108_tag_4_aliased_mine_then_src() {
    let mut r = AsciiRecord::new(10).unwrap();
    r.add_field(Field::with(10, 4, "x", "FSI", "%d.%03d:", "MINE").unwrap())
        .unwrap();
    let e = r.set("SRC", "y").unwrap_err();
    assert_eq!(e.nist_error(), Some(NistError::BadTagDuplicate));
    assert_eq!(
        e.to_string(),
        "BAD_TAG_DUPLICATE: Tag 4 already defined in record 10"
    );
}

#[test]
fn c109_delete_by_alias_has_no_effect() {
    // `remove_field` only takes a number: `del r['IDC']`, which never
    // removes anything, cannot be written (see the `compile_fail` example of
    // `AsciiRecord::remove_field`).
    let r = AsciiRecord::new(2).unwrap();
    assert_eq!(tags(&r), ["1", "2"]);
}

#[test]
fn c110_field_by_alias() {
    let r = AsciiRecord::new(2).unwrap();
    assert_eq!(r.field("LEN").unwrap().tag(), &PyInt::from(1));
}

// SubField (§5).

#[test]
fn c111_list_of_items() {
    let sf = SubField::new(vec!["a", "b"], "SI").unwrap();
    assert_eq!(
        sf.value(),
        SubValue::Items(vec![Item::from("a"), Item::from("b")])
    );
    assert_eq!(sf.value().to_string(), "['a', 'b']");
    assert_eq!(sf.to_bytes().unwrap(), b"a\x1fb");
}

#[test]
fn c112_add_no_values() {
    let mut sf = SubField::default();
    sf.add_values(Vec::<Item>::new()).unwrap();
    assert_eq!(sf.value(), SubValue::Scalar(Scalar::from("")));
}

#[test]
fn c113_empty_kind() {
    let sf = SubField::new("", "").unwrap();
    assert_eq!(sf.to_bytes().unwrap(), b"");
}

#[test]
fn c114_s_field_with_value() {
    let e = Field::with(2, 3, "x", "S", "%d.%03d:", "").unwrap_err();
    assert_eq!(e.nist_error(), Some(NistError::BadFieldValue));
    assert_eq!(
        e.to_string(),
        "BAD_FIELD_VALUE: Field 2.003: cannot have a value (only subfields and/or items)"
    );
}

// Reading numbers, complements (§3.1).

#[test]
fn c115_long_invalid_idc() {
    let b = cat(&[&t1(), &r(2, &cat(&[b"2.002:", &[b'A'; 300]]))]);
    // `%.200R`: the repr is cut at 200 characters, quote included.
    let expected = format!(
        "invalid literal for int() with base 10: '{}",
        "A".repeat(199)
    );
    assert_eq!(raises(parse(&b)), expected);
}

#[test]
fn c116_idc_followed_by_nul() {
    let b = cat(&[&t1(), &r(2, b"2.002:1\x00")]);
    assert_eq!(
        raises(parse(&b)),
        "invalid literal for int() with base 10: '1\\x00'"
    );
}

#[test]
fn c117_idc_of_4301_digits() {
    let b = cat(&[&t1(), &r(2, &cat(&[b"2.002:", &[b'1'; 4301], b"\x00A"]))]);
    let e = parse(&b).unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::DigitsLimit);
    assert_eq!(
        e.to_string(),
        "Exceeds the limit (4300 digits) for integer string conversion: value has 4301 digits; use sys.set_int_max_str_digits() to increase the limit"
    );
}

#[test]
fn c118_text_tag_after_fs() {
    let e = Field::new(2, "\x1c3", "x").unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::InvalidLiteral);
    assert_eq!(
        raises(Field::new(2, "\x1c3", "x")),
        "invalid literal for int() with base 10: '\\x1c3'"
    );
}
