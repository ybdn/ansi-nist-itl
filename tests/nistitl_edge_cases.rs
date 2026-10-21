//! Edge cases of SPEC appendix C, checked against `nistitl` 0.6 (§15.2).
//!
//! One test per case: same values read, same bytes written, or same text
//! for the error.

use ansi_nist_itl::{
    Error, ErrorKind, Field, Item, ListElem, NistError, Scalar, SubField, SubValue, Value,
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
