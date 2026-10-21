//! Edge cases of SPEC appendix C, checked against `nistitl` 0.6 (§15.2).
//!
//! One test per case: same values read, same bytes written, or same text
//! for the error.

use ansi_nist_itl::{
    AnyField, AsciiRecord, BinaryField, Error, ErrorKind, Field, Item, ListElem, NistError, PyInt,
    Scalar, SubField, SubValue, Value,
};

/// `str()` of the expected exception.
fn raises<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
    r.unwrap_err().to_string()
}

/// `Field(2, 3, value).NIST`.
fn nist(value: impl Into<Value>) -> Vec<u8> {
    Field::new(2, 3, value).unwrap().to_bytes().unwrap()
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

// Message (§10.2–10.5).

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
fn c118_text_tag_after_fs() {
    let e = Field::new(2, "\x1c3", "x").unwrap_err();
    assert_eq!(e.kind(), &ErrorKind::InvalidLiteral);
    assert_eq!(
        raises(Field::new(2, "\x1c3", "x")),
        "invalid literal for int() with base 10: '\\x1c3'"
    );
}
